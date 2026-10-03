use super::{
    MAX_TEMPERATURE_K, MIN_PRESSURE_PA, SaturationTable, WaterEquilibrium, WaterError,
    WaterInventory, if97,
};

/// Lowest temperature supported by the liquid projection, in K.
pub const MIN_TEMPERATURE_K: f64 = 273.15;
/// Highest supported pressure, in Pa. Region 3 is not used below 623.15 K.
pub const MAX_PRESSURE_PA: f64 = 20e6;
/// IF97 equation source and single-phase interpolation layout.
pub const SINGLE_PHASE_PROPERTY_VERSION: &str = "iapws-if97-r7-97-2012-single-phase-v1";

/// Stable single-phase water branch. Saturated endpoints belong to both closures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaterPhase {
    /// Saturated or compressed liquid with finite compressibility.
    Liquid,
    /// Saturated or superheated vapor.
    Vapor,
}

/// Derived single-phase properties in SI units, using the IF97 energy zero.
#[derive(Clone, Copy, Debug)]
pub struct WaterProperties {
    /// Temperature, in K.
    pub temperature_k: f64,
    /// Thermodynamic pressure, in Pa.
    pub pressure_pa: f64,
    /// Specific volume, in m3/kg.
    pub volume_m3_kg: f64,
    /// Specific internal energy, in J/kg.
    pub energy_j_kg: f64,
}

#[derive(Clone, Copy, Debug)]
struct Node {
    volume: f64,
    energy: f64,
}

#[derive(Debug)]
struct Row {
    temperature: f64,
    lower_pressure: f64,
    upper_pressure: f64,
    nodes: Vec<Node>,
}

#[derive(Debug)]
struct PhaseTable {
    phase: WaterPhase,
    rows: Vec<Row>,
}

/// One property owner for saturated, compressed-liquid and superheated-vapor water.
/// Generation uses only the existing IF97 equations. Queries do not evaluate them.
#[derive(Debug)]
pub struct WaterTable {
    saturation: SaturationTable,
    liquid: PhaseTable,
    vapor: PhaseTable,
}

impl Default for WaterTable {
    fn default() -> Self {
        Self::new()
    }
}

impl WaterTable {
    /// Generates immutable projections with 65 liquid and 513 vapor pressure nodes per row.
    pub fn new() -> Self {
        let saturation = SaturationTable::new();
        let liquid = PhaseTable::generate(WaterPhase::Liquid, &saturation, 64);
        let vapor = PhaseTable::generate(WaterPhase::Vapor, &saturation, 512);
        Self {
            saturation,
            liquid,
            vapor,
        }
    }

    /// Existing two-phase projection. Its saturation domain remains unchanged.
    pub fn saturation(&self) -> &SaturationTable {
        &self.saturation
    }

    /// Looks up a stable single-phase state. Metastable and out-of-domain queries reject.
    pub fn sample_single_phase(
        &self,
        phase: WaterPhase,
        temperature_k: f64,
        pressure_pa: f64,
    ) -> Result<WaterProperties, WaterError> {
        if !temperature_k.is_finite() || !pressure_pa.is_finite() {
            return Err(WaterError::NonFiniteInput);
        }
        if !(MIN_PRESSURE_PA..=MAX_PRESSURE_PA).contains(&pressure_pa) {
            return Err(WaterError::PressureOutsideTable);
        }
        let table = match phase {
            WaterPhase::Liquid => &self.liquid,
            WaterPhase::Vapor => &self.vapor,
        };
        let (row, t) = table.interval(temperature_k)?;
        let (lower, upper) = table.pressures(row, t);
        if pressure_pa < lower || pressure_pa > upper {
            return Err(WaterError::UnsupportedEquilibrium);
        }
        let q = if upper == lower {
            0.0
        } else {
            match phase {
                WaterPhase::Liquid => (pressure_pa - lower) / (upper - lower),
                WaterPhase::Vapor => {
                    let log_fraction = (pressure_pa / lower).ln() / (upper / lower).ln();
                    1.0 - (1.0 - log_fraction).max(0.0).sqrt()
                }
            }
        };
        let node = table.sample(row, t, q);
        Ok(WaterProperties {
            temperature_k,
            pressure_pa,
            volume_m3_kg: node.volume,
            energy_j_kg: node.energy,
        })
    }

