use super::{WaterError, if97};

/// Version of the equation source, SI projection and interpolation layout.
pub const WATER_PROPERTY_VERSION: &str = "iapws-if97-r7-97-2012-saturation-v1";
/// Highest temperature covered by the region-1/2 saturation projection.
pub const MAX_TEMPERATURE_K: f64 = 623.15;
/// Lower saturation pressure of the initial water reference, in Pa.
pub const MIN_PRESSURE_PA: f64 = 10000.0;

/// Saturated liquid and vapor at one temperature. Quantities are specific to mass.
#[derive(Clone, Copy, Debug)]
pub struct SaturationProperties {
    /// Equilibrium temperature, in K.
    pub temperature_k: f64,
    /// Equilibrium pressure, in Pa.
    pub pressure_pa: f64,
    /// Liquid specific volume, in m3/kg.
    pub liquid_volume_m3_kg: f64,
    /// Vapor specific volume, in m3/kg.
    pub vapor_volume_m3_kg: f64,
    /// Liquid specific internal energy, in J/kg, with the IF97 reference zero.
    pub liquid_energy_j_kg: f64,
    /// Vapor specific internal energy, in J/kg, with the same reference zero.
    pub vapor_energy_j_kg: f64,
}

impl SaturationProperties {
    /// Liquid enthalpy, in J/kg. This preserves h = u + pv after interpolation.
    pub fn liquid_enthalpy_j_kg(self) -> f64 {
        self.liquid_energy_j_kg + self.pressure_pa * self.liquid_volume_m3_kg
    }

    /// Vapor enthalpy, in J/kg. This preserves h = u + pv after interpolation.
    pub fn vapor_enthalpy_j_kg(self) -> f64 {
        self.vapor_energy_j_kg + self.pressure_pa * self.vapor_volume_m3_kg
    }
}

/// Immutable bounded projection generated from IF97 regions 1, 2 and 4.
///
/// The 2441 nodes cover saturation from 10 kPa to 623.15 K (about 16.529 MPa).
/// The spacing is less than 0.125 K. This table excludes compressed liquid,
/// superheated vapor, ice, carrier air and temperatures outside that interval.
/// It does not read or reinterpret legacy parcel enthalpy.
#[derive(Debug)]
pub struct SaturationTable {
    nodes: Vec<SaturationProperties>,
}

impl Default for SaturationTable {
    fn default() -> Self {
        Self::new()
    }
}

impl SaturationTable {
    /// Generates the fixed projection without external dependencies or downloads.
    pub fn new() -> Self {
        Self::generate(2440)
    }

    fn generate(intervals: usize) -> Self {
        let mut lower = 273.15;
        let mut upper = MAX_TEMPERATURE_K;
        for _ in 0..64 {
            let t = (lower + upper) * 0.5;
            if if97::saturation_pressure(t) < MIN_PRESSURE_PA {
                lower = t;
            } else {
                upper = t;
            }
        }
        let min_k = upper;
        let nodes = (0..=intervals)
            .map(|i| {
                let temperature_k = if i == intervals {
                    MAX_TEMPERATURE_K
                } else {
                    min_k + (MAX_TEMPERATURE_K - min_k) * i as f64 / intervals as f64
                };
                source_properties(temperature_k)
            })
            .collect();
        Self { nodes }
    }

    /// Lowest supported saturation temperature, in K (about 318.958 K).
    pub fn min_temperature_k(&self) -> f64 {
        self.nodes[0].temperature_k
    }

