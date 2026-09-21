# Project structure

The project groups code by responsibility. Source and test filenames use lowercase
kebab-case; tests use the `.test.ts` suffix. `README.md` stays at the repository root
with the package, TypeScript, Vite, and Biome configuration files.

```text
index.html                  Vite HTML entry point
src/
  main.ts                   Browser startup and animation loop
  app/                      DOM controls and viewport navigation
  materials/                Catalogue, validation, IDs and reference data
  physics/                  Physical solvers and reaction rules
  rendering/                Canvas rendering, field maps and visual effects
  scenes/                   Initial scene construction
  simulation/               World state, orchestration, clock and diagnostics
  styles/                   Application styles
  tools/                    Painting and editing tools
tests/
  app/                      DOM controls and pointer/viewport tests
  integration/              Cross-system scenarios and model regressions
  materials/                Catalogue and element-reference tests
  physics/                  Solver and reaction tests
  rendering/                Renderer and visual-effect tests
  simulation/               World state and clock tests
  tools/                    Brush and ignition-tool tests
  fixtures/                 Shared deterministic scene builders
  types/                    Compile-time catalogue checks
benchmarks/                 Reproducible performance workloads
docs/
  model.md                  Current physical model and planned extensions
  elements.md               Supported element models and limitations
  roadmap.md                Delivery sequence
  remaining-work.md         Active backlog and delivered history
  plans/                    Implementation proposals and their audit evidence
artifacts/                  Ignored local output; not application source
  archives/                 Source archives
  browser/                  Browser captures
  logs/                     Local check logs
  profiles/                 CPU profiles and reports
  snapshots/                Historical verification workspaces
```

## Module ownership

- `simulation/world.ts` owns mutable simulation state. `simulation/physics.ts`
  orders the solvers; `simulation/sandbox.ts` composes the public application API.
- `physics/` implements model behavior using world state and material definitions.
  Browser and DOM access belong in `app/` and `main.ts`.
- `app/controls.ts` coordinates input using a focused command/readings contract.
  `material-picker.ts` owns catalogue filtering and selection, `view-controls.ts`
  owns field toggles and legends, and `pointer-mapping.ts` contains pure coordinate
  conversion. The picker and view controls receive only the callbacks they use.
  `dom.ts` provides the shared required-element lookup with descriptive failures.
- `materials/definitions.ts` is the single material catalogue. Import its public
  API through `materials/index.ts` (the `materials` directory import). `ids.ts`,
  `physical-properties.ts`, and `presentation.ts` retain the existing focused
  re-export interfaces; they do not own duplicate catalogue data.
- `materials/queries.ts` owns catalogue-derived phase classification and motion
  profiles. Rendering, diagnostics, and physical solvers query material properties
  without importing the movement solver. Lookup arrays stay private; the shared
  motion-profile lookup is frozen.
- `rendering/` reads physical state. Its visual effects must not mutate occupancy
  or the physical energy budget.
- `tools/` handles user edits and their source accounting; `scenes/` builds initial
  conditions. Keep shared test scenes in `tests/fixtures/`.

Use relative imports and keep descriptive module names. Add a subdirectory when a
domain grows into multiple related modules; do not introduce generic `utils/` or
duplicate state to make an import shorter. The future CPU/GPU backend layout remains
specified in the [fluid redesign plan](plans/fluid-gpu-redesign/plan.md).

## Engineering conventions

- Prefer the simplest implementation for current requirements. Do not add future
  GPU interfaces, extension registries, generic frameworks, or inheritance trees
  before a concrete caller needs them.
- Group behavior that changes together. Compose small functions and focused
  contracts at UI/domain boundaries; consumers should require only operations they
  use. Implementations must preserve their contract's guarantees and failure modes.
- Keep material knowledge in the catalogue and simulation state in its existing
  owner. Centralize repeated rules when callers share the same meaning, not merely
  similar-looking code. Add catalogue entries for existing behavior rather than
  modifying every consumer.
- Prefer readonly input types and detached readings. Keep mutable hot-path arrays
  inside the simulation boundary and private caches inside their owning module;
  do not create a second authority to simplify UI access.
- Validate public input before indexing or mutating. `World.indexAt()` is the
  shared safe-integer and bounds check for cell readings and dynamic-cell commands.
  Brush commands intentionally have a different contract: valid outside strokes
  are ignored. Preserve these documented differences.
- Use explicit domain types and readable control flow. Repeated setters and reset
  commands should have predictable results; physics steps and paint sources remain
  intentionally state-changing operations.
- Diagnose failures with contextual errors and existing physical diagnostics.
  Avoid per-cell logging and hidden state repair. Test behavior at module boundaries
  and add regression tests for demonstrated defects.
- Keep refactors scoped, preserve unrelated working changes, and run the relevant
  checks. Record planned capabilities separately from implemented behavior.

## Checks and historical evidence

Run `bun run lint`, `bun run check`, and `bun run build` from the repository root.
`bun run test` explicitly discovers `tests/`; Biome excludes `artifacts/` so old
verification snapshots cannot introduce duplicate tests or nested configuration.

CPU profile reports, audit output, and source hashes preserve their original paths
and line numbers as historical evidence. They are not rewritten to imply that a
measurement was repeated after the directory change. The executable audit harness
is preserved, and its build instructions in the fluid plan target the new paths.

Local artifacts were moved without deleting their contents. They are ignored by
Git; durable project documentation and numerical audit evidence stay under `docs/`.