    pub(super) fn close_single_phase(
        &self,
        inventory: WaterInventory,
    ) -> Result<WaterEquilibrium, WaterError> {
        let (u, v) = inventory.specific()?;
        for table in [&self.liquid, &self.vapor] {
            if let Some(state) = table.close(inventory, u, v)? {
                return Ok(state);
            }
        }
        Err(WaterError::UnsupportedEquilibrium)
    }

    /// Inverts only the existing vapor projection at a supplied temperature.
    /// Mixture closure uses this pressure as water's partial pressure.
    pub(super) fn vapor_at_volume(
        &self,
        temperature_k: f64,
        volume_m3_kg: f64,
    ) -> Result<WaterProperties, WaterError> {
        if !temperature_k.is_finite() || !volume_m3_kg.is_finite() {
            return Err(WaterError::NonFiniteInput);
        }
        let (row, t) = self.vapor.interval(temperature_k)?;
        let last = self.vapor.rows[row].nodes.len() - 1;
        let minimum = self.vapor.node(row, t, last).volume;
        let maximum = self.vapor.node(row, t, 0).volume;
        let tolerance = 2e-13 * volume_m3_kg;
        if volume_m3_kg <= 0.0
            || volume_m3_kg < minimum - tolerance
            || volume_m3_kg > maximum + tolerance
        {
            return Err(WaterError::UnsupportedEquilibrium);
        }
        let (node, q) = self.vapor.at_volume(row, t, volume_m3_kg);
        if (node.volume - volume_m3_kg).abs() > tolerance {
            return Err(WaterError::UnsupportedEquilibrium);
        }
        let (lower, upper) = self.vapor.pressures(row, t);
        Ok(WaterProperties {
            temperature_k,
            pressure_pa: lower * (upper / lower).powf(q * (2.0 - q)),
            volume_m3_kg: node.volume,
            energy_j_kg: node.energy,
        })
    }
}

fn mix(a: f64, b: f64, t: f64) -> f64 {
    a + t * (b - a)
}

impl PhaseTable {
    fn generate(phase: WaterPhase, saturation: &SaturationTable, intervals: usize) -> Self {
        let min_saturation = saturation.min_temperature_k();
        let mut temperatures = Vec::new();
        if phase == WaterPhase::Liquid {
            let count = ((min_saturation - MIN_TEMPERATURE_K)
                / ((MAX_TEMPERATURE_K - min_saturation) / (saturation.nodes.len() - 1) as f64))
                .ceil() as usize;
            temperatures.extend((0..count).map(|i| {
                MIN_TEMPERATURE_K + (min_saturation - MIN_TEMPERATURE_K) * i as f64 / count as f64
            }));
        }
        // Match saturation nodes so both projections share the same phase boundary.
        temperatures.extend(saturation.nodes.iter().map(|s| s.temperature_k));
        let rows = temperatures
            .into_iter()
            .map(|temperature| {
                let sat = saturation.sample(temperature).ok();
                let sat_pressure = sat.map_or(MIN_PRESSURE_PA, |s| s.pressure_pa);
                let (lower_pressure, upper_pressure) = match phase {
                    WaterPhase::Liquid => (sat_pressure, MAX_PRESSURE_PA),
                    WaterPhase::Vapor => (MIN_PRESSURE_PA, sat_pressure),
                };
                let nodes = (0..=intervals)
                    .map(|i| {
                        let q = i as f64 / intervals as f64;
                        let pressure = match phase {
                            WaterPhase::Liquid => mix(lower_pressure, upper_pressure, q),
                            WaterPhase::Vapor => {
                                lower_pressure
                                    * (upper_pressure / lower_pressure).powf(q * (2.0 - q))
                            }
                        };
                        let source = match phase {
                            WaterPhase::Liquid => if97::liquid(temperature, pressure),
                            WaterPhase::Vapor => if97::vapor(temperature, pressure),
                        };
                        Node {
                            volume: source.volume_m3_kg,
                            energy: source.energy_j_kg,
                        }
                    })
                    .collect();
                Row {
                    temperature,
                    lower_pressure,
                    upper_pressure,
                    nodes,
                }
            })
            .collect();
        Self { phase, rows }
    }

    fn interval(&self, temperature: f64) -> Result<(usize, f64), WaterError> {
        if !(self.rows[0].temperature..=MAX_TEMPERATURE_K).contains(&temperature) {
            return Err(WaterError::TemperatureOutsideTable);
        }
        let next = self
            .rows
            .partition_point(|r| r.temperature < temperature)
            .max(1);
        let row = next - 1;
        let t = (temperature - self.rows[row].temperature)
            / (self.rows[row + 1].temperature - self.rows[row].temperature);
        Ok((row, t))
    }

