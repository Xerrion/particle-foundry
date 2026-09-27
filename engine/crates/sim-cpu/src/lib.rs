//! Rust f64 reference backend. The bootstrap deliberately exposes no fluid stepping.

use particle_sim::Grid;

/// Lifecycle bootstrap, replaced with an owned reference world at E05/E06.
#[derive(Debug)]
pub struct ReferenceSession {
    grid: Grid,
}

impl ReferenceSession {
    /// Constructs an isolated session with validated geometry.
    pub fn new(grid: Grid) -> Self {
        Self { grid }
    }
    /// Returns immutable geometry, not a mutable world view.
    pub fn grid(&self) -> Grid {
        self.grid
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
