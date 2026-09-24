# Fire evidence and corrected acceptance

**Revision:** 23 September 2026. **Status:** historical characterization imported; all corrected gates planned. Controlling work: [fire/combustion plan](../plans/fire-combustion/plan.md). Developer entrypoint: [START_HERE.md](../START_HERE.md).

## Evidence boundary

The [original report](../evidence/fire-review-2026-09-23/FIRE_REVIEW.md), [measured results](../evidence/fire-review-2026-09-23/fire-audit-results.json), [audit harness](../evidence/fire-review-2026-09-23/audit-fire.cjs), [runner](../evidence/fire-review-2026-09-23/run-review.py) and [source provenance](../evidence/fire-review-2026-09-23/provenance.json) are preserved byte-for-byte. [The import manifest](../data/fire-review-manifest.json) records their hashes and the retained original ZIP.

The review ran 12 headless characterization checks against `src(3).zip`, whose hash matches the existing 43-file static source manifest. F02 and F08 used the complete physics tick; the other ten checks isolated subsystems. The report records TypeScript 5.8.3 and Node v22.16.0. Those are prior-run facts, not the required toolchain for the redesigned engine or tests executed in this documentation revision.

**A passing characterization harness proves that an observation was reproduced, including an undesirable behavior. It does not mean fire is correct.** New assertions use FIRE-A IDs. Preserve F IDs and original results instead of inverting the old harness and silently reusing its evidence.

The update does not rerun the fire checks, compile the application, run Rust/WASM code, validate rendering, or execute a GPU. The corrected implementation remains future work.

## Finding-to-work traceability

| Historical checks | Observed scope and interpretation | Corrected work / gates |
| --- | --- | --- |
| F01, F02; F03 control | Closed boundary flag still replenished O2; approximately 100 J was released in one tick from a zero-O2 setup. An explicit stone enclosure stopped burning. Two different sealing semantics, not proof that every sealed cavity fails. | FIRE-W01-W03; FIRE-A01, FIRE-A03 |
| F04; F12 control | Generic combustion spent O2 inventory without consuming O2 material mass or forming products. About 18.87 J was released with near-zero tracked scalar energy error. The explicit H2/O2 control retained mass/momentum/energy. | FIRE-W03-W05; FIRE-A05, FIRE-A06, FIRE-A11 |
| F07, F08 | A cold FIRE neighbor ignited cold H2/O2 at 22 C, including the full pipeline; approximately 9.98 J released. | FIRE-W03; FIRE-A04 |
| F05, F10 | Generic flames were heated-air proxies with zero chemical inventory. Sealed expired smoke preserved mass/heat and added no O2, but lost its smoke identity. | FIRE-W04-W06; FIRE-A09, FIRE-A13 |
| F06 | A tiny water neighbor stopped burning in a reaction-only call without a heat or phase debit. The separate thermal solver was deliberately excluded; this is not a full-tick cooling result. | FIRE-W02, FIRE-W04; FIRE-A08, FIRE-A12 |
| F09 | Hot hydrogen could not use finite O2 in enclosed ordinary air. This restriction was disclosed in the legacy material guide, unlike generic-fuel behavior. | FIRE-W03; FIRE-A02 |
| F11 | Ignition brush added ledgered external heat, preserved air mass, supplied no chemical fuel and expired without smoke. A useful control to retain, not validation of sustained combustion. | FIRE-W04, FIRE-W06; FIRE-A07 |

All numerical observations above are from [the preserved JSON](../evidence/fire-review-2026-09-23/fire-audit-results.json). They are not acceptance targets for the corrected model. Source locations and the inspection of carbon/sulfur oxygen rules are in [the original report](../evidence/fire-review-2026-09-23/FIRE_REVIEW.md). Locations refer to that source snapshot, not line numbers after a future refactor.

## Baseline and reproduction policy

At M0 verify source fingerprints and inspect the real repository scripts/lockfile before execution. The retained runner is historical tooling; review its arguments, temporary compilation, dependencies and output paths before using it. Do not overwrite the imported files or install its old toolchain into the application as a side effect. New runs belong under a fresh `artifacts/validation/p1-m0/<run-id>/fire/` directory, with exact commands and source hashes.

Keep the two full-pipeline bug fixtures alongside isolated cases and positive controls. A newer checkout may already fix or change them. Record that difference honestly. Tests for the Rust reference assert corrected behavior; an expected-to-fail baseline must not remain a permanently skipped promotion gate.

## Corrected gates

The canonical [fire work manifest](../data/fire-combustion-work.json) separates tasks, fixtures and aggregate gates. All statuses and evidence arrays are initially planned/empty. Fixture descriptions below are proposed requirements, not newly measured results.

| Gate | Parent milestone | Required scope |
| --- | --- | --- |
| FIRE-PRECONDITIONS | P1/M4, E09 | FIRE-A03 and FIRE-A12 in passive-transport/preflight mode; appropriate hot-scene properties before coupling |
| FIRE-M6 | P1/M6, E11-E12 | FIRE-A01 through FIRE-A13 on the declared supported reference/GPU integration paths, with applicable controls |
| FIRE-M7 | P1/M7, E13-E14 | FIRE-A01 through FIRE-A14 plus the existing GPU-M7 browser, numerical, transfer/performance and recovery requirements |

