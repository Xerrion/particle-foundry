//! Heat exchange between two isolated rigid vessels with one conserved owner each.
//! The caller selects the conductance, duration and any smaller-step retry policy.

use super::{MixtureInventory, MixtureVessel, WaterError, WaterTable};

/// Maximum arithmetic error relative to the requested heat magnitude.
/// Each energy delta and their signed sum must fit this bound.
pub const MAX_CONDUCTION_RELATIVE_ENERGY_ROUNDOFF: f64 = 1e-10;

// Mixture closure uses a relative energy residual of 1e-11. Permit the resulting
// temperature endpoint uncertainty without replacing either temperature.
const TEMPERATURE_ENDPOINT_TOLERANCE: f64 = 1e-10;

/// One explicit conductance step with finite nonnegative inputs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConductionStep {
    /// Thermal conductance, in W/K. The caller supplies the geometry and material model.
    pub conductance_w_k: f64,
    /// Duration, in seconds. Zero selects an unchanged control.
    pub duration_s: f64,
}

/// Signed changes in the two conserved energy inventories, in J.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ConductionExchange {
    /// Requested heat `G * (T_second - T_first) * duration`, signed into the first vessel.
    pub requested_heat_into_first_j: f64,
    /// Actual representable energy change in the first vessel.
    pub first_energy_delta_j: f64,
    /// Actual representable energy change in the second vessel.
    pub second_energy_delta_j: f64,
    /// Signed sum of both energy changes. The caller records this arithmetic error explicitly.
    pub energy_roundoff_j: f64,
}

/// Explicit rejection from the bounded conduction reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConductionError {
    /// An input is negative or nonfinite, or an arithmetic result is nonfinite.
    InvalidInput,
    /// A nonzero request cannot change both energies within the relative roundoff bound.
    UnrepresentableTransfer,
    /// The candidate temperatures reverse their initial order beyond closure endpoint uncertainty.
    TemperatureCrossing,
    /// The first candidate inventory cannot close within the existing mixture domain.
    FirstClosure(WaterError),
    /// The second candidate inventory cannot close within the existing mixture domain.
    SecondClosure(WaterError),
}

impl std::fmt::Display for ConductionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ConductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::FirstClosure(error) | Self::SecondClosure(error) => Some(error),
            _ => None,
        }
    }
}

fn represented_energy(initial: f64, intended: f64) -> Result<(f64, f64), ConductionError> {
    let candidate = initial + intended;
    let actual = candidate - initial;
    if !candidate.is_finite() || !actual.is_finite() {
        return Err(ConductionError::InvalidInput);
    }
    if actual == 0.0 || actual.signum() != intended.signum() {
        return Err(ConductionError::UnrepresentableTransfer);
    }
    Ok((candidate, actual))
}

/// Exchanges heat at fixed component masses and rigid volumes, then closes both vessels.
/// Heat follows the initial temperature difference. Both candidates must close before mutation.
/// Zero conductance, zero duration and equal initial temperatures leave both owners unchanged.
/// Active steps must change both energies within the relative arithmetic error bound.
/// Temperature reversal beyond closure endpoint uncertainty rejects the complete step.
/// This operation does not advance fluid transport or a simulation clock.
pub fn conduct_heat(
    table: &WaterTable,
    first: &mut MixtureVessel,
    second: &mut MixtureVessel,
    step: ConductionStep,
) -> Result<ConductionExchange, ConductionError> {
    if !step.conductance_w_k.is_finite()
        || !step.duration_s.is_finite()
        || step.conductance_w_k < 0.0
        || step.duration_s < 0.0
    {
        return Err(ConductionError::InvalidInput);
    }
    let temperature_delta = second.equilibrium().temperature_k - first.equilibrium().temperature_k;
    if step.conductance_w_k == 0.0 || step.duration_s == 0.0 || temperature_delta == 0.0 {
        return Ok(ConductionExchange::default());
    }
    let power = step.conductance_w_k * temperature_delta;
    let requested = power * step.duration_s;
    if !power.is_finite() || !requested.is_finite() {
        return Err(ConductionError::InvalidInput);
    }
    if requested == 0.0 {
        return Err(ConductionError::UnrepresentableTransfer);
    }
    let first_inventory = first.inventory();
    let second_inventory = second.inventory();
    let (first_energy, first_delta) =
        represented_energy(first_inventory.internal_energy_j, requested)?;
    let (second_energy, second_delta) =
        represented_energy(second_inventory.internal_energy_j, -requested)?;
    let energy_roundoff = first_delta + second_delta;
    if !energy_roundoff.is_finite() {
        return Err(ConductionError::InvalidInput);
    }
    // Stored energies may be much larger than Q. A tolerance based on stored U
    // would permit a large spurious source relative to the attempted transfer.
    // Bound each representation error and the net source by Q instead.
    let roundoff_budget = MAX_CONDUCTION_RELATIVE_ENERGY_ROUNDOFF * requested.abs();
    if (first_delta - requested).abs() > roundoff_budget
        || (second_delta + requested).abs() > roundoff_budget
        || energy_roundoff.abs() > roundoff_budget
    {
        return Err(ConductionError::UnrepresentableTransfer);
    }
    let first_candidate = MixtureVessel::new(
        table,
        MixtureInventory {
            internal_energy_j: first_energy,
            ..first_inventory
        },
    )
    .map_err(ConductionError::FirstClosure)?;
    let second_candidate = MixtureVessel::new(
        table,
        MixtureInventory {
            internal_energy_j: second_energy,
            ..second_inventory
        },
    )
    .map_err(ConductionError::SecondClosure)?;
    let first_temperature = first_candidate.equilibrium().temperature_k;
    let second_temperature = second_candidate.equilibrium().temperature_k;
    let temperature_tolerance =
        TEMPERATURE_ENDPOINT_TOLERANCE * first_temperature.max(second_temperature);
    if (requested > 0.0 && first_temperature - second_temperature > temperature_tolerance)
        || (requested < 0.0 && second_temperature - first_temperature > temperature_tolerance)
    {
        return Err(ConductionError::TemperatureCrossing);
    }
    *first = first_candidate;
    *second = second_candidate;
    Ok(ConductionExchange {
        requested_heat_into_first_j: requested,
        first_energy_delta_j: first_delta,
        second_energy_delta_j: second_delta,
        energy_roundoff_j: energy_roundoff,
    })
}
