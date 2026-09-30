//! Isolated E09 pure-water thermodynamic reference. Conserved mass, internal energy
//! and available volume determine phase amounts. This is not a fluid stage.
//! No GPU, air mixture, legacy conversion or fire property support is implied.

mod if97;
mod single_phase;
mod table;

pub use single_phase::{
    MAX_PRESSURE_PA, MIN_TEMPERATURE_K, SINGLE_PHASE_PROPERTY_VERSION, WaterPhase, WaterProperties,
    WaterTable,
};

pub use table::{
    MAX_TEMPERATURE_K, MIN_PRESSURE_PA, SaturationProperties, SaturationTable,
    WATER_PROPERTY_VERSION,
};

/// Explicit rejection from the bounded water reference. State is never clamped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaterError {
    /// An input or a derived specific quantity is not finite.
    NonFiniteInput,
    /// Mass and available volume must both be positive.
    NonPositiveInventory,
    /// A property lookup temperature is outside the bounded table.
    TemperatureOutsideTable,
    /// A pressure query is outside 10 kPa to 20 MPa.
    PressureOutsideTable,
    /// No liquid/vapor equilibrium fits the inventory within this table.
    UnsupportedEquilibrium,
    /// The bounded nonlinear solve did not meet its energy tolerance.
    IterationLimit,
}

impl std::fmt::Display for WaterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for WaterError {}

/// Extensive conserved inputs for one isolated, rigid pure-water vessel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterInventory {
    /// Total water mass, in kg.
    pub mass_kg: f64,
    /// Total internal energy, in J, using the IF97 reference zero. This is not H.
    pub internal_energy_j: f64,
    /// Actual volume available to water, in m3, after excluding solids.
    pub available_volume_m3: f64,
}

impl WaterInventory {
    fn specific(self) -> Result<(f64, f64), WaterError> {
        if !self.mass_kg.is_finite()
            || !self.internal_energy_j.is_finite()
            || !self.available_volume_m3.is_finite()
        {
            return Err(WaterError::NonFiniteInput);
        }
        if self.mass_kg <= 0.0 || self.available_volume_m3 <= 0.0 {
            return Err(WaterError::NonPositiveInventory);
        }
        let u = self.internal_energy_j / self.mass_kg;
        let v = self.available_volume_m3 / self.mass_kg;
        if !u.is_finite() || !v.is_finite() {
            return Err(WaterError::NonFiniteInput);
        }
        Ok((u, v))
    }
}

/// Derived equilibrium. Phase amounts are observations, not extra inventories.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterEquilibrium {
    /// Common temperature, in K.
    pub temperature_k: f64,
    /// Chamber pressure p0, in Pa. This is not mechanical projection pressure.
    pub pressure_pa: f64,
    /// Vapor mass divided by total water mass, between zero and one.
    pub vapor_mass_fraction: f64,
    /// Derived liquid mass, in kg.
    pub liquid_mass_kg: f64,
    /// Derived vapor mass, in kg.
    pub vapor_mass_kg: f64,
    /// Derived liquid volume, in m3.
    pub liquid_volume_m3: f64,
    /// Derived vapor volume, in m3.
    pub vapor_volume_m3: f64,
    /// Signed reconstructed energy minus conserved input, in J.
    pub energy_residual_j: f64,
    /// Number of energy bisections in the accepted temperature bracket.
    pub iterations: u32,
}

/// Solves saturated or stable single-phase water from conserved inputs.
/// Unsupported candidates leave the input unchanged. Saturation has priority at
/// phase endpoints. Each single-phase temperature interval uses at most 64 bisections.
pub fn close_water(
    table: &WaterTable,
    inventory: WaterInventory,
) -> Result<WaterEquilibrium, WaterError> {
    match close_saturated_water(table.saturation(), inventory) {
        Err(WaterError::UnsupportedEquilibrium) => table.close_single_phase(inventory),
        result => result,
    }
}

