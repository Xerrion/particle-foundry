# Rust water thermodynamics

E09 has an isolated Rust f64 reference for saturated pure-water vessels.
[`water`](../engine/crates/sim-cpu/src/water/mod.rs) owns this reference.
It does not advance the M3 fluid scene or the default legacy sandbox.
[E09 evidence](validation/p1-m4-e09.md) records the checks and remaining gates.

## Conserved inputs and phase closure

`WaterInventory` stores water mass in kg, internal energy U in J and actual
available volume in m3. Available volume excludes solid geometry.
`close_water` derives temperature, chamber pressure p0 and liquid/vapor amounts.
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

## Property source and generated table

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
The broader plan's proposed 20 MPa limit requires other property support above
this saturation endpoint. This implementation makes no claim for that range.

## Unsupported states and remaining work

Invalid finite-value, mass, volume and temperature inputs return typed errors.
States without a feasible equilibrium return `UnsupportedEquilibrium`.
Saturated liquid and vapor endpoints are supported.
Compressed liquid and superheated vapor remain unsupported.
This includes fully liquid heated cavities that require finite compressibility.
The solver never clamps temperature, energy or phase amounts into the table.

Ice, carrier air, hot fire/water mixtures and elemental phase families remain
outside this reference. It does not satisfy CUR-07 or CUR-09.
It does not establish FIRE-A03, FIRE-A12 or FIRE-PRECONDITIONS.
Air/steam mixtures must use vapor partial pressure rather than total pressure.

The next E09 deliverable is bounded single-phase water closure.
Air/steam mixture closure, finite ventilation, transport/conduction coupling,
chamber topology, broader current-material domains and GPU parity follow under
the [M4 contract](plans/fluid-gpu-redesign/plan.md#thermodynamics-and-chamber-closure).
All remain required before M4 or E09 can be validated.

## Verification

[`water_vessel.rs`](../engine/crates/sim-cpu/tests/water_vessel.rs) checks phase
amounts, actual geometry, source heat, rejected candidates and repeated closure.
Property unit tests compare all published verification points for the selected
IF97 equations. Table tests check every interval at three interior positions.
They report interpolation error separately from the nonlinear closure residual.
They also compare 610, 1220 and 2440 intervals to test refinement.

Run focused checks from `engine/`:

```sh
cargo test --locked -p particle-sim-cpu water:: -- --nocapture
cargo test --locked -p particle-sim-cpu --test water_vessel
```

Run `mise run ci` from the repository root before delivering engine changes.
