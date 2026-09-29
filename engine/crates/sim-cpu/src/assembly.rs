//! Matrix-free sealed pressure assembly for the CPU reference.

use particle_sim::{Grid, contracts::Boundary};

use crate::fluid::{FaceValues, PressureFields};

/// Invalid pressure assembly input or an arithmetic result outside f64.
#[derive(Clone, Debug, PartialEq)]
pub enum PressureAssemblyError {
    /// A reservoir pressure and flux rule is required before assembling this side.
    OpenBoundaryRequiresReservoir {
        /// Left, right, top, or bottom outer boundary.
        side: &'static str,
    },
    /// A positive aperture produced a zero, infinite, or NaN face coefficient.
    InvalidFaceCoefficient {
        /// `u` or `v` face array.
        axis: &'static str,
        /// Row-major face index.
        index: usize,
        /// Inverse density or assembled stencil weight.
        coefficient: &'static str,
    },
    /// The sum of incident face weights overflowed at a cell.
    InvalidDiagonal {
        /// Row-major cell index.
        cell: usize,
    },
    /// An input or output vector does not match the cell layout.
    Length {
        /// Name of the rejected vector.
        field: &'static str,
        /// Required number of entries.
        expected: usize,
        /// Supplied number of entries.
        actual: usize,
    },
    /// An input vector contains NaN or infinity.
    NonFiniteInput {
        /// Name of the rejected vector.
        field: &'static str,
        /// Row-major cell index.
        cell: usize,
    },
    /// A finite input produced an unrepresentable matrix-vector result.
    NonFiniteApply {
        /// Row-major cell index at the first failing edge.
        cell: usize,
    },
    /// A component reduction overflowed despite finite RHS entries.
    NonFiniteRhsReduction {
        /// Deterministic component number, ordered by first row-major cell.
        component: usize,
    },
    /// A sealed component requests net volume creation or removal.
    IncompatibleRhs {
        /// Deterministic component number, ordered by first row-major cell.
        component: usize,
        /// Sum of RHS values over that component, in s⁻².
        sum: f64,
        /// Summation-roundoff allowance, in s⁻².
        tolerance: f64,
    },
}

impl std::fmt::Display for PressureAssemblyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PressureAssemblyError {}

/// The closed-boundary matrix `A = -D[(1/rho_face)G]` in a MAC layout.
///
/// `D` includes the face aperture once. An interior edge therefore has weight
/// `aperture / (rho_face * grid.cell_width_m()²)`, and the two neighboring rows receive
/// equal and opposite edge contributions. Positive weights make `A` symmetric
/// positive semidefinite, with one constant-pressure null mode per connected
/// component. `A * pressure_pa` has units s⁻². Density is an input coefficient;
/// assembly does not own or advance transported mass.
#[derive(Debug)]
pub struct PressureAssembly {
    grid: Grid,
    inverse_face_density_m3_kg: FaceValues,
    weights_m_kg: FaceValues,
    diagonal_m_kg: Vec<f64>,
    component_of_cell: Vec<usize>,
    component_sizes: Vec<usize>,
}

impl PressureAssembly {
    /// Assembles only closed outer boundaries; open faces need a reservoir contract.
    ///
    /// The face density is the arithmetic mean of its two adjacent positive cell
    /// densities. The mean is evaluated without summing the cells first, avoiding
    /// overflow for two large, finite densities. Unrepresentable inverse density,
    /// edge weight, or diagonal is rejected instead of silently disconnecting a
    /// positive-aperture face.
    pub fn new(fields: &PressureFields) -> Result<Self, PressureAssemblyError> {
        for (side, boundary) in ["left", "right", "top", "bottom"]
            .into_iter()
            .zip(fields.boundaries())
        {
            if matches!(boundary, Boundary::Open { .. }) {
                return Err(PressureAssemblyError::OpenBoundaryRequiresReservoir { side });
            }
        }

        let grid = fields.grid();
        let width = grid.width() as usize;
        let height = grid.height() as usize;
        let cell_width_m = grid.cell_width_m();
        let density = fields.density_kg_m3();
        let aperture = fields.aperture();
        let mut inverse_face_density_m3_kg = FaceValues {
            u: vec![0.0; grid.u_faces()],
            v: vec![0.0; grid.v_faces()],
        };
        let mut weights_m_kg = FaceValues {
            u: vec![0.0; grid.u_faces()],
            v: vec![0.0; grid.v_faces()],
        };
        let mut diagonal_m_kg = vec![0.0; grid.cells()];

        for y in 0..height {
            for x in 1..width {
                let face = y * (width + 1) + x;
                let left = y * width + x - 1;
                let right = left + 1;
                if aperture.u[face] > 0.0 {
                    let (beta, weight) = face_coefficients(
                        "u",
                        face,
                        aperture.u[face],
                        density[left],
                        density[right],
                        cell_width_m,
                    )?;
                    inverse_face_density_m3_kg.u[face] = beta;
                    weights_m_kg.u[face] = weight;
                    add_diagonal(&mut diagonal_m_kg, left, weight)?;
                    add_diagonal(&mut diagonal_m_kg, right, weight)?;
                }
            }
        }
        for y in 1..height {
            for x in 0..width {
                let face = y * width + x;
                let top = (y - 1) * width + x;
                let bottom = top + width;
                if aperture.v[face] > 0.0 {
                    let (beta, weight) = face_coefficients(
                        "v",
                        face,
                        aperture.v[face],
                        density[top],
                        density[bottom],
                        cell_width_m,
                    )?;
                    inverse_face_density_m3_kg.v[face] = beta;
                    weights_m_kg.v[face] = weight;
                    add_diagonal(&mut diagonal_m_kg, top, weight)?;
                    add_diagonal(&mut diagonal_m_kg, bottom, weight)?;
                }
            }
        }

        let (component_of_cell, component_sizes) = components(grid, &weights_m_kg);
        Ok(Self {
            grid,
            inverse_face_density_m3_kg,
            weights_m_kg,
            diagonal_m_kg,
            component_of_cell,
            component_sizes,
        })
    }