    fn pressures(&self, row: usize, t: f64) -> (f64, f64) {
        let a = &self.rows[row];
        let b = &self.rows[row + 1];
        (
            mix(a.lower_pressure, b.lower_pressure, t),
            mix(a.upper_pressure, b.upper_pressure, t),
        )
    }

    fn node(&self, row: usize, t: f64, index: usize) -> Node {
        let a = self.rows[row].nodes[index];
        let b = self.rows[row + 1].nodes[index];
        Node {
            volume: mix(a.volume, b.volume, t),
            energy: mix(a.energy, b.energy, t),
        }
    }

    fn sample(&self, row: usize, t: f64, q: f64) -> Node {
        let intervals = self.rows[row].nodes.len() - 1;
        let coordinate = q * intervals as f64;
        let index = (coordinate.floor() as usize).min(intervals - 1);
        let a = self.node(row, t, index);
        let b = self.node(row, t, index + 1);
        let w = coordinate - index as f64;
        Node {
            volume: mix(a.volume, b.volume, w),
            energy: mix(a.energy, b.energy, w),
        }
    }

    fn at_volume(&self, row: usize, t: f64, v: f64) -> (Node, f64) {
        let last = self.rows[row].nodes.len() - 1;
        let mut low = 0;
        let mut high = last;
        // Every pressure row has decreasing specific volume. At the vapor floor
        // the row collapses to one saturated point; all q values are equivalent.
        while high - low > 1 {
            let middle = (low + high) / 2;
            if self.node(row, t, middle).volume > v {
                low = middle;
            } else {
                high = middle;
            }
        }
        let a = self.node(row, t, low);
        let b = self.node(row, t, high);
        let w = if a.volume == b.volume {
            0.0
        } else {
            ((v - a.volume) / (b.volume - a.volume)).clamp(0.0, 1.0)
        };
        let q = (low as f64 + w) / last as f64;
        (self.sample(row, t, q), q)
    }