A03 and A12 must be rerun with enabled reactions at M6. A physical fire scene cannot be promoted merely because passive gas transport passed M4. Pure gas scenes may have no condensed fuel, water or soot, but any claimed wood/oil/plant, water-suppression or smoke capability requires its corresponding fixtures; record feature-level eligibility rather than deleting gates.

### FIRE-A01: Sealed boundaries cannot create oxidizer

**First milestone:** P1/M6. **Historical context:** F01, F02, F03.

Run a closed-edge zero-O2 chamber and an explicitly wall-enclosed zero-O2 chamber with hot fuel. No oxidation or new O2 is allowed without an explicit source; nonoxidative heating or declared pyrolysis is distinguished from combustion.

**Measure:** Before/after component masses, oxidation extent, chemical and thermal energy, and zero boundary/source fluxes; run both reaction-only and complete-substep variants.

### FIRE-A02: One finite oxidizer inventory for every supported fuel

**First milestone:** P1/M6. **Historical context:** F04, F09.

In sealed ordinary air and equivalent explicit gas mixtures, every enabled compatible fuel channel sees the same available O2. Consumption is finite, products accumulate, and extinction follows the declared model, not a requirement to consume every last molecule.

**Measure:** Compare equivalent compositions across legacy brush presets; log O2, residual fuel, products, reaction rate and extinction reason at equal physical times.

### FIRE-A03: Ventilation is local finite boundary transport

**First milestone:** P1/M4. **Historical context:** F01, F02.

Open and close a vent with chemistry disabled first, then with the supported burner. Only accepted face fluxes change the inventory. No instantaneous reachability-based interior reset of O2, pressure or energy is permitted.

**Measure:** Integrate signed inlet/outlet component, mass, momentum and energy fluxes; check a sealed baseline and transient transport/refinement, not just connectivity.

### FIRE-A04: Cold labels and stale burning flags cannot ignite

**First milestone:** P1/M6. **Historical context:** F07, F08.

Cold H2/O2 beside a cold or quenched former FIRE representation must not react merely because of that label or flag. A separate admissible hot-state or funded ignition control must react under the supported model.

**Measure:** Use the prior 22 C edge-case setup and a cooled-after-burning case through the complete stage graph; compare unchanged reactants to the positive ignition control.

### FIRE-A05: Reactant loss, products and energy reconcile

**First milestone:** P1/M6. **Historical context:** F04, F12.

A limited-reagent reaction converts consumed fuel and O2 into declared products, preserves excess reactants, and reconciles scalar mass plus constituent-element inventories. Debit the selected chemical-energy reference once; include momentum and energy partition.

**Measure:** Independent stoichiometric oracle; per-species deltas, atom/charge balance, residual reagent identity, and chemical/thermal/kinetic totals. A small scalar energy error alone is insufficient.

### FIRE-A06: Competing reactions cannot spend the same oxygen

**First milestone:** P1/M6. **Historical context:** F04, F09.

Two or more reaction sites share a limited oxidizer parcel. Resolve proposals before a single commit so all accepted reactant debits fit the available inventory. Failed proposals do not create products or heat.

**Measure:** Check nonnegative amounts, shared-resource reservations and product/source totals under alternative CPU traversal and GPU workgroup orders; document any fixed priority.

### FIRE-A07: The Fire brush is a funded ignition command

**First milestone:** P1/M6. **Historical context:** F11.

Apply the Fire brush to clean air and to supported fuel. It adds only the declared external heat/impulse, without replacing fuel, creating chemical inventory or making unbudgeted soot. Paused and retried commands apply exactly once.

**Measure:** Mass and constituent inventories, external energy ledger, command IDs and commit counts; repeat with visual effects disabled. Cosmetic expiration cannot mutate composition.

### FIRE-A08: Water suppression follows amount and thermal state

**First milestone:** P1/M6. **Historical context:** F06.

Compare no water, a small finite dose and a larger dose within the validated domain. Heat uptake, evaporation and supported mixing effects determine suppression; WATER adjacency alone is not an extinction switch.

**Measure:** Isolate the legacy reaction-only edge case from integrated tests. Record absorbed heat, phase amounts, vapor composition, temperature and extinction criterion; require convergence to the zero-dose case as dose tends to zero, not a universal monotonic flame response.

### FIRE-A09: Invisible smoke retains its physical inventory

**First milestone:** P1/M6. **Historical context:** F05, F10.

Cool or visually fade smoke/soot in a sealed chamber. Composition persists until a declared transport, deposition, reaction or external removal changes it. Rendering cannot turn it into ambient air or add oxygen.

**Measure:** Compare opaque and invisible render settings with identical physics seeds; track suspended and deposited inventories plus boundary losses through save/load where enabled.

