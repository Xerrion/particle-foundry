//! One detached f32 phase and marker transport candidate on a closed MAC grid.
//!
//! Face passes own signed fluxes. Cell passes gather those fluxes in X then Y
//! order, matching the f64 reference. The caller must inspect the four-byte
//! status before publishing the candidate buffers. A failed candidate does not
//! replace the input buffers.

mod bindings;
mod buffers;
mod encoding;

use particle_sim::Grid;

use buffers::preflight;

/// Maximum normal displacement in cell widths for one transport substep.
pub const MAX_FACE_CFL: f32 = 0.5;
/// Status bit for invalid face geometry, velocity, aperture, or CFL.
pub const STATUS_INVALID_FACE: u32 = 1;
/// Status bit for invalid phase, marker, or wall state.
pub const STATUS_INVALID_CELL: u32 = 2;
/// Status bit for a directional donor losing more amount than it owns.
pub const STATUS_DONOR_OVERDRAW: u32 = 4;
/// Status bit for a completed cell with an incompatible phase volume.
pub const STATUS_VOLUME_CLOSURE: u32 = 8;
/// Status bit for a shared-face correction beyond its bounded CFL budget.
pub const STATUS_CORRECTION_LIMIT: u32 = 16;

const WORKGROUP_SIZE: u32 = 64;
const FACE_FLUX_BYTES: u64 = 5 * size_of::<f32>() as u64;
// Each face is visited once per round. The shader limits one adjustment to
// 1e-5 cell widths, so 256 rounds cap total correction at 256e-5 per face.
const LOCAL_CLOSURE_ROUNDS: usize = 256;
/// The four incident faces can each receive one bounded adjustment per round.
pub(crate) const CLOSURE_CELL_CFL_HEADROOM: f64 = 4.0 * LOCAL_CLOSURE_ROUNDS as f64 * 1e-5;

/// Physical values for one bounded transport substep.
#[derive(Clone, Copy, Debug)]
pub struct GpuTransportStep {
    /// Accepted substep duration in seconds.
    pub dt_s: f32,
    /// Liquid density in kg/m³.
    pub liquid_density_kg_m3: f32,
    /// Carrier density in kg/m³.
    pub carrier_density_kg_m3: f32,
}

/// Resident input fields. Mass and marker arrays are component-major with
/// liquid in slot zero and carrier in slot one. Headers use `GpuCellHeader`.
pub struct GpuTransportInput<'a> {
    /// Two component-major f32 phase-mass slots, in kg.
    pub mass_kg: &'a wgpu::Buffer,
    /// Two component-major f32 associated-marker slots.
    pub marker: &'a wgpu::Buffer,
    /// Cell headers with fixed-wall flags.
    pub headers: &'a wgpu::Buffer,
    /// Projected horizontal MAC face speeds in m/s. Requires `COPY_SRC`.
    pub u_velocity_m_s: &'a wgpu::Buffer,
    /// Projected vertical MAC face speeds in m/s. Requires `COPY_SRC`.
    pub v_velocity_m_s: &'a wgpu::Buffer,
    /// Horizontal face aperture fractions in zero to one.
    pub u_aperture: &'a wgpu::Buffer,
    /// Vertical face aperture fractions in zero to one.
    pub v_aperture: &'a wgpu::Buffer,
}

/// Candidate buffers owned by one in-flight substep. Face ledgers have five
/// f32 values per face: signed volume, liquid mass, carrier mass, and markers.
pub struct GpuTransportCandidate {
    /// Relative volume error before the shared-face correction.
    pub base_residual: wgpu::Buffer,
    /// Corrected horizontal speeds used to form one shared phase-flux ledger.
    pub closed_u_velocity_m_s: wgpu::Buffer,
    /// Corrected vertical speeds used to form one shared phase-flux ledger.
    pub closed_v_velocity_m_s: wgpu::Buffer,
    /// Detached liquid/carrier mass after the X sweep.
    pub after_x_mass_kg: wgpu::Buffer,
    /// Detached markers after the X sweep.
    pub after_x_marker: wgpu::Buffer,
    /// Completed candidate mass. Publish only when `status` is zero.
    pub mass_kg: wgpu::Buffer,
    /// Completed candidate markers. Publish only when `status` is zero.
    pub marker: wgpu::Buffer,
    /// Detached unbudgeted face proposals, never consumed by gather or momentum.
    pub provisional_u_flux: wgpu::Buffer,
    /// Detached unbudgeted face proposals, never consumed by gather or momentum.
    pub provisional_v_flux: wgpu::Buffer,
    /// Signed horizontal face flux ledger for compatible momentum.
    pub u_flux: wgpu::Buffer,
    /// Signed vertical face flux ledger for compatible momentum.
    pub v_flux: wgpu::Buffer,
    /// Atomic status word. Read this small buffer before accepting the candidate.
    pub status: wgpu::Buffer,
}

/// Reusable X/Y face production and cell-gather pipelines for one grid.
pub struct GpuTransportStage {
    grid: Grid,
    residual: wgpu::ComputePipeline,
    pair: wgpu::ComputePipeline,
    x_face: wgpu::ComputePipeline,
    budget: wgpu::ComputePipeline,
    x_gather: wgpu::ComputePipeline,
    y_face: wgpu::ComputePipeline,
    y_gather: wgpu::ComputePipeline,
}

impl GpuTransportStage {
    /// Compiles the bounded transport passes after checking effective limits.
    pub fn new(device: &wgpu::Device, grid: Grid) -> Result<Self, String> {
        preflight(grid, &device.limits())?;
        let face_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("transport face flux"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/transport_face.wgsl").into()),
        });
        let gather_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("transport cell gather"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/transport_gather.wgsl").into(),
            ),
        });
        let residual_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("transport volume residual"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/transport_close_residual.wgsl").into(),
            ),
        });
        let pair_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("transport local residual pairs"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/transport_pair.wgsl").into()),
        });
        let budget_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("transport donor budget"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/transport_donor_budget.wgsl").into(),
            ),
        });
        let pipeline = |label, module, entry_point| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: None,
                module,
                entry_point: Some(entry_point),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        Ok(Self {
            grid,
            residual: pipeline(
                "transport volume residual",
                &residual_shader,
                "cell_residual",
            ),
            pair: pipeline("transport local residual pairs", &pair_shader, "pair_faces"),
            x_face: pipeline("transport x faces", &face_shader, "x_faces"),
            budget: pipeline("transport donor budget", &budget_shader, "budget_face"),
            x_gather: pipeline("transport x cells", &gather_shader, "x_cells"),
            y_face: pipeline("transport y faces", &face_shader, "y_faces"),
            y_gather: pipeline("transport y cells", &gather_shader, "y_cells"),
        })
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
