# Rust water and carrier-air thermodynamics

E09 has an isolated Rust f64 reference for saturated and stable single-phase
pure-water vessels, a bounded air/steam mixture approximation, finite gas
ventilation and conduction between two finite vessels.
[`water`](../engine/crates/sim-cpu/src/water/mod.rs) owns this reference.
It does not advance the M3 fluid scene or the default legacy sandbox.
[E09 evidence](validation/p1-m4-e09.md) records the checks and remaining gates.

## Conserved inputs and phase closure

`WaterInventory` stores water mass in kg, internal energy U in J and actual
available volume in m3. Available volume excludes solid geometry.
`WaterTable` owns the saturated, liquid and vapor projections.
`close_water` derives temperature, chamber pressure p0 and liquid/vapor amounts.
It tries the two-phase closure first, then the stable single-phase branches.
Projection pressure is a separate quantity.

At each candidate saturation temperature, the volume constraint determines the
vapor mass fraction:

```text
v = available_volume / mass
x = (v - v_liquid) / (v_vapor - v_liquid)
u = u_liquid + x * (u_vapor - u_liquid)
U = mass * u
```

The solver first finds the feasible temperature interval from the phase volumes.
It then bisects the internal-energy residual. Each search allows at most 64
iterations. The energy tolerance is `1e-11 * max(abs(U/mass), 1000 J/kg)`.
The floor provides an absolute scale. Supported states have substantially larger
specific internal energies.
Phase closure preserves the supplied inventory. It reports its energy residual.
It never replaces U with a reconstructed value or changes mass to fit geometry.

`WaterVessel` owns one inventory and a derived equilibrium.
`apply_heat` validates a candidate before replacing either value.
Its return value is the representable change in U for the caller's source ledger.
Unsupported heating or cooling leaves both values unchanged.
This operation models external heat at fixed mass and rigid volume.
It does not model conduction, moving-wall work or a fluid timestep.

## Single-phase closure

[`single_phase.rs`](../engine/crates/sim-cpu/src/water/single_phase.rs) owns
compressed-liquid and superheated-vapor properties and their inverse solve.
Liquid volume changes with pressure. Heating a full rigid liquid vessel therefore
produces a finite pressure response within the supported domain.

The liquid projection covers 273.15 to 623.15 K. Its pressure range is from
`max(10 kPa, p_sat(T))` to 20 MPa. The vapor projection covers the saturation
floor near 318.958 K to 623.15 K, from 10 kPa to `p_sat(T)`.
`sample_single_phase` requires an explicit `WaterPhase` and rejects queries on
the wrong side of the phase boundary. These boundaries use the interpolated
saturation projection, with the property errors recorded in the evidence article.

The inverse solve clips each temperature interval against both phase-volume
bounds. This preserves cold liquid states around water's density maximum.
At a candidate temperature, decreasing volume across the pressure row permits
binary search and linear volume inversion. Energy bisection then selects the
accepted temperature within that interval. The maximum is 64 energy bisections
in one bracket, after a bounded scan of the temperature intervals.

The energy tolerance remains `1e-11 * max(abs(U/mass), 1000 J/kg)`.
The reconstructed-volume residual must be at most `2e-13 * available_volume`.
Only representable roundoff at an interval endpoint can use the adjacent pressure
node. Unsupported inventories reject without modifying mass, U or volume.
Single-phase observations allocate all water to one phase. Saturated endpoints
retain the two-phase closure's priority and share the same boundary properties.

## Property source and generated tables

