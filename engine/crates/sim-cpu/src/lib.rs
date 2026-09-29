//! Rust f64 reference backend. Fluid stepping is not yet connected to the live browser.

pub mod assembly;
pub mod fluid;
pub mod operator;
pub mod solver;
pub mod substep;
pub mod transport;
pub mod viscosity;

use assembly::{PressureAssembly, PressureAssemblyError};
use fluid::{FaceValues, PressureFieldError, PressureFields};
use particle_sim::Grid;
use solver::{PressureSolveError, Projection, SolveConfig, project_closed_with_assembly};
use transport::{TransportError, TransportInventory};

/// Versions of the inputs from which a session's pressure coefficients are built.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PressureInputVersions {
    /// Grid and outer-boundary topology. Grid dimensions are fixed per session.
    pub geometry: u64,
    /// Face aperture field, including wall edits.
    pub aperture: u64,
    /// Cell density field used for face coefficients.
    pub density: u64,
}

#[derive(Debug)]
struct CachedPressureAssembly {
    versions: PressureInputVersions,
    assembly: PressureAssembly,
}

/// Failure to project an initialized reference session.
#[derive(Debug)]
pub enum PressureSessionError {
    /// The session has no pressure fields yet.
    MissingFields,
    /// Current inputs could not be assembled.
    Assembly(PressureAssemblyError),
    /// The pressure solve or its input checks failed.
    Solve(PressureSolveError),
}

impl std::fmt::Display for PressureSessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PressureSessionError {}

/// Isolated CPU session with pressure inputs, transported inventory and derived
/// assembly cache. A coupled fluid step remains future work.
#[derive(Debug)]
pub struct ReferenceSession {
    grid: Grid,
    pressure_fields: Option<PressureFields>,
    transport_inventory: Option<TransportInventory>,
    pressure_versions: PressureInputVersions,
    pressure_assembly: Option<CachedPressureAssembly>,
    pressure_assembly_builds: u64,
}

impl ReferenceSession {
    /// Constructs an isolated session with validated geometry.
    pub fn new(grid: Grid) -> Self {
        Self {
            grid,
            pressure_fields: None,
            transport_inventory: None,
            pressure_versions: PressureInputVersions::default(),
            pressure_assembly: None,
            pressure_assembly_builds: 0,
        }
    }
    /// Returns immutable geometry, not a mutable world view.
    pub fn grid(&self) -> Grid {
        self.grid
    }

    /// Replaces this session's pressure inputs after geometry validation.
    pub fn set_pressure_fields(
        &mut self,
        fields: PressureFields,
    ) -> Result<(), PressureFieldError> {
        if fields.grid() != self.grid {
            return Err(PressureFieldError::GridMismatch);
        }
        let mut versions = self.pressure_versions;
        if let Some(previous) = &self.pressure_fields {
            if previous.boundaries() != fields.boundaries() {
                versions.geometry = versions.geometry.wrapping_add(1);
            }
            if previous.aperture() != fields.aperture() {
                versions.aperture = versions.aperture.wrapping_add(1);
            }
            if previous.density_kg_m3() != fields.density_kg_m3() {
                versions.density = versions.density.wrapping_add(1);
            }
        } else {
            versions.geometry = versions.geometry.wrapping_add(1);
            versions.aperture = versions.aperture.wrapping_add(1);
            versions.density = versions.density.wrapping_add(1);
        }
        if versions != self.pressure_versions {
            self.pressure_assembly = None;
        }
        self.pressure_versions = versions;
        self.pressure_fields = Some(fields);
        Ok(())
    }

    /// Returns this session's owned pressure fields, if initialized.
    pub fn pressure_fields(&self) -> Option<&PressureFields> {
        self.pressure_fields.as_ref()
    }

    /// Replaces this session's owned transport inventory after geometry validation.
    pub fn set_transport_inventory(
        &mut self,
        inventory: TransportInventory,
    ) -> Result<(), TransportError> {
        if inventory.grid() != self.grid {
            return Err(TransportError::GridMismatch);
        }
        self.transport_inventory = Some(inventory);
        Ok(())
    }

    /// Returns this session's transported mass and marker authority, if initialized.
    pub fn transport_inventory(&self) -> Option<&TransportInventory> {
        self.transport_inventory.as_ref()
    }

    /// Current private revisions of the three coefficient inputs.
    pub fn pressure_versions(&self) -> PressureInputVersions {
        self.pressure_versions
    }

    /// Number of session-owned coefficient builds, excluding forced comparisons.
    pub fn pressure_assembly_builds(&self) -> u64 {
        self.pressure_assembly_builds
    }

    /// Projects with a session-owned assembly, rebuilding it only after its inputs change.
    pub fn project_pressure_cached(
        &mut self,
        predictor_m_s: &FaceValues,
        volume_source_per_s: &[f64],
        dt_s: f64,
        config: SolveConfig,
    ) -> Result<Projection, PressureSessionError> {
        let fields = self
            .pressure_fields
            .as_ref()
            .ok_or(PressureSessionError::MissingFields)?;
        if self
            .pressure_assembly
            .as_ref()
            .map(|cached| cached.versions)
            != Some(self.pressure_versions)
        {
            let assembly = PressureAssembly::new(fields).map_err(PressureSessionError::Assembly)?;
            self.pressure_assembly = Some(CachedPressureAssembly {
                versions: self.pressure_versions,
                assembly,
            });
            self.pressure_assembly_builds = self.pressure_assembly_builds.wrapping_add(1);
        }
        let assembly = &self
            .pressure_assembly
            .as_ref()
            .expect("built above")
            .assembly;
        project_closed_with_assembly(
            fields,
            assembly,
            predictor_m_s,
            volume_source_per_s,
            dt_s,
            config,
        )
        .map_err(PressureSessionError::Solve)
    }

    /// Projects after a forced coefficient rebuild for cache-parity checks.
    pub fn project_pressure_forced_rebuild(
        &self,
        predictor_m_s: &FaceValues,
        volume_source_per_s: &[f64],
        dt_s: f64,
        config: SolveConfig,
    ) -> Result<Projection, PressureSessionError> {
        let fields = self
            .pressure_fields
            .as_ref()
            .ok_or(PressureSessionError::MissingFields)?;
        let assembly = PressureAssembly::new(fields).map_err(PressureSessionError::Assembly)?;
        project_closed_with_assembly(
            fields,
            &assembly,
            predictor_m_s,
            volume_source_per_s,
            dt_s,
            config,
        )
        .map_err(PressureSessionError::Solve)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolated_sessions_keep_their_own_geometry() {
        let a = ReferenceSession::new(Grid::new(2.0, 3.0).unwrap());
        let b = ReferenceSession::new(Grid::new(4.0, 5.0).unwrap());
        assert_eq!(a.grid().cells(), 6);
        assert_eq!(b.grid().cells(), 20);
    }
}
