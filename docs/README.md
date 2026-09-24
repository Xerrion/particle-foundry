# Particle Foundry developer documentation

**Start at [START_HERE.md](START_HERE.md).** First task: P1 / M0 / E00. The selected architecture keeps the TypeScript frontend and introduces a Rust/WASM engine with wgpu-managed WGSL compute and direct rendering. All implementation phases remain planned.

| Document | Purpose |
| --- | --- |
| [Developer entrypoint](START_HERE.md) | Reading order, copy-paste brief and first tasks |
| [ADR-001](architecture/adr-001-rust-wasm-wgpu.md) | Selected stack, rationale, costs and explicit non-goals |
| [Engine boundary](architecture/engine-boundary.md) | State authority, async commands/probes, ABI, renderer and recovery |
| [Rust/WASM migration work](plans/rust-wasm-migration/plan.md) | E00-E14 dependencies inside P1 and execution ownership for P2-P9 |
| [Roadmap](roadmap.md) and [active work](remaining-work.md) | P1-P9 delivery gates and R-work-item tracking |
| [Fire integration](plans/fire-combustion/plan.md) and [fire acceptance](validation/fire-combustion.md) | FIRE-W00-W08 tasks, FIRE-A01-A14 gates and preserved audit traceability |
| [Source review](validation/source-review-2026-09-21.md) | Static source findings, exact locations and evidence limits |
| [Project structure](project-structure.md) | Existing modules, proposed Rust/WGSL owners and migration map |
| [Physical model](model.md) | Legacy versus planned numerical/physical contracts |
| [Elements](elements.md) and [matter model](architecture/matter-model.md) | Eight legacy elemental models, all 118 identities and composition |
| [Fluid/GPU plan](plans/fluid-gpu-redesign/plan.md) | M0-M7 numerical reference, GPU implementation, integration and promotion |
| [Pressure/breakage](plans/pressure-waves-breakage/plan.md) | P2 W0-W4 |
| [Materials/chemistry](plans/materials-and-chemistry/plan.md) | P3-P5 C0-C9, core 40 and expansion to 83 |
| [Nuclear simulation](plans/nuclear-simulation/plan.md) | P6-P8 N0-N8, decay, radiation, fusion/fission and 118 coverage |
| [Acceptance](validation/acceptance.md) and [backend migration](validation/backend-migration.md) | Numerical, runtime, product and measurement gates |
| [Sources](sources.md), [change log](CHANGELOG.md), [history](history/README.md) | Provenance, revisions and unchanged historical evidence |

Planning manifests: [phases](data/development-phases.json), [engine work](data/engine-migration-work.json), [element roster](data/element-roadmap.json), [roster schema](data/element-roadmap.schema.json), [source review](data/source-review-manifest.json), [fire work](data/fire-combustion-work.json) and [fire evidence provenance](data/fire-review-manifest.json). Planning entries are not runtime material properties or evidence of implementation; the fire evidence manifest identifies preserved historical observations.

From the extracted package or repository root:

```sh
python docs/scripts/validate-docs.py
```

To additionally verify the exact read-only source archive used for the review:

```sh
python docs/scripts/validate-docs.py --source-archive "path/to/src(3).zip"
```

The validator checks documents, manifests, local links, generated HTML freshness and preserved evidence. It does not compile the game or validate numerical physics/GPU performance. The source upload was not modified. The earlier static review and the separately imported 12-check fire audit are identified as historical evidence; this revision reruns only documentation/package validation, not the game or audit. See [package-checks.txt](validation/package-checks.txt) for this revision's documentation-check output.