    fn close(
        &self,
        inventory: WaterInventory,
        u: f64,
        v: f64,
    ) -> Result<Option<WaterEquilibrium>, WaterError> {
        let energy_tolerance = 1e-11 * u.abs().max(1000.0);
        for row in 0..self.rows.len() - 1 {
            let last = self.rows[row].nodes.len() - 1;
            let mut lower: f64 = 0.0;
            let mut upper: f64 = 1.0;
            // Clip against BOTH volume boundaries in each temperature interval.
            // Liquid density has a maximum near 277 K, so a global monotone
            // temperature bracket would incorrectly discard valid cold water.
            for (index, maximum) in [(0, true), (last, false)] {
                let a = self.rows[row].nodes[index].volume;
                let b = self.rows[row + 1].nodes[index].volume;
                let (fa, fb) = if maximum {
                    (a - v, b - v)
                } else {
                    (v - a, v - b)
                };
                if fa < 0.0 && fb < 0.0 {
                    lower = 1.0;
                    upper = 0.0;
                    break;
                }
                if fa < 0.0 {
                    lower = lower.max(-fa / (fb - fa));
                }
                if fb < 0.0 {
                    upper = upper.min(fa / (fa - fb));
                }
            }
            if lower > upper {
                continue;
            }
            let (a, _) = self.at_volume(row, lower, v);
            let (b, _) = self.at_volume(row, upper, v);
            if u < a.energy - energy_tolerance || u > b.energy + energy_tolerance {
                continue;
            }
            for iterations in 0..64 {
                let t = if (a.energy - u).abs() <= energy_tolerance {
                    lower
                } else if (b.energy - u).abs() <= energy_tolerance {
                    upper
                } else {
                    (lower + upper) * 0.5
                };
                let (node, q) = self.at_volume(row, t, v);
                let residual = node.energy - u;
                if residual.abs() <= energy_tolerance {
                    if (node.volume - v).abs() > 2e-13 * v {
                        return Err(WaterError::UnsupportedEquilibrium);
                    }
                    let (pmin, pmax) = self.pressures(row, t);
                    let pressure_pa = match self.phase {
                        WaterPhase::Liquid => mix(pmin, pmax, q),
                        WaterPhase::Vapor => pmin * (pmax / pmin).powf(q * (2.0 - q)),
                    };
                    let vapor = self.phase == WaterPhase::Vapor;
                    return Ok(Some(WaterEquilibrium {
                        temperature_k: mix(
                            self.rows[row].temperature,
                            self.rows[row + 1].temperature,
                            t,
                        ),
                        pressure_pa,
                        vapor_mass_fraction: if vapor { 1.0 } else { 0.0 },
                        liquid_mass_kg: if vapor { 0.0 } else { inventory.mass_kg },
                        vapor_mass_kg: if vapor { inventory.mass_kg } else { 0.0 },
                        liquid_volume_m3: if vapor {
                            0.0
                        } else {
                            inventory.mass_kg * node.volume
                        },
                        vapor_volume_m3: if vapor {
                            inventory.mass_kg * node.volume
                        } else {
                            0.0
                        },
                        energy_residual_j: residual * inventory.mass_kg,
                        iterations,
                    }));
                }
                if residual < 0.0 {
                    lower = t;
                } else {
                    upper = t;
                }
            }
            return Err(WaterError::IterationLimit);
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn maximum_error(table: &PhaseTable) -> [f64; 2] {
        let mut error = [0.0_f64; 2];
        let intervals = table.rows[0].nodes.len() - 1;
        // Stratify the complete temperature/pressure domain. Compare pressure-cell
        // interiors and the upper pressure endpoint against the IF97 source equations.
        for row in (0..table.rows.len() - 1).step_by(16) {
            for t in [0.25, 0.5, 0.75] {
                let temperature = mix(
                    table.rows[row].temperature,
                    table.rows[row + 1].temperature,
                    t,
                );
                let (lower, upper) = table.pressures(row, t);
                for column in 0..=intervals {
                    let q = (column as f64 + 0.5).min(intervals as f64) / intervals as f64;
                    let p = match table.phase {
                        WaterPhase::Liquid => mix(lower, upper, q),
                        WaterPhase::Vapor => lower * (upper / lower).powf(q * (2.0 - q)),
                    };
                    let source = match table.phase {
                        WaterPhase::Liquid => if97::liquid(temperature, p),
                        WaterPhase::Vapor => if97::vapor(temperature, p),
                    };
                    let sample = table.sample(row, t, q);
                    error[0] = error[0].max((sample.volume / source.volume_m3_kg - 1.0).abs());
                    error[1] = error[1].max((sample.energy - source.energy_j_kg).abs());
                }
            }
        }
        error
    }

    #[test]
    fn single_phase_projection_errors_decrease_with_refinement() {
        let coarse_sat = SaturationTable::generate(610);
        let medium_sat = SaturationTable::generate(1220);
        let fine = WaterTable::new();
        for (phase, intervals, table) in [
            (WaterPhase::Liquid, 16, &fine.liquid),
            (WaterPhase::Vapor, 128, &fine.vapor),
        ] {
            let coarse = maximum_error(&PhaseTable::generate(phase, &coarse_sat, intervals));
            let medium = maximum_error(&PhaseTable::generate(phase, &medium_sat, intervals * 2));
            let error = maximum_error(table);
            println!(
                "{phase:?} interpolation errors: coarse={coarse:?}, medium={medium:?}, fine={error:?}"
            );
            assert!(error[0] < 2e-4);
            assert!(error[1] < 20.0);
            for i in 0..2 {
                assert!(medium[i] < coarse[i] * 0.3);
                assert!(error[i] < medium[i] * 0.3);
            }
        }
    }

    #[test]
    fn volume_decreases_with_pressure_and_isochoric_energy_increases_in_every_interval() {
        let table = WaterTable::new();
        for phase in [&table.liquid, &table.vapor] {
            let last = phase.rows[0].nodes.len() - 1;
            for row in 0..phase.rows.len() - 1 {
                for column in 0..last {
                    assert!(
                        phase.rows[row].nodes[column].volume
                            >= phase.rows[row].nodes[column + 1].volume
                    );
                }
                // The bilinear EOS has positive isochoric heat capacity. Check
                // local isochoric energy at interior pressure coordinates.
                for q in [0.1, 0.5, 0.9] {
                    let v = phase.sample(row, 0.5, q).volume;
                    let a = phase.at_volume(row, 0.49, v).0;
                    let b = phase.at_volume(row, 0.51, v).0;
                    assert!(b.energy > a.energy, "{:?} row {row}", phase.phase);
                }
            }
        }
    }
}