    /// Returns linearly interpolated properties. Unsupported temperatures reject.
    pub fn sample(&self, temperature_k: f64) -> Result<SaturationProperties, WaterError> {
        if !temperature_k.is_finite() {
            return Err(WaterError::NonFiniteInput);
        }
        if !(self.min_temperature_k()..=MAX_TEMPERATURE_K).contains(&temperature_k) {
            return Err(WaterError::TemperatureOutsideTable);
        }
        let next = self
            .nodes
            .partition_point(|node| node.temperature_k < temperature_k);
        if next == 0 {
            return Ok(self.nodes[0]);
        }
        let a = self.nodes[next - 1];
        let b = self.nodes[next];
        let weight = (temperature_k - a.temperature_k) / (b.temperature_k - a.temperature_k);
        let mix = |x, y| x + weight * (y - x);
        Ok(SaturationProperties {
            temperature_k,
            pressure_pa: mix(a.pressure_pa, b.pressure_pa),
            liquid_volume_m3_kg: mix(a.liquid_volume_m3_kg, b.liquid_volume_m3_kg),
            vapor_volume_m3_kg: mix(a.vapor_volume_m3_kg, b.vapor_volume_m3_kg),
            liquid_energy_j_kg: mix(a.liquid_energy_j_kg, b.liquid_energy_j_kg),
            vapor_energy_j_kg: mix(a.vapor_energy_j_kg, b.vapor_energy_j_kg),
        })
    }
}

fn source_properties(temperature_k: f64) -> SaturationProperties {
    let pressure_pa = if97::saturation_pressure(temperature_k);
    let liquid = if97::liquid(temperature_k, pressure_pa);
    let vapor = if97::vapor(temperature_k, pressure_pa);
    SaturationProperties {
        temperature_k,
        pressure_pa,
        liquid_volume_m3_kg: liquid.volume_m3_kg,
        vapor_volume_m3_kg: vapor.volume_m3_kg,
        liquid_energy_j_kg: liquid.energy_j_kg,
        vapor_energy_j_kg: vapor.energy_j_kg,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn maximum_error(table: &SaturationTable) -> [f64; 5] {
        let mut maxima = [0.0_f64; 5];
        // Quarter, midpoint and three-quarter positions cover every interval.
        for pair in table.nodes.windows(2) {
            for fraction in [0.25, 0.5, 0.75] {
                let t = pair[0].temperature_k
                    + fraction * (pair[1].temperature_k - pair[0].temperature_k);
                let interpolated = table.sample(t).unwrap();
                let source = source_properties(t);
                for (i, (a, b)) in [
                    (interpolated.pressure_pa, source.pressure_pa),
                    (interpolated.liquid_volume_m3_kg, source.liquid_volume_m3_kg),
                    (interpolated.vapor_volume_m3_kg, source.vapor_volume_m3_kg),
                    (interpolated.liquid_energy_j_kg, source.liquid_energy_j_kg),
                    (interpolated.vapor_energy_j_kg, source.vapor_energy_j_kg),
                ]
                .into_iter()
                .enumerate()
                {
                    maxima[i] = maxima[i].max((a / b - 1.0).abs());
                }
            }
        }
        maxima
    }

    #[test]
    fn interpolation_error_and_refinement() {
        let coarse = maximum_error(&SaturationTable::generate(610));
        let medium = maximum_error(&SaturationTable::generate(1220));
        let fine = maximum_error(&SaturationTable::new());
        eprintln!(
            "water property relative errors: coarse={coarse:?} medium={medium:?} fine={fine:?}"
        );
        // Independent equation evaluations bound interpolation separately from closure.
        for (i, tolerance) in [5e-6, 2e-6, 1e-5, 2e-6, 2e-6].into_iter().enumerate() {
            assert!(
                fine[i] < tolerance,
                "property {i}: {} >= {tolerance}",
                fine[i]
            );
            assert!(medium[i] < coarse[i] * 0.3);
            assert!(fine[i] < medium[i] * 0.3);
        }
    }

    #[test]
    fn generated_nodes_keep_monotone_volume_bounds_and_positive_latent_energy() {
        let table = SaturationTable::new();
        for pair in table.nodes.windows(2) {
            assert!(pair[1].pressure_pa > pair[0].pressure_pa);
            assert!(pair[1].liquid_volume_m3_kg > pair[0].liquid_volume_m3_kg);
            assert!(pair[1].vapor_volume_m3_kg < pair[0].vapor_volume_m3_kg);
        }
        for node in table.nodes {
            assert!(node.liquid_volume_m3_kg > 0.0);
            assert!(node.vapor_volume_m3_kg > node.liquid_volume_m3_kg);
            assert!(node.vapor_energy_j_kg > node.liquid_energy_j_kg);
        }
    }
}