### FIRE-A10: Burn rates converge in physical time

**First milestone:** P1/M6. **Historical context:** new requirement; no historical test result claimed.

Run equal accepted physical time with dt, dt/2 and a finer reference, including adaptive rejected/retried substeps. Integrated reaction extent and released energy converge under the declared tolerances; no legacy per-tick amount repeats on each substep.

**Measure:** Report accepted dt sequence, rejected attempts, cumulative fuel/O2/product changes, released energy and independent integration error at identical end times.

### FIRE-A11: Rust and WGSL agree beyond scalar totals

**First milestone:** P1/M6. **Historical context:** F04, F12.

Compare the corrected Rust f64 reference with WGSL/f32 for all enabled FIRE-M6 fixtures. Use independent balance/reference checks and order-variation tests; no bit-identical trajectory requirement.

**Measure:** Per-component norms, element totals, energy channels, reaction extents, nonnegativity, spatial aggregate outcomes, boundary fluxes and residuals at equal accepted times; hardware evidence identifies actual backend.

### FIRE-A12: High-temperature domain failures are explicit

**First milestone:** P1/M4. **Historical context:** new requirement; no historical test result claimed.

Preflight the full reactant/product/water domain and validate runtime boundary crossings. The initial water table ending at 623.15 K does not authorize legacy 450 C flame/water scenes. Extend validated property coverage or reject the combination without temperature clamps or partial commits.

**Measure:** Property-table/source hashes, temperature/pressure/composition ranges, boundary and out-of-domain fixtures, accepted/rejected state IDs and complete energy/amount rollback. Re-run with combustion at M6.

### FIRE-A13: Condensed fuel release is bounded and accounted

**First milestone:** P1/M6. **Historical context:** F04, F05.

For each promoted wood/oil/plant preset, a declared reduced pyrolysis or evaporation model moves bounded condensed mass into fuel vapor and declared residues. Gas combustion consumes that vapor; supported char/surface oxidation is a separate channel.

**Measure:** Track condensed fuel, volatile fuel, residue, O2, all products and heat of release/phase change. Include insufficient heat, exhausted fuel, O2-free heating and a sustained air-fed control; no automatic whole-parcel conversion to smoke.

### FIRE-A14: Transactions, persistence and visuals cannot alter combustion

**First milestone:** P1/M7. **Historical context:** new requirement; no historical test result claimed.

Test rejected substeps, command replay, component-capacity failure, save/load and device-loss rollback. Reactions commit once with their energy and products. Preserve the network version, soot/residue and approximation settings; no frame-loop full-world readback.

**Measure:** Before/after committed-state hashes and inventories, tick/epoch and event IDs, capability rejection, checkpoint notices, transfer-byte measurements, and render-on/off equivalence.

## Oracles, tolerances and reporting

Freeze test scales, physical times and precision-specific tolerances before tuning the implementation. Start from [the numerical acceptance policy](acceptance.md#tolerance-policy) and [the GPU plan](../plans/fluid-gpu-redesign/plan.md#4-acceptance-matrix); select explicit absolute floors for tiny inventory tests rather than dividing by zero initial O2. Do not invent a passing tolerance from the observed error after a run.

Chemical definitions require exact atom/charge balance at the definition level for declared compositions. Numerical mass/energy/species balance is tested within the selected floating-point and table/interpolation error budget. Do not require each chemical species to remain constant during a reaction: reconcile its change to stoichiometric production/consumption plus boundary/source terms. During ordinary chemistry, constituent-element and isotope totals remain conserved under their declared representation.

Use analytic limiting-reagent checks for the simplest channels, independent source/flux sums, nonnegativity, failed-step rollback, and dt/grid refinement. CPU/GPU agreement alone is not an independent physical oracle. Global totals alone can conceal spatially incorrect burning or the F04 composition inconsistency.

For a sealed test, reconcile initial inventory plus explicit sources with final reactants, products, residues and any physical deposited/escaped population. For open tests include signed boundary fluxes. Report thermal/internal, chemical, kinetic and gravitational energy, pressure work, source inputs, physical radiation losses if enabled, and numerical error separately. Reference energies must not be counted twice.

Evidence must identify model/network/property versions, all active components, pseudo-material approximations, grid/scale, initial conditions, boundary types, accepted dt sequence and physical time, backend/precision, exact commands, measured errors, tolerances and pass/fail/unrun outcomes. Hardware comparisons include browser/adapter, workgroup configuration, transfer bytes and probe age. Use [the phase evidence template](../templates/phase-evidence.md).

## Promotion decision

Record per-scene capability evidence for the burner, closed chamber, vent, water suppression, ignition-only tool and persistent smoke scenarios actually supported. M7 cannot use the historical F01-F12 report as a substitute for FIRE-A evidence. Basic O2/product/ignition fixes are P1 requirements; P4 extends fidelity and channels without reopening those defects.

No mandatory FPS multiplier or claim of realistic real-world fire prediction is added by this update. A visually convincing fire remains a bounded simulation with explicit calibration, exclusions and numerical evidence.