    /// Geometry for which these face coefficients and components were built.
    pub fn grid(&self) -> Grid {
        self.grid
    }

    /// Inverse adjacent-cell arithmetic-mean density, in m³/kg; zero on blocked and outer faces.
    pub fn inverse_face_density_m3_kg(&self) -> &FaceValues {
        &self.inverse_face_density_m3_kg
    }

    /// Matrix edge weights in m/kg; zero on blocked and outer faces.
    pub fn weights_m_kg(&self) -> &FaceValues {
        &self.weights_m_kg
    }

    /// Matrix diagonal in m/kg. An isolated cell has a zero diagonal.
    pub fn diagonal_m_kg(&self) -> &[f64] {
        &self.diagonal_m_kg
    }

    /// Component number for each row-major cell; numbers follow first-cell order.
    pub fn component_of_cell(&self) -> &[usize] {
        &self.component_of_cell
    }

    /// Number of cells in each sealed component.
    pub fn component_sizes(&self) -> &[usize] {
        &self.component_sizes
    }

    /// Applies `A` without storing a sparse matrix or allocating a cell vector.
    ///
    /// The output is fully overwritten. Input validation leaves it untouched on
    /// failure; arithmetic overflow clears it to zero before returning an error.
    pub fn apply_into(
        &self,
        pressure_pa: &[f64],
        output_per_s2: &mut [f64],
    ) -> Result<(), PressureAssemblyError> {
        self.validate_cell_vector("pressure_pa", pressure_pa)?;
        self.validate_cell_length("output_per_s2", output_per_s2.len())?;
        output_per_s2.fill(0.0);

        let width = self.grid.width() as usize;
        let height = self.grid.height() as usize;
        for y in 0..height {
            for x in 1..width {
                let face = y * (width + 1) + x;
                let weight = self.weights_m_kg.u[face];
                if weight > 0.0 {
                    let left = y * width + x - 1;
                    let right = left + 1;
                    if !accumulate_edge(output_per_s2, pressure_pa, left, right, weight) {
                        output_per_s2.fill(0.0);
                        return Err(PressureAssemblyError::NonFiniteApply { cell: left });
                    }
                }
            }
        }
        for y in 1..height {
            for x in 0..width {
                let face = y * width + x;
                let weight = self.weights_m_kg.v[face];
                if weight > 0.0 {
                    let top = (y - 1) * width + x;
                    let bottom = top + width;
                    if !accumulate_edge(output_per_s2, pressure_pa, top, bottom, weight) {
                        output_per_s2.fill(0.0);
                        return Err(PressureAssemblyError::NonFiniteApply { cell: top });
                    }
                }
            }
        }
        Ok(())
    }

    /// Allocating convenience form of [`Self::apply_into`].
    pub fn apply(&self, pressure_pa: &[f64]) -> Result<Vec<f64>, PressureAssemblyError> {
        let mut output = vec![0.0; self.grid.cells()];
        self.apply_into(pressure_pa, &mut output)?;
        Ok(output)
    }

