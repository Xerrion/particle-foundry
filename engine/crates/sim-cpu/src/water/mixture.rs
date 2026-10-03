use super::{
    MAX_TEMPERATURE_K, MIN_PRESSURE_PA, MIN_TEMPERATURE_K, WaterError, WaterInventory, WaterPhase,
    WaterTable, close_water,
};

/// Fixed carrier-air model coefficient, in J/(kg K).
pub const AIR_GAS_CONSTANT_J_KG_K: f64 = 287.05;
/// Constant-volume heat capacity for the model's fixed gamma of 1.4, in J/(kg K).
pub const AIR_HEAT_CAPACITY_J_KG_K: f64 = AIR_GAS_CONSTANT_J_KG_K / 0.4;
/// Carrier-air internal-energy reference temperature, in K.
pub const AIR_REFERENCE_TEMPERATURE_K: f64 = 273.15;
/// Total-pressure guard for states with positive carrier-air mass, in Pa.
/// This is a development-model limit, not a humid-air accuracy certification.
pub const MAX_MIXTURE_PRESSURE_PA: f64 = 1e6;
/// Ideal carrier air plus the existing bounded IF97 water projections.
pub const MIXTURE_PROPERTY_VERSION: &str = "e09-air-steam-v1";

const ENDPOINT_ROUNDOFF: f64 = 8.0 * f64::EPSILON;

fn pressure_fits(pressure: f64) -> bool {
    pressure <= MAX_MIXTURE_PRESSURE_PA * (1.0 + ENDPOINT_ROUNDOFF)
}

/// One isolated rigid vessel. Phase masses are derived from these inputs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MixtureInventory {
    /// Total water mass, in kg. Zero selects the dry-air component limit.
    pub water_mass_kg: f64,
    /// Inert carrier-air mass, in kg. Zero selects the pure-water component limit.
    pub air_mass_kg: f64,
    /// Conserved internal energy, in J, with IF97 water and the air reference zero.
    pub internal_energy_j: f64,
    /// Actual available volume after excluding solids, in m3.
    pub available_volume_m3: f64,
}

#[derive(Clone, Copy)]
struct SpecificInventory {
    mass: f64,
    water: f64,
    air: f64,
    energy: f64,
    volume: f64,
}

impl MixtureInventory {
    fn specific(self) -> Result<SpecificInventory, WaterError> {
        let values = [
            self.water_mass_kg,
            self.air_mass_kg,
            self.internal_energy_j,
            self.available_volume_m3,
        ];
        if values.iter().any(|value| !value.is_finite()) {
            return Err(WaterError::NonFiniteInput);
        }
        if self.water_mass_kg < 0.0
            || self.air_mass_kg < 0.0
            || self.available_volume_m3 <= 0.0
            || (self.water_mass_kg == 0.0 && self.air_mass_kg == 0.0)
        {
            return Err(WaterError::NonPositiveInventory);
        }
        let mass = self.water_mass_kg + self.air_mass_kg;
        let water = self.water_mass_kg / mass;
        let air = self.air_mass_kg / mass;
        let energy = self.internal_energy_j / mass;
        let volume = self.available_volume_m3 / mass;
        if [mass, water, air, energy, volume]
            .iter()
            .any(|value| !value.is_finite())
            || volume <= 0.0
            || (self.water_mass_kg > 0.0 && water == 0.0)
            || (self.air_mass_kg > 0.0 && air == 0.0)
        {
            return Err(WaterError::NonFiniteInput);
        }
        Ok(SpecificInventory {
            mass,
            water,
            air,
            energy,
            volume,
        })
    }
}

