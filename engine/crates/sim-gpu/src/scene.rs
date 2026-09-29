//! Private owner for a base-spacing, sealed two-phase GPU fixture.
//!
//! Construction validates all host values before creating GPU resources.
//! The selected generation is the only committed fluid state. No stage can
//! advance its tick or accepted time until the coupled GPU graph exists.

#![allow(dead_code)] // E07 stage graph will consume this private owner.

mod coupled;
pub(crate) mod demo;
mod paint;

pub use coupled::GpuSceneValidationReport;
pub(crate) use coupled::validate_scene_fixture;
#[cfg(target_arch = "wasm32")]
pub(crate) use coupled::{GpuCoupledScene, GpuTickOutcome};
pub(crate) use paint::GpuPaintMaterial;

use particle_sim::{
    Grid,
    gpu_layout::{GpuCellHeader, GpuGridParams},
};
use std::collections::VecDeque;
use wgpu::util::DeviceExt;

use crate::GpuContext;

const COMPONENTS: usize = 2;
const CPU_VOLUME_CLOSURE_REL_TOL: f64 = 1e-12;
// Matches the final-volume gate in transport_gather.wgsl. f32 rounding needs
// a wider gate than the f64 CPU reference, but both inputs and packed values
// must close before this owner allocates resident state.
const GPU_VOLUME_CLOSURE_REL_TOL: f32 = 1e-5;

/// Initial host state for the E07 liquid/carrier/fixed-wall fixture only.
/// Component-major arrays use liquid slot zero and carrier slot one.
pub(crate) struct GpuSceneSeed {
    grid: Grid,
    phase_density_kg_m3: [f64; COMPONENTS],
    mass_kg: Vec<f64>,
    marker: Vec<f64>,
    energy_j: Vec<f64>,
    fixed_wall: Vec<u8>,
    u_velocity_m_s: Vec<f64>,
    v_velocity_m_s: Vec<f64>,
    u_aperture: Vec<f64>,
    v_aperture: Vec<f64>,
    epoch: u64,
    tick: u64,
    accepted_time_s: f64,
}

/// Validated scalar values and deterministic bytes awaiting device preflight.
#[derive(Debug)]
struct PreparedSeed {
    grid: Grid,
    grid_bytes: [u8; 16],
    header_bytes: Vec<u8>,
    mass: Vec<f32>,
    marker: Vec<f32>,
    density: Vec<f32>,
    u_velocity: Vec<f32>,
    v_velocity: Vec<f32>,
    u_aperture: Vec<f32>,
    v_aperture: Vec<f32>,
    component_root: Vec<u32>,
    phase_density_kg_m3: [f32; COMPONENTS],
    epoch: u64,
    tick: u64,
    accepted_time_s: f64,
}