[IAPWS R7-97(2012)](https://iapws.org/technical-guidance/release/IF97-Rev)
is the equation source. The numerical coefficients are attributed to the
International Association for the Properties of Water and Steam.
[`if97.rs`](../engine/crates/sim-cpu/src/water/if97.rs) contains the required
region-1, region-2 and region-4 coefficients.
Published Tables 5, 15 and 35 provide independent numerical verification points.

`SaturationTable::new` generates 2441 immutable nodes from these equations.
Runtime generation requires no download, installed property library or copied
material catalogue. Nodes contain saturation pressure, both specific volumes
and both specific internal energies in SI units.
Temperature spacing is less than 0.125 K. Queries interpolate linearly.
The version is `iapws-if97-r7-97-2012-saturation-v1`.

IF97 sets saturated-liquid internal energy to zero at the triple point.
Enthalpy is derived as `h = u + p*v`, including after interpolation.
Legacy parcel enthalpy cannot be passed as U without an explicit conversion.
No legacy conversion is implemented in this reference.

The public table covers saturation from 10 kPa to 623.15 K.
The corresponding temperatures begin near 318.958 K.
The upper saturation pressure is about 16.529 MPa.
Single-phase liquid properties extend to 20 MPa. Pure vapor pressure remains
bounded by saturation below 623.15 K. Region 3 is outside this projection.

Single-phase temperature rows share all 2441 saturation nodes. Liquid rows also
cover the colder interval with spacing below 0.125 K. Each liquid row has 65
uniform pressure nodes. Each vapor row has 513 nodes, with pressure coordinate
`p = 10 kPa * (p_sat / 10 kPa)^(q * (2 - q))` for uniform `q` from zero to one.
This coordinate concentrates nodes near saturation, where dense vapor properties
vary quickly. Volume and internal energy interpolate linearly in temperature
and `q`. The version is `iapws-if97-r7-97-2012-single-phase-v1`.
Generation evaluates IF97 when constructing each table. Property lookups and closure use only the tables.
The f64 CPU reference has no per-frame performance qualification or GPU projection.

## Air/steam mixture approximation

[`mixture.rs`](../engine/crates/sim-cpu/src/water/mixture.rs) owns the isolated
`MixtureInventory`, `close_mixture` and `MixtureVessel` API.
Its version is `e09-air-steam-v1`. One inventory contains water mass, inert
carrier-air mass, total internal energy U and actual available volume.
Liquid and vapor water masses are derived observations. Carrier air is a single
nonreactive component; it does not supply O2 or combustion-product inventories.

The carrier model fixes `R_air = 287.05 J/(kg K)` and `gamma = 1.4`.
It uses `cv_air = R_air / (gamma - 1) = 717.625 J/(kg K)` and
`u_air(T) = cv_air * (T - 273.15 K)`.
The coefficient is a project model choice. Constant heat capacity across the
supported temperature range is an approximation.
[NASA's equation-of-state reference](https://www1.grc.nasa.gov/beginners-guide-to-aeronautics/equation-of-state/)
describes the ideal-gas relation; its
[specific-heat reference](https://www1.grc.nasa.gov/beginners-guide-to-aeronautics/specific-heats-cp-and-cv-1/)
describes `cp - cv = R` and `gamma = cp/cv`.

Air and water vapor occupy one shared gas volume. Liquid water displaces that
volume. The rigid-vessel constraints are:

```text
m_water = m_liquid + m_vapor
V_available = m_liquid * v_liquid(T, p0) + V_gas
p_air = m_air * R_air * T / V_gas
p0 = p_vapor + p_air
U = m_liquid * u_liquid(T, p0)
  + m_vapor * u_vapor(T, p_vapor) + m_air * u_air(T)
```

When liquid is present, `p_vapor = p_sat(T)` and
`V_gas = m_vapor * v_vapor(T, p_sat(T))`.
Liquid properties use total pressure p0. Vapor properties use water's partial
pressure. When all water is vapor, `V_gas = V_available`; a volume inversion of
the existing vapor projection supplies `p_vapor` and water internal energy.
The mixture never adds a separate air volume to the steam volume.

This development model omits humidity enhancement, water/air interaction and
dissolved air. It combines real-water projections with ideal carrier air and
additive partial pressures. It is not the full humid-air equation of state in
[IAPWS G8-10, sections 3 and 6](https://iapws.org/technical-guidance/release/SeaAir.download).
The 1 MPa total-pressure limit is a model guard, not a validated accuracy bound
for humid air. The tests establish closure consistency and conservation;
they do not measure mixture accuracy against independent humid-air data.

With positive air and water masses, temperature starts at the existing water
saturation floor near 318.958 K and ends at 623.15 K. Water vapor partial pressure
must be at least 10 kPa; total pressure must be at most 1 MPa. A state with liquid
therefore reaches its pressure limit below 623.15 K. A superheated mixture can
remain valid up to 623.15 K when its partial pressures satisfy the limits.
Room-temperature humidity and steam diluted below 10 kPa are unsupported.

Exactly zero air delegates to `close_water` and retains its wider pure-water
domain. A compressed pure liquid has no gas volume or vapor partial pressure;
its pressure comes from the liquid closure. The sum of gas partial pressures
does not describe that component limit. Exactly zero water uses analytic dry
air from 273.15 to 623.15 K and 10 kPa to 1 MPa. Derived dry-air temperature and
pressure checks and the mixture total-pressure guard allow `8 * f64::EPSILON`
relative arithmetic roundoff at inclusive endpoints. U, volume and observed
temperature/pressure remain unchanged. Empty vessels reject.

The solve normalizes mass, U and volume by total mass. It first finds the feasible
temperature interval from the vapor floor and total-pressure guard. For a
two-phase candidate, up to 64 logarithmic gas-volume bisections satisfy water
mass and actual volume with liquid properties at p0. Up to 64 temperature bisections then satisfy
total internal energy. Invalid candidates are never used as an energy-residual
sign. The energy tolerance is `1e-11 * max(abs(U/total_mass), 1000 J/kg)`.
The inner occupied-volume tolerance is `2e-13 * available_volume`.
The pressure-bound gas-volume endpoint can satisfy this tolerance directly;
it is accepted after checking phase masses and total pressure.
The logarithmic coordinate resolves very small gas volumes without an overflowing
volume ratio. Conservation tolerances do not guarantee relative accuracy for a
phase amount or partial pressure that is negligible at the supplied U/V precision.
Reported residuals retain extensive units.

`MixtureVessel::apply_heat` validates each candidate before committing its U and
observations. Its return value is the representable energy change for the
caller's source ledger. Rejected heat preserves both inventories and equilibrium.
This thermal operation does not advance fluid transport, chamber topology or conduction.

## Finite gas ventilation

[`vent.rs`](../engine/crates/sim-cpu/src/water/vent.rs) exchanges gas between one
`MixtureVessel` and an immutable prescribed `VentReservoir`. The reservoir has
explicit temperature and air/steam partial pressures. Its composition and gas
enthalpy use the same carrier-air coefficients and water projections as the
vessel. It represents an external source with fixed properties. It is not a
second finite vessel or a second ticking inventory.

`VentStep` supplies conductance in kg/(s Pa) and physical duration in seconds.
The development resistance model proposes a signed mass transfer:

```text
delta_m = conductance * (p_reservoir - p_vessel) * dt
```

Positive transfer enters the vessel. The upstream gas supplies the water/air
mass fractions and specific enthalpies. Outflow extracts only the current vapor
and air inventory. It does not extract liquid water. Subsequent equilibrium can
evaporate or condense water within the retained total water inventory.
The actual available vessel volume and property model remain unchanged.

An open rigid vessel changes internal energy through upstream enthalpy flux:

```text
h_air = cv_air * (T - 273.15 K) + R_air * T
h_vapor = u_vapor(T, p_vapor) + p_vapor * v_vapor(T, p_vapor)
source_enthalpy = delta_m_air * h_air + delta_m_water * h_vapor
delta_U = source_enthalpy + arithmetic_roundoff
```

The [DOE thermodynamics handbook](https://www.energy.gov/sites/default/files/2026-04/DOE-HDBK-1012-92_VOL1.pdf),
printed pages 18, 53 through 54 and 59, defines enthalpy and the general
input/output/storage balance. The equation above applies that unsteady balance
to this rigid vessel with no heat, shaft work or retained kinetic/potential energy.
This excludes liquid internal energy from the outgoing gas properties.
The summed component flow work equals total pressure times shared gas volume.
The model excludes kinetic/potential energy and external heat during the vent
step. It does not resolve an opening, velocity field, turbulence or shocks.
Linear conductance is a project development model. It is not a calibrated
orifice law or a measured ventilation rate.
[NIST CONTAM 3.4](https://nvlpubs.nist.gov/nistpubs/TechnicalNotes/NIST.TN.1887r1.pdf),
section 8.3.3, describes pressure-dependent mass flow and a linear low-flow
approximation. That reference supports the form, not the coefficient or accuracy
of this simplified operator. Reservoir temperature, pressure and composition are
prescribed external-source assumptions.

Active flow permits at most a 0.1 relative pressure difference, measured against
the smaller pressure, and at most 1 percent turnover of the chamber's current
gas mass per call. These are project guards. They do not establish a Mach-number
bound. Both the reservoir and the chamber must remain within their documented
thermodynamic domains. A gas-free compressed-liquid chamber rejects active
ventilation. Reservoir total pressure is limited to 10 kPa through 1 MPa.
Reservoir steam uses the existing stable vapor domain and 10 kPa partial-pressure
floor. Dry-air reservoirs use the existing 273.15 through 623.15 K model.

`MixtureVessel::apply_vent` first derives representable component mass changes.
Those changes fund the enthalpy source. It closes the complete candidate before
committing inventory and observations. The returned `VentExchange` supplies
signed actual water mass, air mass and U changes for the caller's source ledger.
It reports source enthalpy and arithmetic energy discrepancy separately.
The caller records the opposite reservoir exchange and owns any retry policy.

Negative or nonfinite inputs, excessive turnover/pressure differences, gas
overdraw, unrepresentable transfer, unsupported equilibrium and pressure crossing
reject without partial edits. The operator does not cap a request or retry it
automatically. A caller can retry a rejected timestep with a smaller duration.
Inclusive ratio and turnover guards permit `8 * f64::EPSILON` relative roundoff.
The pressure-crossing check permits `1e-10 * p_reservoir` in Pa at its endpoint,
consistent with the existing energy closure precision. Neither pressure is clamped.
Zero conductance, zero duration and equal pressure preserve state exactly.
Opening a vent therefore does not assign atmospheric pressure to the interior.
Repeated accepted steps relax its pressure through finite mass and energy flux.

## Conduction between finite rigid vessels

[`conduction.rs`](../engine/crates/sim-cpu/src/water/conduction.rs) owns
`conduct_heat`, `ConductionStep`, `ConductionExchange` and `ConductionError`.
The operator connects two `MixtureVessel` owners through a caller-supplied
thermal conductance G in W/K for a duration in seconds.
It exchanges internal energy without moving water, air or available volume.
Geometry and material conductivity determine G outside this operator.
No simulation clock, fluid stage or external heat source is advanced.

The explicit step uses the initial equilibrium temperatures:

```text
Q_into_first = G * (T_second - T_first) * duration
U_first_candidate = U_first + Q_into_first
U_second_candidate = U_second - Q_into_first
```

Both new inventories must close before either owner changes.
The complete transaction rejects if either closure fails or the temperatures
reverse their initial order beyond closure uncertainty.
The reversal check permits `1e-10 * max(T_first_candidate, T_second_candidate)` K
at the endpoint. It never clamps either temperature or the requested heat.
The caller selects a smaller duration after rejection if appropriate.

The exchange receipt records requested heat into the first vessel, both actual
energy changes and their signed sum as arithmetic roundoff, all in J.
Each actual energy change must match its requested signed heat within
`1e-10 * abs(Q_into_first)`. Their sum must satisfy the same bound.
This guard uses transferred heat as its scale, so large stored energies cannot
hide a spurious source. Closure energy residuals remain separate observations.
The caller records the roundoff explicitly without adding compensating heat.

Negative or nonfinite inputs, nonfinite arithmetic, active-transfer underflow,
unchanged energy on either side, excessive representation error, unsupported
candidates and temperature crossing reject both owners without partial edits.
Zero conductance, zero duration and equal observed temperatures preserve both
complete owners exactly. Inputs are checked before these controls.
The existing mixture domains apply, including pure-water compressed-liquid
states above the carrier-air pressure limit when air mass is exactly zero.
No vent gas-fraction or active-flow pressure guard applies to conduction.

## Unsupported states and remaining work

Invalid finite-value, mass, volume and temperature inputs return typed errors.
States without a feasible equilibrium return `UnsupportedEquilibrium`.
Saturated liquid and vapor endpoints are supported.
Compressed liquid and superheated vapor are supported within the domains above.
Fully liquid heated cavities reject if the pressure or temperature exceeds them.
The solver never clamps temperature, energy or phase amounts into the table.

Ice, reactive carrier species, hot fire/water mixtures and elemental phase families remain
outside this reference. It does not satisfy CUR-07 or CUR-09.
It does not establish FIRE-A03, FIRE-A12 or FIRE-PRECONDITIONS.
The next E09 work integrates conservative thermal transport and conduction
with the fluid reference. The isolated pair operator does not supply that coupling.
Chamber topology, broader current-material domains and GPU parity follow under
the [M4 contract](plans/fluid-gpu-redesign/plan.md#thermodynamics-and-chamber-closure).
All remain required before M4 or E09 can be validated.

## Verification

[`water_vessel.rs`](../engine/crates/sim-cpu/tests/water_vessel.rs) checks phase
amounts, actual geometry, source heat, rejected candidates and repeated closure.
Property unit tests compare all published verification points for the selected
IF97 equations. Table tests check every interval at three interior positions.
They report interpolation error separately from the nonlinear closure residual.
They also compare 610, 1220 and 2440 intervals to test refinement.
Single-phase property tests compare stratified samples against the source equations,
check refinement and inspect pressure monotonicity and isochoric energy.
[`water_single_phase.rs`](../engine/crates/sim-cpu/tests/water_single_phase.rs)
checks a published liquid inversion point, cold and hot states, domain boundaries,
all mass scales, finite liquid pressure, reversible source heat, rejected edits
and transitions across both saturation endpoints.
Table error and nonlinear closure residual are separate measurements.
[`water_mixture.rs`](../engine/crates/sim-cpu/tests/water_mixture.rs) checks shared
gas volume, partial versus total pressure, superheated states, mass scales,
repeat closure, reversible heating across the zero-liquid boundary, component
limits, rejected inputs and atomic thermal edits. A stratified fixed-inventory
unit test checks contiguous feasibility and increasing energy/pressure along
the mixture temperature interval. These samples support the bisection assumption;
they are not a proof over every possible inventory.
[`water_vent.rs`](../engine/crates/sim-cpu/tests/water_vent.rs) checks finite
inflow/outflow, gas composition, upstream enthalpy, source accounting, controls,
time refinement, mass scales and atomic rejection. These checks qualify the
isolated reference. They do not validate browser or GPU vent behavior.
[`water_conduction.rs`](../engine/crates/sim-cpu/tests/water_conduction.rs)
checks two finite heat capacities, exact controls, signed energy receipts,
analytic time refinement, inventory scales, evaporation/condensation and atomic
rejection. It also checks compressed liquid above 1 MPa and transfers that
cannot change both stored energies accurately. These CPU checks do not execute
browser or GPU conduction.

Run focused checks from `engine/`:

```sh
cargo test --locked -p particle-sim-cpu water:: -- --nocapture
cargo test --locked -p particle-sim-cpu --test water_vessel --test water_single_phase --test water_mixture --test water_vent --test water_conduction
```

Run `mise run ci` from the repository root before delivering engine changes.
