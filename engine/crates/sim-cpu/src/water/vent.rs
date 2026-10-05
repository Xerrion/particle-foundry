//! Finite gas exchange for the isolated E09 rigid-vessel reference.
//! The caller owns the external source ledger and the policy for smaller steps.

use super::{
    AIR_GAS_CONSTANT_J_KG_K, AIR_HEAT_CAPACITY_J_KG_K, AIR_REFERENCE_TEMPERATURE_K,
    MAX_MIXTURE_PRESSURE_PA, MAX_TEMPERATURE_K, MIN_PRESSURE_PA, MIN_TEMPERATURE_K,
    MixtureEquilibrium, MixtureInventory, WaterError, WaterPhase, WaterTable, close_mixture,
};

/// Development bound on `abs(p_reservoir - p_chamber) / min(p_reservoir, p_chamber)`.
/// This is not a Mach-number limit or a certified orifice-flow correlation.
pub const MAX_VENT_RELATIVE_PRESSURE_DIFFERENCE: f64 = 0.1;
/// Maximum requested and accepted gas mass per step, relative to chamber gas mass.
pub const MAX_VENT_GAS_FRACTION: f64 = 0.01;

const ENDPOINT_ROUNDOFF: f64 = 8.0 * f64::EPSILON;
// The mixture energy closure uses a relative residual of 1e-11. Allow its
// pressure endpoint roundoff without replacing either pressure observation.
const PRESSURE_ENDPOINT_TOLERANCE: f64 = 1e-10;

/// Explicit rejection from the bounded finite-vent reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VentError {
    /// An input is negative or nonfinite, or a derived quantity is nonfinite.
    InvalidInput,
    /// The reservoir does not fit the selected gas-property domain.
    UnsupportedReservoir(WaterError),
    /// Gas is absent, the pressure or relative difference exceeds its guard, or gas properties reject.
    UnsupportedFlowDomain,
    /// Transfer exceeds one percent of chamber gas mass or the available donor component.
    StepTooLarge,
    /// The candidate pressure crosses the reservoir pressure beyond endpoint roundoff.
    PressureCrossing,
    /// A requested nonzero mass or energy transfer cannot change the inventory.
    UnrepresentableTransfer,
    /// The candidate inventory cannot close within the existing mixture domain.
    Closure(WaterError),
}

impl std::fmt::Display for VentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for VentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::UnsupportedReservoir(error) | Self::Closure(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct GasProperties {
    water_fraction: f64,
    air_fraction: f64,
    water_enthalpy: f64,
    air_enthalpy: f64,
}

impl GasProperties {
    fn enthalpy(self) -> f64 {
        self.water_fraction * self.water_enthalpy + self.air_fraction * self.air_enthalpy
    }
}

fn air_enthalpy(temperature: f64) -> f64 {
    AIR_HEAT_CAPACITY_J_KG_K * (temperature - AIR_REFERENCE_TEMPERATURE_K)
        + AIR_GAS_CONSTANT_J_KG_K * temperature
}

/// Immutable external gas source at fixed temperature and partial pressures.
/// The source is a prescribed boundary, not a second conserved chamber.
/// Dry air supports 273.15 to 623.15 K. Present vapor retains the 10 kPa table floor.
/// Total pressure must be 10 kPa to 1 MPa, including the pure-steam limit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VentReservoir {
    temperature_k: f64,
    air_partial_pressure_pa: f64,
    vapor_partial_pressure_pa: f64,
    gas: GasProperties,
}