/// Observations of one equilibrium. Air and steam share the same gas volume.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MixtureEquilibrium {
    /// Common equilibrium temperature, in K.
    pub temperature_k: f64,
    /// Total chamber pressure p0, in Pa.
    pub pressure_pa: f64,
    /// Steam partial pressure, in Pa. Zero for pure liquid or dry air.
    pub vapor_partial_pressure_pa: f64,
    /// Inert carrier-air partial pressure, in Pa.
    pub air_partial_pressure_pa: f64,
    /// Derived liquid water mass, in kg.
    pub liquid_water_mass_kg: f64,
    /// Derived water vapor mass, in kg.
    pub vapor_water_mass_kg: f64,
    /// Volume occupied by liquid water, in m3.
    pub liquid_volume_m3: f64,
    /// Shared volume of air and water vapor. Do not add a second air volume.
    pub gas_volume_m3: f64,
    /// Reconstructed water internal energy, in J, using the IF97 reference zero.
    pub water_energy_j: f64,
    /// Reconstructed air internal energy, in J, using the model reference zero.
    pub air_energy_j: f64,
    /// Reconstructed energy minus supplied U, in J.
    pub energy_residual_j: f64,
    /// Reconstructed occupied volume minus supplied volume, in m3.
    pub volume_residual_m3: f64,
    /// Number of energy bisections in the accepted temperature interval.
    pub iterations: u32,
}

#[derive(Clone, Copy)]
struct Candidate {
    temperature: f64,
    vapor_pressure: f64,
    air_pressure: f64,
    liquid_mass: f64,
    vapor_mass: f64,
    liquid_volume: f64,
    gas_volume: f64,
    water_energy: f64,
    air_energy: f64,
}

impl Candidate {
    fn energy(self) -> f64 {
        self.water_energy + self.air_energy
    }

    fn observe(
        self,
        input: MixtureInventory,
        specific: SpecificInventory,
        iterations: u32,
    ) -> Result<MixtureEquilibrium, WaterError> {
        let state = MixtureEquilibrium {
            temperature_k: self.temperature,
            pressure_pa: self.vapor_pressure + self.air_pressure,
            vapor_partial_pressure_pa: self.vapor_pressure,
            air_partial_pressure_pa: self.air_pressure,
            liquid_water_mass_kg: self.liquid_mass * specific.mass,
            vapor_water_mass_kg: self.vapor_mass * specific.mass,
            liquid_volume_m3: self.liquid_volume * specific.mass,
            gas_volume_m3: self.gas_volume * specific.mass,
            water_energy_j: self.water_energy * specific.mass,
            air_energy_j: self.air_energy * specific.mass,
            energy_residual_j: (self.energy() - specific.energy) * specific.mass,
            volume_residual_m3: (self.liquid_volume + self.gas_volume) * specific.mass
                - input.available_volume_m3,
            iterations,
        };
        finite_observation(state)
    }
}

fn finite_observation(state: MixtureEquilibrium) -> Result<MixtureEquilibrium, WaterError> {
    if [
        state.temperature_k,
        state.pressure_pa,
        state.vapor_partial_pressure_pa,
        state.air_partial_pressure_pa,
        state.liquid_water_mass_kg,
        state.vapor_water_mass_kg,
        state.liquid_volume_m3,
        state.gas_volume_m3,
        state.water_energy_j,
        state.air_energy_j,
        state.energy_residual_j,
        state.volume_residual_m3,
    ]
    .iter()
    .any(|value| !value.is_finite())
    {
        Err(WaterError::NonFiniteInput)
    } else {
        Ok(state)
    }
}

fn air_energy(temperature: f64) -> f64 {
    AIR_HEAT_CAPACITY_J_KG_K * (temperature - AIR_REFERENCE_TEMPERATURE_K)
}