impl PreparedSeed {
    fn new(seed: GpuSceneSeed) -> Result<Self, String> {
        let GpuSceneSeed {
            grid,
            phase_density_kg_m3,
            mass_kg,
            marker,
            energy_j,
            fixed_wall,
            u_velocity_m_s,
            v_velocity_m_s,
            u_aperture,
            v_aperture,
            epoch,
            tick,
            accepted_time_s,
        } = seed;
        let grid_bytes = GpuGridParams::new(grid, COMPONENTS)
            .map_err(|error| error.to_string())?
            .to_le_bytes();
        if epoch == 0 || !accepted_time_s.is_finite() || accepted_time_s < 0.0 {
            return Err("GPU scene needs a positive epoch and finite accepted time".into());
        }
        check_len("component mass", mass_kg.len(), grid.cells() * COMPONENTS)?;
        check_len("phase marker", marker.len(), grid.cells() * COMPONENTS)?;
        check_len("cell energy", energy_j.len(), grid.cells())?;
        check_len("fixed wall", fixed_wall.len(), grid.cells())?;
        check_len("horizontal velocity", u_velocity_m_s.len(), grid.u_faces())?;
        check_len("vertical velocity", v_velocity_m_s.len(), grid.v_faces())?;
        check_len("horizontal aperture", u_aperture.len(), grid.u_faces())?;
        check_len("vertical aperture", v_aperture.len(), grid.v_faces())?;
        validate_faces(
            grid,
            &fixed_wall,
            &u_velocity_m_s,
            &u_aperture,
            &v_velocity_m_s,
            &v_aperture,
        )?;

        let density0 = positive_f32("liquid density", phase_density_kg_m3[0])?;
        let density1 = positive_f32("carrier density", phase_density_kg_m3[1])?;
        let phase_density = [density0, density1];
        let mass = convert_nonnegative("component mass", &mass_kg)?;
        let marker = convert_nonnegative("phase marker", &marker)?;
        let u_velocity = convert_finite("horizontal velocity", &u_velocity_m_s)?;
        let v_velocity = convert_finite("vertical velocity", &v_velocity_m_s)?;
        let u_aperture = convert_finite("horizontal aperture", &u_aperture)?;
        let v_aperture = convert_finite("vertical aperture", &v_aperture)?;
        let component_root = closed_component_roots(grid, &fixed_wall, &u_aperture, &v_aperture);
        let header_bytes = energy_j
            .into_iter()
            .zip(&fixed_wall)
            .map(|(energy, &wall)| {
                GpuCellHeader::new(energy, wall)
                    .map(GpuCellHeader::to_le_bytes)
                    .map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect();

        let cell_volume = grid.cell_volume_m3();
        let packed_cell_volume = (cell_volume as f32).max(0.0);
        if !packed_cell_volume.is_finite() || packed_cell_volume == 0.0 {
            return Err("GPU scene cell volume is not representable in f32".into());
        }
        let mut density = Vec::with_capacity(grid.cells());
        for cell in 0..grid.cells() {
            let liquid = mass_kg[cell];
            let carrier = mass_kg[grid.cells() + cell];
            let liquid_marker = marker[cell];
            let carrier_marker = marker[grid.cells() + cell];
            if (liquid == 0.0 && liquid_marker != 0.0) || (carrier == 0.0 && carrier_marker != 0.0)
            {
                return Err(format!("cell {cell} has a marker without its phase"));
            }
            if (mass[cell] == 0.0 && liquid_marker != 0.0)
                || (mass[grid.cells() + cell] == 0.0 && carrier_marker != 0.0)
            {
                return Err(format!("cell {cell} loses its marker phase in f32"));
            }
            if fixed_wall[cell] == 1 {
                if liquid != 0.0 || carrier != 0.0 || liquid_marker != 0.0 || carrier_marker != 0.0
                {
                    return Err(format!("fixed wall cell {cell} owns fluid amount"));
                }
                // Match the CPU reference's inert carrier coefficient on a
                // fixed wall cell, including non-unit carrier densities.
                density.push(density1);
                continue;
            }
            let closure = liquid / phase_density_kg_m3[0] + carrier / phase_density_kg_m3[1];
            if !closure.is_finite()
                || (closure - cell_volume).abs() > CPU_VOLUME_CLOSURE_REL_TOL * cell_volume
            {
                return Err(format!("cell {cell} does not close f64 phase volume"));
            }
            let packed_closure = mass[cell] / density0 + mass[grid.cells() + cell] / density1;
            if !packed_closure.is_finite()
                || (packed_closure - packed_cell_volume).abs()
                    > GPU_VOLUME_CLOSURE_REL_TOL * packed_cell_volume
            {
                return Err(format!("cell {cell} does not close f32 phase volume"));
            }
            let cell_density = (mass[cell] + mass[grid.cells() + cell]) / packed_cell_volume;
            if !cell_density.is_finite() || cell_density <= 0.0 {
                return Err(format!("cell {cell} has invalid packed density"));
            }
            density.push(cell_density);
        }

        Ok(Self {
            grid,
            grid_bytes,
            header_bytes,
            mass,
            marker,
            density,
            u_velocity,
            v_velocity,
            u_aperture,
            v_aperture,
            component_root,
            phase_density_kg_m3: phase_density,
            epoch,
            tick,
            accepted_time_s,
        })
    }
}

fn check_len(field: &str, actual: usize, expected: usize) -> Result<(), String> {
    if actual != expected {
        return Err(format!("{field} needs {expected} entries, got {actual}"));
    }
    Ok(())
}

fn finite_f32(field: &str, value: f64, index: usize) -> Result<f32, String> {
    let packed = value as f32;
    if !value.is_finite() || !packed.is_finite() || (value != 0.0 && packed == 0.0) {
        return Err(format!(
            "{field}[{index}] is not representable in finite f32"
        ));
    }
    Ok(packed)
}

fn positive_f32(field: &str, value: f64) -> Result<f32, String> {
    let packed = finite_f32(field, value, 0)?;
    if value <= 0.0 || packed <= 0.0 {
        return Err(format!("{field} must be positive in f32"));
    }
    Ok(packed)
}

fn convert_finite(field: &str, values: &[f64]) -> Result<Vec<f32>, String> {
    values
        .iter()
        .enumerate()
        .map(|(index, &value)| finite_f32(field, value, index))
        .collect()
}

fn convert_nonnegative(field: &str, values: &[f64]) -> Result<Vec<f32>, String> {
    values
        .iter()
        .enumerate()
        .map(|(index, &value)| {
            let packed = finite_f32(field, value, index)?;
            if value < 0.0 || (value > 0.0 && packed == 0.0) {
                return Err(format!("{field}[{index}] is negative or lost in f32"));
            }
            Ok(packed)
        })
        .collect()
}

fn validate_faces(
    grid: Grid,
    wall: &[u8],
    u_speed: &[f64],
    u_aperture: &[f64],
    v_speed: &[f64],
    v_aperture: &[f64],
) -> Result<(), String> {
    let width = grid.width() as usize;
    let height = grid.height() as usize;
    for y in 0..height {
        for x in 0..=width {
            let face = y * (width + 1) + x;
            let blocked =
                x == 0 || x == width || wall[y * width + x - 1] == 1 || wall[y * width + x] == 1;
            check_face("u", face, blocked, u_speed[face], u_aperture[face])?;
        }
    }
    for y in 0..=height {
        for x in 0..width {
            let face = y * width + x;
            let blocked =
                y == 0 || y == height || wall[(y - 1) * width + x] == 1 || wall[y * width + x] == 1;
            check_face("v", face, blocked, v_speed[face], v_aperture[face])?;
        }
    }
    Ok(())
}

fn check_face(
    axis: &str,
    face: usize,
    blocked: bool,
    speed: f64,
    aperture: f64,
) -> Result<(), String> {
    if !(0.0..=1.0).contains(&aperture) {
        return Err(format!("{axis} face {face} has invalid aperture"));
    }
    if (blocked && aperture != 0.0) || (aperture == 0.0 && speed != 0.0) {
        return Err(format!(
            "{axis} face {face} violates fixed-wall or blocked-face closure"
        ));
    }
    Ok(())
}

/// The pressure system subtracts each closed fluid component's mean pressure.
/// This map labels each component with its minimum row-major cell index.
/// Static aperture topology is authored on the host, so the map is built once.
/// Rebuild it if the topology changes.
fn closed_component_roots(grid: Grid, wall: &[u8], u_open: &[f32], v_open: &[f32]) -> Vec<u32> {
    let width = grid.width() as usize;
    let height = grid.height() as usize;
    let mut roots = vec![u32::MAX; grid.cells()];
    let mut queue = VecDeque::new();
    for root in 0..grid.cells() {
        if roots[root] != u32::MAX {
            continue;
        }
        roots[root] = root as u32;
        if wall[root] == 1 {
            continue;
        }
        queue.push_back(root);
        while let Some(cell) = queue.pop_front() {
            let x = cell % width;
            let y = cell / width;
            let adjacent = [
                (x > 0, cell.wrapping_sub(1), u_open[y * (width + 1) + x]),
                (x + 1 < width, cell + 1, u_open[y * (width + 1) + x + 1]),
                (y > 0, cell.wrapping_sub(width), v_open[y * width + x]),
                (y + 1 < height, cell + width, v_open[(y + 1) * width + x]),
            ];
            for (inside, neighbor, aperture) in adjacent {
                if inside && aperture > 0.0 && roots[neighbor] == u32::MAX {
                    roots[neighbor] = root as u32;
                    queue.push_back(neighbor);
                }
            }
        }
    }
    roots
}

/// Exact allocation size for the resources created by `GpuScene::new`.
/// Later stage scratch has a separate preflight at its own constructor.
struct SceneAllocationPlan {
    total_bytes: u64,
}

fn preflight_scene_buffers(
    grid: Grid,
    limits: &wgpu::Limits,
) -> Result<SceneAllocationPlan, String> {
    let cell_count = grid.cells() as u64;
    let u_count = grid.u_faces() as u64;
    let v_count = grid.v_faces() as u64;
    let field_sizes = [
        ("mass", cell_count * COMPONENTS as u64 * 4),
        ("marker", cell_count * COMPONENTS as u64 * 4),
        ("density", cell_count * 4),
        ("horizontal velocity", u_count * 4),
        ("vertical velocity", v_count * 4),
    ];
    let immutable_sizes = [
        ("cell header", cell_count * 8),
        ("horizontal aperture", u_count * 4),
        ("vertical aperture", v_count * 4),
        ("component root", cell_count * 4),
    ];
    let mut total_bytes = 16_u64;
    for (label, bytes) in field_sizes.into_iter().chain(immutable_sizes) {
        if bytes > limits.max_storage_buffer_binding_size || bytes > limits.max_buffer_size {
            return Err(format!("GPU scene {label} exceeds effective storage limit"));
        }
    }
    for (_, bytes) in field_sizes {
        total_bytes = total_bytes
            .checked_add(bytes * 2)
            .ok_or("GPU scene allocation size overflow")?;
    }
    for (_, bytes) in immutable_sizes {
        total_bytes = total_bytes
            .checked_add(bytes)
            .ok_or("GPU scene allocation size overflow")?;
    }
    if limits.max_uniform_buffer_binding_size < 16 || limits.max_buffer_size < 16 {
        return Err("GPU scene grid uniform exceeds effective adapter limit".into());
    }
    Ok(SceneAllocationPlan { total_bytes })
}

struct GpuGeneration {
    mass_kg: wgpu::Buffer,
    marker: wgpu::Buffer,
    density_kg_m3: wgpu::Buffer,
    u_velocity_m_s: wgpu::Buffer,
    v_velocity_m_s: wgpu::Buffer,
}

impl GpuGeneration {
    fn initial(device: &wgpu::Device, state: &PreparedSeed) -> Self {
        let state_usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST;
        let upload = |label, values: &[f32]| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: &f32_bytes(values),
                usage: state_usage,
            })
        };
        Self {
            mass_kg: upload("scene phase mass", &state.mass),
            marker: upload("scene phase marker", &state.marker),
            density_kg_m3: upload("scene derived density", &state.density),
            u_velocity_m_s: upload("scene horizontal velocity", &state.u_velocity),
            v_velocity_m_s: upload("scene vertical velocity", &state.v_velocity),
        }
    }
}