impl VentReservoir {
    /// Validates stable gas properties and derives mass fractions from shared-volume densities.
    /// Negative or nonfinite inputs reject. A zero partial pressure omits that component.
    pub fn new(
        table: &WaterTable,
        temperature_k: f64,
        air_partial_pressure_pa: f64,
        vapor_partial_pressure_pa: f64,
    ) -> Result<Self, VentError> {
        if [
            temperature_k,
            air_partial_pressure_pa,
            vapor_partial_pressure_pa,
        ]
        .iter()
        .any(|value| !value.is_finite())
            || air_partial_pressure_pa < 0.0
            || vapor_partial_pressure_pa < 0.0
        {
            return Err(VentError::InvalidInput);
        }
        if !(MIN_TEMPERATURE_K..=MAX_TEMPERATURE_K).contains(&temperature_k) {
            return Err(VentError::UnsupportedReservoir(
                WaterError::TemperatureOutsideTable,
            ));
        }
        let pressure = air_partial_pressure_pa + vapor_partial_pressure_pa;
        if !pressure.is_finite() {
            return Err(VentError::InvalidInput);
        }
        if !(MIN_PRESSURE_PA..=MAX_MIXTURE_PRESSURE_PA).contains(&pressure) {
            return Err(VentError::UnsupportedReservoir(
                WaterError::PressureOutsideTable,
            ));
        }
        let air_density = air_partial_pressure_pa / (AIR_GAS_CONSTANT_J_KG_K * temperature_k);
        let (water_density, water_enthalpy) = if vapor_partial_pressure_pa == 0.0 {
            (0.0, 0.0)
        } else {
            let properties = table
                .sample_single_phase(WaterPhase::Vapor, temperature_k, vapor_partial_pressure_pa)
                .map_err(VentError::UnsupportedReservoir)?;
            (
                1.0 / properties.volume_m3_kg,
                properties.energy_j_kg + vapor_partial_pressure_pa * properties.volume_m3_kg,
            )
        };
        let density = air_density + water_density;
        let gas = GasProperties {
            water_fraction: water_density / density,
            air_fraction: air_density / density,
            water_enthalpy,
            air_enthalpy: air_enthalpy(temperature_k),
        };
        if [
            air_density,
            water_density,
            density,
            gas.air_fraction,
            gas.water_fraction,
            gas.water_enthalpy,
            gas.air_enthalpy,
            gas.enthalpy(),
        ]
        .iter()
        .any(|value| !value.is_finite())
        {
            return Err(VentError::InvalidInput);
        }
        if (air_partial_pressure_pa > 0.0 && gas.air_fraction == 0.0)
            || (vapor_partial_pressure_pa > 0.0 && gas.water_fraction == 0.0)
        {
            return Err(VentError::UnrepresentableTransfer);
        }
        Ok(Self {
            temperature_k,
            air_partial_pressure_pa,
            vapor_partial_pressure_pa,
            gas,
        })
    }

    /// External gas temperature, in K.
    pub fn temperature_k(&self) -> f64 {
        self.temperature_k
    }

    /// Sum of external air and water-vapor partial pressures, in Pa.
    pub fn pressure_pa(&self) -> f64 {
        self.air_partial_pressure_pa + self.vapor_partial_pressure_pa
    }

    /// External carrier-air partial pressure, in Pa.
    pub fn air_partial_pressure_pa(&self) -> f64 {
        self.air_partial_pressure_pa
    }

    /// External water-vapor partial pressure, in Pa.
    pub fn vapor_partial_pressure_pa(&self) -> f64 {
        self.vapor_partial_pressure_pa
    }

    /// Water-vapor mass fraction of external gas.
    pub fn water_mass_fraction(&self) -> f64 {
        self.gas.water_fraction
    }

    /// Carrier-air mass fraction of external gas.
    pub fn air_mass_fraction(&self) -> f64 {
        self.gas.air_fraction
    }

    /// Mass-weighted external gas enthalpy, in J/kg, including pressure flow work.
    pub fn specific_enthalpy_j_kg(&self) -> f64 {
        self.gas.enthalpy()
    }
}

/// One explicit linear-conductance step. The caller selects both finite nonnegative values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VentStep {
    /// Gas conductance in kg/(s Pa). This coefficient does not specify vent geometry.
    pub conductance_kg_s_pa: f64,
    /// Duration, in seconds. Zero selects an unchanged control.
    pub dt_s: f64,
}

