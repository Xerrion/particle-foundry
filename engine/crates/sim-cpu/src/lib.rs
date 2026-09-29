//! Rust f64 reference backend. The bootstrap deliberately exposes no fluid stepping.

pub mod fluid;

use fluid::{PressureFieldError, PressureFields};
use particle_sim::Grid;

/// Lifecycle bootstrap, replaced with an owned reference world at E05/E06.
#[derive(Debug)]
pub struct ReferenceSession {
    grid: Grid,
    pressure_fields: Option<PressureFields>,
}

impl ReferenceSession {
    /// Constructs an isolated session with validated geometry.
    pub fn new(grid: Grid) -> Self {
        Self {
            grid,
            pressure_fields: None,
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
        self.pressure_fields = Some(fields);
        Ok(())
    }

    /// Returns this session's owned pressure fields, if initialized.
    pub fn pressure_fields(&self) -> Option<&PressureFields> {
        self.pressure_fields.as_ref()
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