fn candidate(
    table: &WaterTable,
    input: SpecificInventory,
    temperature: f64,
) -> Result<Candidate, WaterError> {
    let saturation = table.saturation().sample(temperature)?;
    let air_energy = input.air * air_energy(temperature);
    if input.volume >= input.water * saturation.vapor_volume_m3_kg {
        let vapor = table.vapor_at_volume(temperature, input.volume / input.water)?;
        let air_pressure = input.air * AIR_GAS_CONSTANT_J_KG_K * temperature / input.volume;
        if !pressure_fits(vapor.pressure_pa + air_pressure) {
            return Err(WaterError::UnsupportedEquilibrium);
        }
        return Ok(Candidate {
            temperature,
            vapor_pressure: vapor.pressure_pa,
            air_pressure,
            liquid_mass: 0.0,
            vapor_mass: input.water,
            liquid_volume: 0.0,
            gas_volume: input.volume,
            water_energy: input.water * vapor.energy_j_kg,
            air_energy,
        });
    }
    if saturation.pressure_pa >= MAX_MIXTURE_PRESSURE_PA {
        return Err(WaterError::UnsupportedEquilibrium);
    }
    let air_pv = input.air * AIR_GAS_CONSTANT_J_KG_K * temperature;
    let mut lower = (air_pv / (MAX_MIXTURE_PRESSURE_PA - saturation.pressure_pa)).next_up();
    let mut upper = input.volume;
    if lower <= 0.0 || lower > upper {
        return Err(WaterError::UnsupportedEquilibrium);
    }
    let phases = |gas_volume: f64| {
        let air_pressure = air_pv / gas_volume;
        let liquid = table.sample_single_phase(
            WaterPhase::Liquid,
            temperature,
            saturation.pressure_pa + air_pressure,
        )?;
        let vapor_mass = gas_volume / saturation.vapor_volume_m3_kg;
        let liquid_mass = input.water - vapor_mass;
        let liquid_volume = liquid_mass * liquid.volume_m3_kg;
        Ok::<_, WaterError>((
            liquid_volume + gas_volume - input.volume,
            Candidate {
                temperature,
                vapor_pressure: saturation.pressure_pa,
                air_pressure,
                liquid_mass,
                vapor_mass,
                liquid_volume,
                gas_volume,
                water_energy: liquid_mass * liquid.energy_j_kg
                    + vapor_mass * saturation.vapor_energy_j_kg,
                air_energy,
            },
        ))
    };
    // Occupied volume grows with the shared gas volume in this bounded domain.
    // A positive residual at the pressure guard cannot be repaired by more gas.
    let (lower_residual, lower_state) = phases(lower)?;
    if lower_residual > 2e-13 * input.volume {
        return Err(WaterError::UnsupportedEquilibrium);
    }
    if lower_residual.abs() <= 2e-13 * input.volume {
        if lower_state.liquid_mass < 0.0
            || lower_state.vapor_mass <= 0.0
            || !pressure_fits(lower_state.vapor_pressure + lower_state.air_pressure)
        {
            return Err(WaterError::UnsupportedEquilibrium);
        }
        return Ok(lower_state);
    }
    for _ in 0..64 {
        // Gas may occupy many orders of magnitude less volume than the liquid.
        // Bisect its logarithm so the fixed budget also resolves small carriers
        // near the pressure guard. Use no ratio that can overflow.
        let gas_volume = (lower.ln() + (upper.ln() - lower.ln()) * 0.5).exp();
        let (residual, state) = phases(gas_volume)?;
        if residual.abs() <= 2e-13 * input.volume {
            if state.liquid_mass < 0.0
                || state.vapor_mass <= 0.0
                || !pressure_fits(state.vapor_pressure + state.air_pressure)
            {
                return Err(WaterError::UnsupportedEquilibrium);
            }
            return Ok(state);
        }
        if residual < 0.0 {
            lower = gas_volume;
        } else {
            upper = gas_volume;
        }
    }
    Err(WaterError::IterationLimit)
}

