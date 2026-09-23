# Source review for the Rust/WASM migration

**Review:** static inspection of `src(3).zip`, 21 September 2026. **Application source modified:** no. **Builds, game tests, benchmarks and GPU runs:** not performed.

The uploaded archive contains 43 files under `src/`. It does not include repository metadata, package/build configuration, tests, benchmark harnesses or Rust tooling. Their absence from this archive does not prove they are absent from the real repository. M0 must inspect the full checkout.

The [source manifest](../data/source-review-manifest.json) records the archive SHA-256, every file hash and line count, and exact observation locations. Source paths below refer to this upload, not to historical audit paths. No source files are included in the returned documentation package.

## Source-to-work traceability

| ID | Source observation | Exact locations in supplied source | Planned response |
| --- | --- | --- | --- |
| SRC-01 | Existing browser shell and grid | `src/main.ts:1-25` | Keep TypeScript controls, viewport, DOM and styling; adapt startup instead of rewriting the application. |
| SRC-02 | Canvas 2D is acquired before engine selection | `src/main.ts:7-20`; `src/rendering/renderer.ts:119-198` | Choose a renderer before context acquisition; a GPU scene needs a wgpu-owned surface and direct GPU rendering. |
| SRC-03 | Synchronous composition and reads | `src/simulation/sandbox.ts:17-57`; `src/simulation/sandbox.ts:83-116`; `src/app/controls.ts:22-36` | Use the narrow facade as the migration seam, but replace fresh synchronous probes with asynchronous tick/epoch-stamped samples. |
| SRC-04 | Current pressure refresh call graph | `src/simulation/physics.ts:56-98`; `src/physics/gas-dynamics.ts:184-185` | Four unconditional derivations per normal physics tick: three direct calls and one inside gas.step; a fifth is conditional on ventOpenEdges(). Commands can add more. |
| SRC-05 | Per-grain overlapping outlet searches | `src/physics/motion.ts:65-127` | One settling attempt can traverse a connected liquid region twice. Replace the algorithm through batched topology and conservative accommodation; a language port preserves the search pattern. |
| SRC-06 | Multiple motion mechanisms share world state | `src/simulation/physics.ts:63-98`; `src/physics/fluid-solver.ts:1-114` | Give each new-model scene exactly one conservative transport owner; keep legacy scenes separate. |
| SRC-07 | Byte material IDs and Float64 parcel arrays | `src/simulation/world.ts:36-88`; `src/simulation/world.ts:123-147` | Preserve legacy IDs in conversion but introduce distinct wider stable material/species/nuclide IDs and compact scene-local mappings. |
| SRC-08 | Legacy energy has mixed representations | `src/simulation/world.ts:182-223`; `src/simulation/diagnostics.ts:6-19`; `src/simulation/diagnostics.ts:29-58`; `src/simulation/physical-scale.ts:1-11` | world.energy is parcel enthalpy in J; chemicalEnergyKj and displayed diagnostic totals are kJ. Convert units and thermodynamic meaning explicitly, not by renaming fields. |
| SRC-09 | Frame scheduler times synchronous work | `src/main.ts:52-94`; `src/simulation/simulation-clock.ts:1-52` | Separate submitted work from accepted simulated time and GPU completion; CPU submission time cannot replace measured completed-step timing. |
| SRC-10 | Catalogue and element reference already exist | `src/materials/definitions.ts:314-739`; `src/materials/element-reference.ts:10-130` | Keep a single authored catalogue and generated cross-language projections. The supplied definitions cover eight distinct elemental identities; reference names alone do not validate materials. |
| SRC-11 | CPU graphs and tools are concrete migration surfaces | `src/physics/electricity.ts:1-125`; `src/tools/brush.ts:1-229` | Move bounded circuit solving into Rust CPU code; send compact commands and sparse energy transactions, not a mirrored full world. |
| SRC-12 | Synchronous diagnostics scan the world | `src/simulation/diagnostics.ts:29-65`; `src/main.ts:85-91` | Replace full-world CPU reads in GPU mode with GPU reductions and cached asynchronous diagnostics. |

## Pressure count clarification

`physics.step()` calls pressure derivation directly at lines 57, 59 and 96. Its call to `gas.step()` at line 63 reaches an additional derivation at `gas-dynamics.ts:185`. Therefore the normal path has **four unconditional** derivations, with **a fifth conditional** derivation after edge venting at `physics.ts:97`. Brush and boundary operations can issue additional refreshes outside this step.

The earlier shorthand of four refreshes misses the possible fifth. Static call counting does not show that every refresh is removable. M1/M2 must establish which topology, mass, energy and coefficient versions invalidate each derived field before eliminating repeated work.

## Existing data, not a blank project

`main.ts:13-14` fixes the current grid at 480 x 270, or 129,600 cells. The existing material definitions contain entries with eight distinct atomic numbers: 1, 2, 6, 7, 8, 16, 26 and 29. Several have multiple phase entries. `element-reference.ts:12-129` already lists all 118 identities. This confirms catalogue structure, not scientific validation of each material. Preserve the exact 40/83/103/118 expansion roster and distinguish legacy implementation from new-backend support.

`world.energy` uses J for legacy parcel enthalpy; `chemicalEnergyKj` is kJ, and diagnostics convert J to kJ for display. The unit conversion is distinct from changing H to U. The new internal-energy convention must define pressure/volume/reference-state accounting and a conversion report.

The source has 2D rendering, synchronous readings and seeded state. It contains no `.rs` or `.wgsl` files in this upload. This does not establish whether a separate branch or unprovided part of the repository already contains migration work.

## Three separate evidence classes

| Evidence | What is available | What may be claimed |
| --- | --- | --- |
| This source review | Supplied source bytes, line locations and SHA-256 inventory | Static control flow, data structures and migration touchpoints |
| Retained audit | Unmodified harness and 35 historical results, 24 pass and 11 fail | Results only for its recorded historical state |
| Quoted performance findings | User-supplied approximately 79 ms ambient, 114 ms pool, 479 ms sand/water per headless tick; preceding plan mentions 58.89 percent inclusive outlet-search profile | Historical prioritization clues, not fresh measurements for this upload |

The original benchmark logs/profile files were not supplied. Do not put the quoted values in a chart as a new measured baseline or attribute them to the user's browser/GPU. Source organization and hashes differ from the historical audit, so preserve that audit unchanged and run a new baseline at M0.

## Implications for the plan

Use [ADR-001](../architecture/adr-001-rust-wasm-wgpu.md) for the stack decision, [the engine boundary](../architecture/engine-boundary.md) for async/state contracts, and [migration work packages](../plans/rust-wasm-migration/plan.md) for the implementation order. The [GPU plan](../plans/fluid-gpu-redesign/plan.md) remains authoritative for numerical gates. No milestone has been marked implemented by this review.
