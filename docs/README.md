# Particle Foundry handbook

**Continue project work from this page.** Use the same starting point in GitHub,
your editor or the compiled browser handbook.

The normal sandbox uses the TypeScript engine. `/gpu.html` runs a limited
experimental Rust/GPU scene. P1 must migrate the complete existing sandbox before
the default can change. P2-P9 remain deferred.

## Continue project work

1. Read [current status](STATUS.md) for the selected P/M/E, delivery state and open gaps.
2. Check the working tree, relevant Git history, associated PR and live Linear issue. Preserve unrelated changes.
3. Find the assigned item in the [work reference](remaining-work.md). Read its controlling contract and acceptance checks.
4. Complete that bounded item. Use [project structure](project-structure.md) for ownership and [AGENTS.md](../AGENTS.md) for working rules.
5. Record verification in the owning evidence article. Update the assigned issue and status when applicable. Pause at the project checkpoint.

P means phase, M means milestone and E means engineering item.
[Phases explained in Danish](PHASES.md) describes the checkpoints.
The selected work track can differ from the manifests' next incomplete item.
A plan or backlog does not authorize additional implementation.

## Read the complete handbook in a browser

Run from the repository root with the existing documentation renderer installed:

```sh
mise run docs:build
mise run docs:serve
```

Open <http://127.0.0.1:8000>. The build also works offline: open
`artifacts/docs/index.html` directly. All documentation chapters share one HTML
page. Navigation and search help locate a topic. Linked source files, manifests
and evidence remain available in the accompanying `files/` directory.

[Documentation maintenance](documentation.md) describes prerequisites, output,
checks and how to share the complete set.

## Find the controlling reference

Read the topic for your task. The tables below are a reference catalogue, not a
required reading sequence.

### Build and operate

| Need                            | Reference                                                                               |
| ------------------------------- | --------------------------------------------------------------------------------------- |
| Install and run the game        | [Root quick start](../README.md)                                                        |
| Find commands and pinned tools  | [mise.toml](../mise.toml)                                                               |
| Locate code and module owners   | [Project structure](project-structure.md), [engine rules](../engine/AGENTS.override.md) |
| Check CI and merge requirements | [GitHub policy](../.github/README.md)                                                   |
| Deploy or investigate errors    | [Deployment](deployment.md), [error reporting](error-reporting.md)                      |

### Understand current behavior

| Subject                                     | Reference                             |
| ------------------------------------------- | ------------------------------------- |
| Controls, materials and tools               | [Existing sandbox](legacy-sandbox.md) |
| State, units and conservation               | [Physical model](model.md)            |
| Water, carrier air and phase closure        | [Thermodynamics](thermodynamics.md)   |
| Ignition and burning                        | [Combustion](combustion.md)           |
| Existing elemental models and future roster | [Elements](elements.md)               |

### Implement an assigned migration item

| Need                                            | Controlling reference                                                                                                                                       |
| ----------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Work mapping and dependencies                   | [Work reference](remaining-work.md), [phase order](roadmap.md)                                                                                              |
| Active release scope                            | [Current-sandbox checklist](plans/rust-wasm-migration/current-sandbox-scope.md)                                                                             |
| E00-E14 tasks and exit gates                    | [Migration plan](plans/rust-wasm-migration/plan.md)                                                                                                         |
| Fluid equations, GPU stages and gameplay target | [Fluid/GPU plan](plans/fluid-gpu-redesign/plan.md), [shared P1 target](plans/fluid-gpu-redesign/plan.md#final-gameplay-target-clarified-on-13-october-2026) |
| Fire integration and corrected requirements     | [Fire plan](plans/fire-combustion/plan.md), [fire acceptance](validation/fire-combustion.md#corrected-gates)                                                |

### Check design and acceptance

| Need                                    | Reference                                                                                                                                                                                                                                                                              |
| --------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Stack decision and integration boundary | [ADR-001](architecture/adr-001-rust-wasm-wgpu.md), [engine boundary](architecture/engine-boundary.md)                                                                                                                                                                                  |
| Planned composition contracts           | [Matter model](architecture/matter-model.md)                                                                                                                                                                                                                                           |
| Required qualification                  | [Acceptance](validation/acceptance.md), [backend migration](validation/backend-migration.md)                                                                                                                                                                                           |
| Executed migration results              | [E00](validation/p1-m0.md), [E01](validation/p1-m1.md), [E02](validation/p1-m1-e02.md), [E03](validation/p1-m1-e03.md), [E05](validation/p1-m2-e05.md), [E06](validation/p1-m2-e06.md), [E07](validation/p1-m3-e07.md), [E08](validation/p1-m3-e08.md), [E09](validation/p1-m4-e09.md) |
| Record a new run                        | [Evidence template](templates/phase-evidence.md)                                                                                                                                                                                                                                       |

### Consult deferred plans and history

| Subject                                       | Reference                                                                                                                                                                                                             |
| --------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Deferred pressure waves and breakage          | [P2 plan](plans/pressure-waves-breakage/plan.md)                                                                                                                                                                      |
| Deferred materials and chemistry              | [P3-P5 plan](plans/materials-and-chemistry/plan.md)                                                                                                                                                                   |
| Deferred nuclear work                         | [P6-P9 plan](plans/nuclear-simulation/plan.md)                                                                                                                                                                        |
| Historical audits and recovery                | [Archive index](history/README.md), [source review](validation/source-review-2026-09-21.md), [fire review](evidence/fire-review-2026-09-23/FIRE_REVIEW.md), [dated recovery record](reference/recovery-2026-09-27.md) |
| External references and documentation changes | [Sources](sources.md), [change log](CHANGELOG.md)                                                                                                                                                                     |

## Authority and status

| Information             | Owner                                                                                                                                                                                                                                          |
| ----------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Current behavior        | Checked-out source, including uncommitted changes, and focused regression tests. Topic articles explain behavior and limits.                                                                                                                   |
| Accepted design         | Architecture decisions and the contract named by the controlling plan. Accepted intent can remain unimplemented.                                                                                                                               |
| Planned work and status | [Current scope](plans/rust-wasm-migration/current-sandbox-scope.md), [phase manifest](data/development-phases.json), [engineering manifest](data/engine-migration-work.json), [fire manifest](data/fire-combustion-work.json) and live Linear. |
| Verification            | Dated evidence with revision, commands, execution scope and limitations. Required gates alone do not establish a pass.                                                                                                                         |
| Historical claims       | Preserved archives and audit records for their recorded snapshot. They do not override current instructions.                                                                                                                                   |

Inspect code, tests and contracts when sources disagree. Keep local checks,
hosted CI, merged code and deployed behavior distinct. A browser smoke result
with `gpu.status: unavailable` does not prove GPU simulation or rendering.

## Evidence and history

Validation articles own their measurements. Historical source manifests identify
original audit snapshots. They do not freeze today's working tree or require a
replacement source ZIP. Preserve `history/`, `evidence/` and original audit files
byte-for-byte.

## Maintain the knowledge base

Update the owning article with a behavior, path or command change. Keep changing
facts in one place and link to them. [STATUS.md](STATUS.md) owns the short Danish
delivery overview. The [documentation guide](documentation.md) owns authoring
rules, browser compilation and checks.