    /// Rejects a net RHS over any sealed component before a pressure solve.
    ///
    /// A compensated sum and a tolerance of 32 machine epsilons times the
    /// component L1 norm admit summation roundoff, not a physical volume source.
    /// Components and reductions are visited in row-major cell order.
    pub fn check_compatible_rhs(&self, rhs_per_s2: &[f64]) -> Result<(), PressureAssemblyError> {
        self.validate_cell_vector("rhs_per_s2", rhs_per_s2)?;
        let count = self.component_sizes.len();
        let mut sums = vec![0.0; count];
        let mut compensation = vec![0.0; count];
        let mut l1 = vec![0.0; count];

        for (cell, &value) in rhs_per_s2.iter().enumerate() {
            let component = self.component_of_cell[cell];
            let next = sums[component] + value;
            if sums[component].abs() >= value.abs() {
                compensation[component] += (sums[component] - next) + value;
            } else {
                compensation[component] += (value - next) + sums[component];
            }
            sums[component] = next;
            l1[component] += value.abs();
            if !sums[component].is_finite()
                || !compensation[component].is_finite()
                || !l1[component].is_finite()
            {
                return Err(PressureAssemblyError::NonFiniteRhsReduction { component });
            }
        }

        for component in 0..count {
            let sum = sums[component] + compensation[component];
            if !sum.is_finite() {
                return Err(PressureAssemblyError::NonFiniteRhsReduction { component });
            }
            let tolerance = 32.0 * f64::EPSILON * l1[component];
            if sum.abs() > tolerance {
                return Err(PressureAssemblyError::IncompatibleRhs {
                    component,
                    sum,
                    tolerance,
                });
            }
        }
        Ok(())
    }

    fn validate_cell_vector(
        &self,
        field: &'static str,
        values: &[f64],
    ) -> Result<(), PressureAssemblyError> {
        self.validate_cell_length(field, values.len())?;
        for (cell, value) in values.iter().enumerate() {
            if !value.is_finite() {
                return Err(PressureAssemblyError::NonFiniteInput { field, cell });
            }
        }
        Ok(())
    }

    fn validate_cell_length(
        &self,
        field: &'static str,
        actual: usize,
    ) -> Result<(), PressureAssemblyError> {
        let expected = self.grid.cells();
        if actual != expected {
            return Err(PressureAssemblyError::Length {
                field,
                expected,
                actual,
            });
        }
        Ok(())
    }
}

fn face_coefficients(
    axis: &'static str,
    index: usize,
    aperture: f64,
    first_density: f64,
    second_density: f64,
    cell_width_m: f64,
) -> Result<(f64, f64), PressureAssemblyError> {
    let face_density = first_density + 0.5 * (second_density - first_density);
    let beta = 1.0 / face_density;
    if !beta.is_finite() || beta <= 0.0 {
        return Err(PressureAssemblyError::InvalidFaceCoefficient {
            axis,
            index,
            coefficient: "inverse_face_density_m3_kg",
        });
    }
    let weight = (aperture / (cell_width_m * cell_width_m)) * beta;
    if !weight.is_finite() || weight <= 0.0 {
        return Err(PressureAssemblyError::InvalidFaceCoefficient {
            axis,
            index,
            coefficient: "weight_m_kg",
        });
    }
    Ok((beta, weight))
}

fn add_diagonal(
    diagonal: &mut [f64],
    cell: usize,
    weight: f64,
) -> Result<(), PressureAssemblyError> {
    diagonal[cell] += weight;
    if !diagonal[cell].is_finite() {
        return Err(PressureAssemblyError::InvalidDiagonal { cell });
    }
    Ok(())
}

fn accumulate_edge(output: &mut [f64], pressure: &[f64], a: usize, b: usize, weight: f64) -> bool {
    let flux = weight * (pressure[a] - pressure[b]);
    let next_a = output[a] + flux;
    let next_b = output[b] - flux;
    if !flux.is_finite() || !next_a.is_finite() || !next_b.is_finite() {
        return false;
    }
    output[a] = next_a;
    output[b] = next_b;
    true
}

fn components(grid: Grid, weights: &FaceValues) -> (Vec<usize>, Vec<usize>) {
    let width = grid.width() as usize;
    let height = grid.height() as usize;
    let mut component_of_cell = vec![usize::MAX; grid.cells()];
    let mut component_sizes = Vec::new();
    let mut stack = Vec::new();

    for seed in 0..grid.cells() {
        if component_of_cell[seed] != usize::MAX {
            continue;
        }
        let component = component_sizes.len();
        let mut size = 0;
        component_of_cell[seed] = component;
        stack.push(seed);
        while let Some(cell) = stack.pop() {
            size += 1;
            let x = cell % width;
            let y = cell / width;
            let mut visit = |neighbor: usize, weight: f64| {
                if weight > 0.0 && component_of_cell[neighbor] == usize::MAX {
                    component_of_cell[neighbor] = component;
                    stack.push(neighbor);
                }
            };
            if x > 0 {
                visit(cell - 1, weights.u[y * (width + 1) + x]);
            }
            if x + 1 < width {
                visit(cell + 1, weights.u[y * (width + 1) + x + 1]);
            }
            if y > 0 {
                visit(cell - width, weights.v[y * width + x]);
            }
            if y + 1 < height {
                visit(cell + width, weights.v[(y + 1) * width + x]);
            }
        }
        component_sizes.push(size);
    }
    (component_of_cell, component_sizes)
}