/// Signed exchange into the chamber. The caller applies the opposite signs to its source ledger.
/// Actual deltas include floating-point representation. Energy roundoff is separate from source enthalpy.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VentExchange {
    /// Requested gas mass `C * (p_reservoir - p_chamber) * dt`, in kg.
    pub requested_gas_mass_delta_kg: f64,
    /// Actual representable change in total water inventory, in kg.
    pub water_mass_delta_kg: f64,
    /// Actual representable change in inert air inventory, in kg.
    pub air_mass_delta_kg: f64,
    /// Upstream component enthalpies multiplied by actual component mass changes, in J.
    pub source_enthalpy_j: f64,
    /// Actual representable change in chamber internal energy, in J.
    pub internal_energy_delta_j: f64,
    /// Actual energy change minus source enthalpy, in J.
    pub energy_roundoff_j: f64,
    /// Actual water change minus its requested upstream fraction, in kg.
    pub water_mass_roundoff_kg: f64,
    /// Actual air change minus its requested upstream fraction, in kg.
    pub air_mass_roundoff_kg: f64,
}

pub(super) struct PreparedVent {
    pub(super) inventory: MixtureInventory,
    pub(super) equilibrium: MixtureEquilibrium,
    pub(super) exchange: VentExchange,
}

fn chamber_gas(
    table: &WaterTable,
    inventory: MixtureInventory,
    equilibrium: MixtureEquilibrium,
    gas_mass: f64,
) -> Result<GasProperties, VentError> {
    let water_enthalpy = if equilibrium.vapor_water_mass_kg == 0.0 {
        0.0
    } else {
        let water = table
            .sample_single_phase(
                WaterPhase::Vapor,
                equilibrium.temperature_k,
                equilibrium.vapor_partial_pressure_pa,
            )
            .map_err(|_| VentError::UnsupportedFlowDomain)?;
        water.energy_j_kg + equilibrium.vapor_partial_pressure_pa * water.volume_m3_kg
    };
    // Retained liquid mass and energy never enter the donor gas enthalpy.
    let gas = GasProperties {
        water_fraction: equilibrium.vapor_water_mass_kg / gas_mass,
        air_fraction: inventory.air_mass_kg / gas_mass,
        water_enthalpy,
        air_enthalpy: air_enthalpy(equilibrium.temperature_k),
    };
    if [
        gas.water_fraction,
        gas.air_fraction,
        gas.water_enthalpy,
        gas.air_enthalpy,
    ]
    .iter()
    .any(|value| !value.is_finite())
    {
        return Err(VentError::InvalidInput);
    }
    if (equilibrium.vapor_water_mass_kg > 0.0 && gas.water_fraction == 0.0)
        || (inventory.air_mass_kg > 0.0 && gas.air_fraction == 0.0)
    {
        return Err(VentError::UnrepresentableTransfer);
    }
    Ok(gas)
}

fn represented_delta(initial: f64, intended: f64) -> Result<(f64, f64), VentError> {
    let candidate = initial + intended;
    let actual = candidate - initial;
    if !candidate.is_finite() || !actual.is_finite() {
        return Err(VentError::InvalidInput);
    }
    if intended != 0.0 && (actual == 0.0 || actual.signum() != intended.signum()) {
        return Err(VentError::UnrepresentableTransfer);
    }
    Ok((candidate, actual))
}

