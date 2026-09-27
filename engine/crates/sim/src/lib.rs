//! Portable engine contracts. No browser objects or concrete backend ownership.

pub mod contracts;
pub mod gpu_layout;
pub mod snapshot;

/// Physical cell width and represented depth in metres.
pub const CELL_WIDTH_M: f64 = 0.01;
/// Represented depth of a cell in metres.
pub const REPRESENTED_DEPTH_M: f64 = 0.01;
/// Fixed outer interval. Only accepted substeps advance physical time.
pub const OUTER_DT_S: f64 = 1.0 / 60.0;

/// Validated rectangular grid geometry; cells are row-major.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grid {
    width: u32,
    height: u32,
}

impl Grid {
    /// Validates JS-facing dimensions before any integer conversion or allocation.
    pub fn new(width: f64, height: f64) -> Result<Self, &'static str> {
        let valid = |n: f64| n.is_finite() && n.fract() == 0.0 && (1.0..=4096.0).contains(&n);
        if !valid(width) || !valid(height) || width * height > 1_048_576.0 {
            return Err("grid dimensions must be integers in 1..4096 with at most 1048576 cells");
        }
        Ok(Self {
            width: width as u32,
            height: height as u32,
        })
    }

    /// Horizontal cell count.
    pub fn width(self) -> u32 {
        self.width
    }
    /// Vertical cell count.
    pub fn height(self) -> u32 {
        self.height
    }
    /// Number of owned cell entries.
    pub fn cells(self) -> usize {
        self.width as usize * self.height as usize
    }

    /// Row-major cell index when the coordinate is inside the grid.
    pub fn cell_index(self, x: u32, y: u32) -> Option<usize> {
        (x < self.width && y < self.height).then(|| y as usize * self.width as usize + x as usize)
    }

    /// Index of a horizontal-velocity face in a (width + 1) by height MAC grid.
    pub fn u_face_index(self, x: u32, y: u32) -> Option<usize> {
        (x <= self.width && y < self.height)
            .then(|| y as usize * (self.width as usize + 1) + x as usize)
    }

    /// Index of a vertical-velocity face in a width by (height + 1) MAC grid.
    pub fn v_face_index(self, x: u32, y: u32) -> Option<usize> {
        (x < self.width && y <= self.height).then(|| y as usize * self.width as usize + x as usize)
    }

    /// Number of horizontal-velocity faces.
    pub fn u_faces(self) -> usize {
        (self.width as usize + 1) * self.height as usize
    }

    /// Number of vertical-velocity faces.
    pub fn v_faces(self) -> usize {
        self.width as usize * (self.height as usize + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_scale_and_real_grid_are_preserved() {
        assert_eq!(Grid::new(480.0, 270.0).unwrap().cells(), 129_600);
        assert_eq!(CELL_WIDTH_M, 0.01);
        assert_eq!(OUTER_DT_S, 1.0 / 60.0);
    }

    #[test]
    fn rejects_before_lossy_conversion_or_large_allocation() {
        for bad in [0.0, -1.0, 1.5, f64::NAN, f64::INFINITY, 4_294_967_297.0] {
            assert!(Grid::new(bad, 1.0).is_err());
            assert!(Grid::new(1.0, bad).is_err());
        }
        assert!(Grid::new(4096.0, 4096.0).is_err());
    }

    #[test]
    fn accepts_limits_but_rejects_the_next_cell() {
        assert_eq!(Grid::new(4096.0, 256.0).unwrap().cells(), 1_048_576);
        assert!(Grid::new(4096.0, 257.0).is_err());
        assert!(Grid::new(4097.0, 1.0).is_err());
        assert_eq!(Grid::new(1.0, 4096.0).unwrap().height(), 4096);
    }
}