/// Closes bounded air/steam equilibrium without changing conserved inputs.
/// Air is calorically perfect and inert. Liquid uses total pressure; water vapor
/// uses partial pressure. Humidity enhancement and dissolved air are omitted.
/// Positive-air states have a 1 MPa total-pressure guard. Water vapor retains its
/// 10 kPa table floor. The exact zero-air limit retains the pure-water domain.
pub fn close_mixture(
    table: &WaterTable,
    inventory: MixtureInventory,
) -> Result<MixtureEquilibrium, WaterError> {
    let input = inventory.specific()?;
    if inventory.air_mass_kg == 0.0 {
        let water = close_water(
            table,
            WaterInventory {
                mass_kg: inventory.water_mass_kg,
                internal_energy_j: inventory.internal_energy_j,
                available_volume_m3: inventory.available_volume_m3,
            },
        )?;
        return finite_observation(MixtureEquilibrium {
            temperature_k: water.temperature_k,
            pressure_pa: water.pressure_pa,
            vapor_partial_pressure_pa: if water.vapor_mass_kg > 0.0 {
                water.pressure_pa
            } else {
                0.0
            },
            air_partial_pressure_pa: 0.0,
            liquid_water_mass_kg: water.liquid_mass_kg,
            vapor_water_mass_kg: water.vapor_mass_kg,
            liquid_volume_m3: water.liquid_volume_m3,
            gas_volume_m3: water.vapor_volume_m3,
            water_energy_j: inventory.internal_energy_j + water.energy_residual_j,
            air_energy_j: 0.0,
            energy_residual_j: water.energy_residual_j,
            volume_residual_m3: water.liquid_volume_m3 + water.vapor_volume_m3
                - inventory.available_volume_m3,
            iterations: water.iterations,
        });
    }
    if inventory.water_mass_kg == 0.0 {
        let temperature = input.energy / AIR_HEAT_CAPACITY_J_KG_K + AIR_REFERENCE_TEMPERATURE_K;
        let pressure = AIR_GAS_CONSTANT_J_KG_K * temperature / input.volume;
        // Normalize and reconstruct analytic endpoint fixtures without rejecting
        // a few ulps of arithmetic roundoff. Keep U, V and observed T/p unchanged.
        let roundoff = ENDPOINT_ROUNDOFF;
        if !(MIN_TEMPERATURE_K * (1.0 - roundoff)..=MAX_TEMPERATURE_K * (1.0 + roundoff))
            .contains(&temperature)
            || !(MIN_PRESSURE_PA * (1.0 - roundoff)..=MAX_MIXTURE_PRESSURE_PA * (1.0 + roundoff))
                .contains(&pressure)
        {
            return Err(WaterError::UnsupportedEquilibrium);
        }
        return Candidate {
            temperature,
            vapor_pressure: 0.0,
            air_pressure: pressure,
            liquid_mass: 0.0,
            vapor_mass: 0.0,
            liquid_volume: 0.0,
            gas_volume: input.volume,
            water_energy: 0.0,
            air_energy: air_energy(temperature),
        }
        .observe(inventory, input, 0);
    }

    // First locate the vapor-floor bound independently of the energy residual.
    // At fixed water mass and volume, the 10 kPa vapor volume increases with T.
    let water_volume = input.volume / input.water;
    if !water_volume.is_finite() {
        return Err(WaterError::NonFiniteInput);
    }
    let floor_fits = |temperature| {
        Ok::<_, WaterError>(
            water_volume
                - table
                    .sample_single_phase(WaterPhase::Vapor, temperature, MIN_PRESSURE_PA)?
                    .volume_m3_kg
                <= 2e-13 * water_volume,
        )
    };
    let mut lower = table.saturation().min_temperature_k();
    let mut upper = MAX_TEMPERATURE_K;
    if !floor_fits(upper)? {
        return Err(WaterError::UnsupportedEquilibrium);
    }
    if !floor_fits(lower)? {
        let mut invalid = lower;
        let mut valid = upper;
        for _ in 0..64 {
            let middle = (invalid + valid) * 0.5;
            if floor_fits(middle)? {
                valid = middle;
            } else {
                invalid = middle;
            }
        }
        lower = valid;
    }
    let minimum = candidate(table, input, lower)?;
    let maximum = match candidate(table, input, upper) {
        Ok(state) => state,
        Err(WaterError::UnsupportedEquilibrium) => {
            // Total pressure increases along these rigid-vessel states. Locate
            // its feasible side before bisecting energy. Superheated states can
            // remain valid above the temperature where p_sat reaches the guard.
            let mut valid = lower;
            for _ in 0..64 {
                let middle = (valid + upper) * 0.5;
                match candidate(table, input, middle) {
                    Ok(_) => valid = middle,
                    Err(WaterError::UnsupportedEquilibrium) => upper = middle,
                    Err(error) => return Err(error),
                }
            }
            upper = valid;
            candidate(table, input, upper)?
        }
        Err(error) => return Err(error),
    };
    let tolerance = 1e-11 * input.energy.abs().max(1000.0);
    if input.energy < minimum.energy() - tolerance || input.energy > maximum.energy() + tolerance {
        return Err(WaterError::UnsupportedEquilibrium);
    }
    for iterations in 0..64 {
        let state = if (minimum.energy() - input.energy).abs() <= tolerance {
            minimum
        } else if (maximum.energy() - input.energy).abs() <= tolerance {
            maximum
        } else {
            candidate(table, input, (lower + upper) * 0.5)?
        };
        let residual = state.energy() - input.energy;
        if residual.abs() <= tolerance {
            return state.observe(inventory, input, iterations);
        }
        if residual < 0.0 {
            lower = state.temperature;
        } else {
            upper = state.temperature;
        }
    }
    Err(WaterError::IterationLimit)
}