fn f32_bytes(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn u32_bytes(values: &[u32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

/// All resident state for one sealed fixture. Only this owner may change
/// `committed_index`, `tick`, or `accepted_time_s` after all stage gates pass.
struct GpuScene {
    context: GpuContext,
    grid: Grid,
    grid_uniform: wgpu::Buffer,
    headers: wgpu::Buffer,
    u_aperture: wgpu::Buffer,
    v_aperture: wgpu::Buffer,
    component_root: wgpu::Buffer,
    generations: [GpuGeneration; 2],
    committed_index: usize,
    phase_density_kg_m3: [f32; COMPONENTS],
    epoch: u64,
    tick: u64,
    accepted_time_s: f64,
    state_revision: u64,
    allocated_bytes: u64,
}

impl GpuScene {
    fn new(context: &GpuContext, seed: GpuSceneSeed) -> Result<Self, String> {
        let prepared = PreparedSeed::new(seed)?;
        context.preflight_core_grid(prepared.grid, COMPONENTS)?;
        let plan = preflight_scene_buffers(prepared.grid, &context.device.limits())?;
        let immutable = |label, bytes: &[u8], usage| {
            context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents: bytes,
                    usage,
                })
        };
        let grid_uniform = immutable(
            "scene grid",
            &prepared.grid_bytes,
            wgpu::BufferUsages::UNIFORM,
        );
        let headers = immutable(
            "scene fixed headers",
            &prepared.header_bytes,
            wgpu::BufferUsages::STORAGE,
        );
        let u_aperture = immutable(
            "scene horizontal aperture",
            &f32_bytes(&prepared.u_aperture),
            wgpu::BufferUsages::STORAGE,
        );
        let v_aperture = immutable(
            "scene vertical aperture",
            &f32_bytes(&prepared.v_aperture),
            wgpu::BufferUsages::STORAGE,
        );
        let component_root = immutable(
            "scene closed component roots",
            &u32_bytes(&prepared.component_root),
            wgpu::BufferUsages::STORAGE,
        );
        let generations = [
            GpuGeneration::initial(&context.device, &prepared),
            GpuGeneration::initial(&context.device, &prepared),
        ];
        Ok(Self {
            context: context.clone(),
            grid: prepared.grid,
            grid_uniform,
            headers,
            u_aperture,
            v_aperture,
            component_root,
            generations,
            committed_index: 0,
            phase_density_kg_m3: prepared.phase_density_kg_m3,
            epoch: prepared.epoch,
            tick: prepared.tick,
            accepted_time_s: prepared.accepted_time_s,
            state_revision: 0,
            allocated_bytes: plan.total_bytes,
        })
    }

    fn committed(&self) -> &GpuGeneration {
        &self.generations[self.committed_index]
    }

    fn candidate(&self) -> &GpuGeneration {
        &self.generations[1 - self.committed_index]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        future::Future,
        sync::Arc,
        task::{Context, Poll, Wake, Waker},
        time::{Duration, Instant},
    };

    fn seed() -> GpuSceneSeed {
        let grid = Grid::new(2.0, 1.0).unwrap();
        let volume = grid.cell_volume_m3();
        GpuSceneSeed {
            grid,
            phase_density_kg_m3: [1000.0, 1.0],
            mass_kg: vec![500.0 * volume, 0.0, 0.5 * volume, volume],
            marker: vec![0.25, 0.0, 0.125, 0.5],
            energy_j: vec![0.0; grid.cells()],
            fixed_wall: vec![0; grid.cells()],
            u_velocity_m_s: vec![0.0; grid.u_faces()],
            v_velocity_m_s: vec![0.0; grid.v_faces()],
            u_aperture: vec![0.0, 1.0, 0.0],
            v_aperture: vec![0.0; grid.v_faces()],
            epoch: 3,
            tick: 7,
            accepted_time_s: 7.0 / 60.0,
        }
    }

    #[test]
    fn invalid_seed_rejects_before_device_allocation() {
        // The CPU reference uses 1e-12 relative closure. Packed GPU state
        // uses the transport_gather.wgsl 1e-5 relative closure gate.
        assert!(PreparedSeed::new(seed()).is_ok());
        let mut missing_phase = seed();
        missing_phase.marker[1] = 1.0;
        assert!(
            PreparedSeed::new(missing_phase)
                .unwrap_err()
                .contains("marker without its phase")
        );

        let mut open_edge = seed();
        open_edge.u_aperture[0] = 1.0;
        assert!(
            PreparedSeed::new(open_edge)
                .unwrap_err()
                .contains("fixed-wall")
        );

        let mut rounded_open_face = seed();
        rounded_open_face.u_aperture[1] = 1.0 + f64::EPSILON;
        assert!(
            PreparedSeed::new(rounded_open_face)
                .unwrap_err()
                .contains("invalid aperture")
        );

        let mut rounded_blocked_speed = seed();
        rounded_blocked_speed.u_velocity_m_s[0] = f64::MIN_POSITIVE;
        assert!(
            PreparedSeed::new(rounded_blocked_speed)
                .unwrap_err()
                .contains("blocked-face")
        );

        let mut wall_with_mass = seed();
        wall_with_mass.fixed_wall[0] = 1;
        wall_with_mass.u_aperture[1] = 0.0;
        assert!(
            PreparedSeed::new(wall_with_mass)
                .unwrap_err()
                .contains("owns fluid")
        );

        let mut nonfinite_packed = seed();
        nonfinite_packed.energy_j[0] = f64::MAX;
        assert!(PreparedSeed::new(nonfinite_packed).is_err());

        let mut missing_volume = seed();
        missing_volume.mass_kg[0] *= 0.9;
        assert!(
            PreparedSeed::new(missing_volume)
                .unwrap_err()
                .contains("f64 phase volume")
        );
    }

    #[test]
    fn preflight_counts_every_buffer_the_constructor_creates() {
        let grid = seed().grid;
        let plan = preflight_scene_buffers(grid, &wgpu::Limits::default()).unwrap();
        let cells = grid.cells() as u64;
        let u = grid.u_faces() as u64;
        let v = grid.v_faces() as u64;
        assert_eq!(
            plan.total_bytes,
            16 + cells * 12 + (u + v) * 4 + 2 * (cells * 20 + (u + v) * 4)
        );
        let limits = wgpu::Limits {
            max_storage_buffer_binding_size: 15,
            ..wgpu::Limits::default()
        };
        assert!(preflight_scene_buffers(grid, &limits).is_err());
    }

    #[test]
    fn closed_component_roots_follow_static_apertures_and_walls() {
        let grid = Grid::new(5.0, 1.0).unwrap();
        let wall = [0, 0, 1, 0, 0];
        let u_aperture = [0.0, 1.0, 0.0, 0.0, 1.0, 0.0];
        let v_aperture = [0.0; 10];
        assert_eq!(
            closed_component_roots(grid, &wall, &u_aperture, &v_aperture),
            [0, 0, 2, 3, 3]
        );
    }

    #[test]
    fn fixed_wall_uses_carrier_density_coefficient() {
        let mut input = seed();
        input.phase_density_kg_m3[1] = 2.0;
        input.mass_kg[2] = input.grid.cell_volume_m3();
        input.fixed_wall[1] = 1;
        input.mass_kg[1] = 0.0;
        input.mass_kg[3] = 0.0;
        input.marker[3] = 0.0;
        input.u_aperture[1] = 0.0;
        let prepared = PreparedSeed::new(input).unwrap();
        assert_eq!(prepared.density[1], 2.0);
    }

    struct Notify(std::thread::Thread);

    impl Wake for Notify {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }

    fn block_on<F: Future>(future: F) -> F::Output {
        let waker: Waker = Arc::new(Notify(std::thread::current())).into();
        let mut context = Context::from_waker(&waker);
        let mut future = Box::pin(future);
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(value) => return value,
                Poll::Pending => {
                    assert!(Instant::now() < deadline, "GPU scene test timed out");
                    std::thread::park_timeout(Duration::from_millis(100));
                }
            }
        }
    }

    async fn read_first_mass(scene: &GpuScene, generation: &GpuGeneration) -> f32 {
        let staging = scene.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene test readback"),
            size: 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder =
            scene
                .context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("scene test readback"),
                });
        encoder.copy_buffer_to_buffer(&generation.mass_kg, 0, &staging, 0, 4);
        scene.context.queue.submit([encoder.finish()]);
        let (sender, receiver) = std::sync::mpsc::channel();
        staging.map_async(wgpu::MapMode::Read, .., move |result| {
            sender.send(result).unwrap();
        });
        scene
            .context
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(Duration::from_secs(10)),
            })
            .unwrap();
        receiver
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .unwrap();
        let mapped = staging.get_mapped_range(..);
        let value = f32::from_le_bytes(mapped[..4].try_into().unwrap());
        drop(mapped);
        staging.unmap();
        value
    }

    #[test]
    #[ignore = "requires a native GPU compute adapter"]
    fn discarded_candidate_keeps_committed_gpu_state_and_clock() {
        block_on(async {
            let context = GpuContext::new().await.unwrap();
            let mut invalid = seed();
            invalid.mass_kg[0] *= 0.9;
            assert!(GpuScene::new(&context, invalid).is_err());
            let scene = GpuScene::new(&context, seed()).unwrap();
            let before = (
                scene.committed_index,
                scene.epoch,
                scene.tick,
                scene.accepted_time_s,
            );
            let committed_mass = read_first_mass(&scene, scene.committed()).await;
            assert!(committed_mass > 0.0);
            assert!(!std::ptr::eq(scene.committed(), scene.candidate()));

            // A rejected stage may have written a detached candidate. Only a
            // later validated coupled completion may switch the index.
            context
                .queue
                .write_buffer(&scene.candidate().mass_kg, 0, &0.0_f32.to_le_bytes());
            assert_eq!(read_first_mass(&scene, scene.candidate()).await, 0.0);
            assert_eq!(
                read_first_mass(&scene, scene.committed()).await,
                committed_mass
            );
            assert_eq!(
                (
                    scene.committed_index,
                    scene.epoch,
                    scene.tick,
                    scene.accepted_time_s
                ),
                before
            );
        });
    }
}