pub(super) fn prepare_vent(
    table: &WaterTable,
    inventory: MixtureInventory,
    equilibrium: MixtureEquilibrium,
    reservoir: &VentReservoir,
    step: VentStep,
) -> Result<Option<PreparedVent>, VentError> {
    if !step.conductance_kg_s_pa.is_finite()
        || !step.dt_s.is_finite()
        || step.conductance_kg_s_pa < 0.0
        || step.dt_s < 0.0
    {
        return Err(VentError::InvalidInput);
    }
    let pressure_delta = reservoir.pressure_pa() - equilibrium.pressure_pa;
    if step.conductance_kg_s_pa == 0.0 || step.dt_s == 0.0 || pressure_delta == 0.0 {
        return Ok(None);
    }
    let relative_difference =
        pressure_delta.abs() / reservoir.pressure_pa().min(equilibrium.pressure_pa);
    if !relative_difference.is_finite()
        || relative_difference > MAX_VENT_RELATIVE_PRESSURE_DIFFERENCE * (1.0 + ENDPOINT_ROUNDOFF)
        || equilibrium.pressure_pa > MAX_MIXTURE_PRESSURE_PA * (1.0 + ENDPOINT_ROUNDOFF)
    {
        return Err(VentError::UnsupportedFlowDomain);
    }
    let gas_mass = inventory.air_mass_kg + equilibrium.vapor_water_mass_kg;
    if !gas_mass.is_finite() {
        return Err(VentError::InvalidInput);
    }
    if gas_mass <= 0.0 || equilibrium.gas_volume_m3 <= 0.0 {
        return Err(VentError::UnsupportedFlowDomain);
    }
    let requested = step.conductance_kg_s_pa * pressure_delta * step.dt_s;
    if !requested.is_finite() {
        return Err(VentError::InvalidInput);
    }
    if requested == 0.0 {
        return Err(VentError::UnrepresentableTransfer);
    }
    let budget = MAX_VENT_GAS_FRACTION * gas_mass;
    if requested.abs() > budget * (1.0 + ENDPOINT_ROUNDOFF) {
        return Err(VentError::StepTooLarge);
    }
    let upstream = if requested > 0.0 {
        reservoir.gas
    } else {
        chamber_gas(table, inventory, equilibrium, gas_mass)?
    };
    let intended_water = requested * upstream.water_fraction;
    let intended_air = requested * upstream.air_fraction;
    if (upstream.water_fraction > 0.0 && intended_water == 0.0)
        || (upstream.air_fraction > 0.0 && intended_air == 0.0)
    {
        return Err(VentError::UnrepresentableTransfer);
    }
    let (water_mass, water_delta) = represented_delta(inventory.water_mass_kg, intended_water)?;
    let (air_mass, air_delta) = represented_delta(inventory.air_mass_kg, intended_air)?;
    if (water_delta + air_delta).abs() > budget * (1.0 + ENDPOINT_ROUNDOFF) {
        return Err(VentError::StepTooLarge);
    }
    if requested < 0.0
        && (-water_delta > equilibrium.vapor_water_mass_kg || -air_delta > inventory.air_mass_kg)
    {
        return Err(VentError::StepTooLarge);
    }
    let source_enthalpy = water_delta * upstream.water_enthalpy + air_delta * upstream.air_enthalpy;
    if !source_enthalpy.is_finite() {
        return Err(VentError::InvalidInput);
    }
    if source_enthalpy == 0.0 || source_enthalpy.signum() != requested.signum() {
        return Err(VentError::UnrepresentableTransfer);
    }
    let (internal_energy, energy_delta) =
        represented_delta(inventory.internal_energy_j, source_enthalpy)?;
    let candidate = MixtureInventory {
        water_mass_kg: water_mass,
        air_mass_kg: air_mass,
        internal_energy_j: internal_energy,
        ..inventory
    };
    let closed = close_mixture(table, candidate).map_err(VentError::Closure)?;
    let pressure_tolerance = PRESSURE_ENDPOINT_TOLERANCE * reservoir.pressure_pa();
    if (requested > 0.0 && closed.pressure_pa > reservoir.pressure_pa() + pressure_tolerance)
        || (requested < 0.0 && closed.pressure_pa < reservoir.pressure_pa() - pressure_tolerance)
    {
        return Err(VentError::PressureCrossing);
    }
    let exchange = VentExchange {
        requested_gas_mass_delta_kg: requested,
        water_mass_delta_kg: water_delta,
        air_mass_delta_kg: air_delta,
        source_enthalpy_j: source_enthalpy,
        internal_energy_delta_j: energy_delta,
        energy_roundoff_j: energy_delta - source_enthalpy,
        water_mass_roundoff_kg: water_delta - intended_water,
        air_mass_roundoff_kg: air_delta - intended_air,
    };
    Ok(Some(PreparedVent {
        inventory: candidate,
        equilibrium: closed,
        exchange,
    }))
}
