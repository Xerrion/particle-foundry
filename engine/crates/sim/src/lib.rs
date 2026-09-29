//! Portable engine contracts. No browser objects or concrete backend ownership.

pub mod contracts;
pub mod gpu_layout;
pub mod snapshot;

/// Default physical cell width in metres.
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
    cell_width_m_bits: u64,
}

impl Grid {
    /// Constructs a grid at the default physical scale.
    pub fn new(width: f64, height: f64) -> Result<Self, &'static str> {
        Self::with_cell_width(width, height, CELL_WIDTH_M)
    }

    /// Validates dimensions and cell scale before any integer conversion or allocation.
    pub fn with_cell_width(
        width: f64,
        height: f64,
        cell_width_m: f64,
    ) -> Result<Self, &'static str> {
        let valid = |n: f64| n.is_finite() && n.fract() == 0.0 && (1.0..=4096.0).contains(&n);
        if !valid(width) || !valid(height) || width * height > 1_048_576.0 {
            return Err("grid dimensions must be integers in 1..4096 with at most 1048576 cells");
        }
        let area_m2 = cell_width_m * cell_width_m;
        let volume_m3 = area_m2 * REPRESENTED_DEPTH_M;
        if !cell_width_m.is_finite()
            || cell_width_m <= 0.0
            || !area_m2.is_finite()
            || area_m2 <= 0.0
            || !volume_m3.is_finite()
            || volume_m3 <= 0.0
        {
            return Err("grid cell width must have finite positive area and volume");
        }
        Ok(Self {
            width: width as u32,
            height: height as u32,
            cell_width_m_bits: cell_width_m.to_bits(),
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
    /// Physical cell width in metres.
    pub fn cell_width_m(self) -> f64 {
        f64::from_bits(self.cell_width_m_bits)
    }
    /// Physical volume of one cell at the represented depth, in cubic metres.
    pub fn cell_volume_m3(self) -> f64 {
        self.cell_width_m() * self.cell_width_m() * REPRESENTED_DEPTH_M
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
        let grid = Grid::new(480.0, 270.0).unwrap();
        assert_eq!(grid.cells(), 129_600);
        assert_eq!(grid.cell_width_m(), CELL_WIDTH_M);
        assert_eq!(
            grid.cell_volume_m3(),
            CELL_WIDTH_M * CELL_WIDTH_M * REPRESENTED_DEPTH_M
        );
        assert_eq!(CELL_WIDTH_M, 0.01);
        assert_eq!(OUTER_DT_S, 1.0 / 60.0);
    }

    #[test]
    fn spacing_is_part_of_grid_identity() {
        let default = Grid::new(2.0, 3.0).unwrap();
        let refined = Grid::with_cell_width(2.0, 3.0, 0.005).unwrap();
        assert_ne!(default, refined);
        assert_eq!(refined.cell_width_m(), 0.005);
        assert_eq!(
            refined.cell_volume_m3(),
            0.005 * 0.005 * REPRESENTED_DEPTH_M
        );
        assert_eq!(
            default,
            Grid::with_cell_width(2.0, 3.0, CELL_WIDTH_M).unwrap()
        );
    }

    #[test]
    fn rejects_spacing_without_representable_area_and_volume() {
        for bad in [0.0, -0.01, f64::NAN, f64::INFINITY, 1.0e-162, 1.0e155] {
            assert!(Grid::with_cell_width(2.0, 3.0, bad).is_err());
        }
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