fn close_saturated_water(
    table: &SaturationTable,
    inventory: WaterInventory,
) -> Result<WaterEquilibrium, WaterError> {
    let (u, v) = inventory.specific()?;
    let mut lower = table.min_temperature_k();
    let mut upper = MAX_TEMPERATURE_K;
    let feasible =
        |s: SaturationProperties| v >= s.liquid_volume_m3_kg && v <= s.vapor_volume_m3_kg;
    if !feasible(table.sample(lower)?) {
        return Err(WaterError::UnsupportedEquilibrium);
    }
    // Saturated liquid volume increases and vapor volume decreases in this domain.
    // Keep the feasible side when locating a single-phase endpoint.
    if !feasible(table.sample(upper)?) {
        let mut valid = lower;
        for _ in 0..64 {
            let middle = (valid + upper) * 0.5;
            if feasible(table.sample(middle)?) {
                valid = middle;
            } else {
                upper = middle;
            }
        }
        upper = valid;
    }
    let energy = |s: SaturationProperties| {
        let x = (v - s.liquid_volume_m3_kg) / (s.vapor_volume_m3_kg - s.liquid_volume_m3_kg);
        s.liquid_energy_j_kg + x * (s.vapor_energy_j_kg - s.liquid_energy_j_kg)
    };
    let min_u = energy(table.sample(lower)?);
    let max_u = energy(table.sample(upper)?);
    // A tolerance accepts representable endpoint roundoff. It never repairs U.
    let tolerance = 1e-11 * u.abs().max(1000.0);
    if u < min_u - tolerance || u > max_u + tolerance {
        return Err(WaterError::UnsupportedEquilibrium);
    }
    for iterations in 0..64 {
        let t = if (u - min_u).abs() <= tolerance {
            lower
        } else if (u - max_u).abs() <= tolerance {
            upper
        } else {
            (lower + upper) * 0.5
        };
        let s = table.sample(t)?;
        let residual = energy(s) - u;
        if residual.abs() <= tolerance {
            let x = (v - s.liquid_volume_m3_kg) / (s.vapor_volume_m3_kg - s.liquid_volume_m3_kg);
            if !(0.0..=1.0).contains(&x) {
                return Err(WaterError::UnsupportedEquilibrium);
            }
            let vapor_mass_kg = inventory.mass_kg * x;
            let liquid_mass_kg = inventory.mass_kg - vapor_mass_kg;
            return Ok(WaterEquilibrium {
                temperature_k: t,
                pressure_pa: s.pressure_pa,
                vapor_mass_fraction: x,
                liquid_mass_kg,
                vapor_mass_kg,
                liquid_volume_m3: liquid_mass_kg * s.liquid_volume_m3_kg,
                vapor_volume_m3: vapor_mass_kg * s.vapor_volume_m3_kg,
                energy_residual_j: residual * inventory.mass_kg,
                iterations,
            });
        }
        if residual < 0.0 {
            lower = t;
        } else {
            upper = t;
        }
    }
    Err(WaterError::IterationLimit)
}

/// One conserved inventory with a derived equilibrium. Thermal edits commit only
/// after successful closure. This owner is separate from the M3 fluid session.
#[derive(Debug)]
pub struct WaterVessel {
    inventory: WaterInventory,
    equilibrium: WaterEquilibrium,
}

impl WaterVessel {
    /// Validates and closes an isolated pure-water vessel.
    pub fn new(table: &WaterTable, inventory: WaterInventory) -> Result<Self, WaterError> {
        let equilibrium = close_water(table, inventory)?;
        Ok(Self {
            inventory,
            equilibrium,
        })
    }

    /// Returns conserved input values. Phase closure never replaces these values.
    pub fn inventory(&self) -> WaterInventory {
        self.inventory
    }

    /// Returns derived equilibrium for the current inventory.
    pub fn equilibrium(&self) -> WaterEquilibrium {
        self.equilibrium
    }

    /// Applies signed external heat in J to a rigid vessel. Returns the actual
    /// representable energy change for the source ledger. Unsupported candidates
    /// leave both inventory and observations unchanged. The caller owns the source ledger and any retry policy.
    pub fn apply_heat(&mut self, table: &WaterTable, heat_j: f64) -> Result<f64, WaterError> {
        if !heat_j.is_finite() {
            return Err(WaterError::NonFiniteInput);
        }
        let candidate = WaterInventory {
            internal_energy_j: self.inventory.internal_energy_j + heat_j,
            ..self.inventory
        };
        let equilibrium = close_water(table, candidate)?;
        let accepted_heat_j = candidate.internal_energy_j - self.inventory.internal_energy_j;
        self.inventory = candidate;
        self.equilibrium = equilibrium;
        Ok(accepted_heat_j)
    }
}