/// Conserved mixture owner. Rejected thermal edits leave all state unchanged.
#[derive(Debug)]
pub struct MixtureVessel {
    inventory: MixtureInventory,
    equilibrium: MixtureEquilibrium,
}

impl MixtureVessel {
    /// Validates and closes an isolated rigid vessel.
    pub fn new(table: &WaterTable, inventory: MixtureInventory) -> Result<Self, WaterError> {
        let equilibrium = close_mixture(table, inventory)?;
        Ok(Self {
            inventory,
            equilibrium,
        })
    }

    /// Returns the conserved inputs. Closure never replaces them.
    pub fn inventory(&self) -> MixtureInventory {
        self.inventory
    }

    /// Returns the derived equilibrium for the current inputs.
    pub fn equilibrium(&self) -> MixtureEquilibrium {
        self.equilibrium
    }

    /// Applies external heat at fixed mass and rigid volume. The return value is
    /// the representable energy change for the caller's source ledger.
    pub fn apply_heat(&mut self, table: &WaterTable, heat_j: f64) -> Result<f64, WaterError> {
        if !heat_j.is_finite() {
            return Err(WaterError::NonFiniteInput);
        }
        let candidate = MixtureInventory {
            internal_energy_j: self.inventory.internal_energy_j + heat_j,
            ..self.inventory
        };
        let equilibrium = close_mixture(table, candidate)?;
        let applied = candidate.internal_energy_j - self.inventory.internal_energy_j;
        self.inventory = candidate;
        self.equilibrium = equilibrium;
        Ok(applied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rigid_mixture_energy_and_pressure_increase_across_the_feasible_interval() {
        let table = WaterTable::new();
        let saturated = table.saturation().sample(400.0).unwrap();
        let mut accepted = 0;
        let mut liquid_samples = 0;
        let mut vapor_samples = 0;
        for vapor_fraction in [1e-4, 0.01, 0.5, 1.0] {
            for air_pressure in [100.0, 50e3, 500e3] {
                let gas_volume = vapor_fraction * saturated.vapor_volume_m3_kg;
                let liquid = table
                    .sample_single_phase(
                        WaterPhase::Liquid,
                        400.0,
                        saturated.pressure_pa + air_pressure,
                    )
                    .unwrap();
                let specific = MixtureInventory {
                    water_mass_kg: 1.0,
                    air_mass_kg: air_pressure * gas_volume / (AIR_GAS_CONSTANT_J_KG_K * 400.0),
                    internal_energy_j: 0.0,
                    available_volume_m3: (1.0 - vapor_fraction) * liquid.volume_m3_kg + gas_volume,
                }
                .specific()
                .unwrap();
                let mut previous: Option<Candidate> = None;
                let mut ended = false;
                for index in 0..=610 {
                    let temperature = table.saturation().min_temperature_k()
                        + (MAX_TEMPERATURE_K - table.saturation().min_temperature_k())
                            * index as f64
                            / 610.0;
                    match candidate(&table, specific, temperature) {
                        Ok(state) => {
                            assert!(!ended, "feasibility must be contiguous");
                            if let Some(last) = previous {
                                assert!(state.energy() > last.energy());
                                assert!(
                                    state.vapor_pressure + state.air_pressure
                                        > last.vapor_pressure + last.air_pressure
                                );
                            }
                            liquid_samples += usize::from(state.liquid_mass > 0.0);
                            vapor_samples += usize::from(state.liquid_mass == 0.0);
                            accepted += 1;
                            previous = Some(state);
                        }
                        Err(WaterError::UnsupportedEquilibrium) => {
                            ended |= previous.is_some();
                        }
                        Err(error) => panic!("unexpected mixture error: {error}"),
                    }
                }
                assert!(previous.is_some());
            }
        }
        assert!(liquid_samples > 0 && vapor_samples > 0);
        println!("rigid mixture monotonicity: {accepted} feasible samples");
    }
}
