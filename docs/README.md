# Particle Foundry knowledge base

Use this index to retrieve the knowledge needed for the current task. Each topic
should explain what is implemented, why it works that way, where the code and
tests live, and what remains uncertain. Read the relevant topic and follow its
links when needed; there is no repository-wide required reading sequence.

[AGENTS.md](../AGENTS.md) owns working rules. [The root README](../README.md)
owns setup, and [mise.toml](../mise.toml) owns executable commands and tool versions.
The running application is TypeScript in `web/`. `engine/` contains the Rust/WASM
bootstrap, portable contracts and the locally validated E05-E06 f64 CPU fluid
reference. E03 adds an experimental browser session contract and one generated
catalogue projection. E06 owns conservative phase and marker transport,
compatible momentum, viscous shear and bounded atomic substeps. Its named
closed-scene and dam-refinement checks pass locally. E07 adds a limited
Rust/WGSL GPU stage graph with native and browser qualification. E08 has an
experimental 480 x 270 browser scene that directly renders committed GPU
state. Bounded paint, inspection, pause and reset have passed isolated browser
checks, and a local Chrome run completed 60 sustained ticks. M3 passed local
checks; the broader current-sandbox migration remains incomplete.
E09 now has an isolated saturated-water CPU reference. It closes phase amounts
from mass, internal energy and actual volume. [Thermodynamics](thermodynamics.md)
owns its bounded domain; [E09 evidence](validation/p1-m4-e09.md) records its limits.
The live sandbox still uses TypeScript. See
[E05 validation](validation/p1-m2-e05.md) and
[E06 validation](validation/p1-m2-e06.md) for the CPU reference results,
[E07 evidence](validation/p1-m3-e07.md) for the GPU stage checks,
[E08 evidence](validation/p1-m3-e08.md) for browser-scene results, and
[the dated recovery handoff](RESUME.md) for the earlier recovered commits.
A plan or reference entry is not evidence of implementation.

## Find knowledge by question

| Question | Start here | Read further when needed |
| --- | --- | --- |
| Where does behavior belong? | [Project structure](project-structure.md) | [Existing controls and sandbox](legacy-sandbox.md) |
| How does the simulation behave? | [Physical model](model.md) | [Combustion](combustion.md), [elements](elements.md) and the relevant source/tests |
| Why was this architecture chosen? | [ADR-001](architecture/adr-001-rust-wasm-wgpu.md) | [Engine boundary](architecture/engine-boundary.md), [matter model](architecture/matter-model.md) |
| What should an assigned migration item deliver? | [Active work](remaining-work.md) | Its controlling plan, dependencies and acceptance gates below |
| What supports a claim? | The topic's source/test links | [Evidence and history](#evidence-and-history), including the recorded snapshot and execution scope |

The user's task determines scope. The active delivery scope is the
[current sandbox migration](plans/rust-wasm-migration/current-sandbox-scope.md).
P2-P9 are deferred and require separate authorization; P1 completion does not
activate them. Reading a plan does not authorize implementing its backlog. Use
[the migration brief](START_HERE.md) for an assigned migration task; read later
phases only when their contracts constrain that task.

## Authority and status

| Knowledge kind | Authority | Interpretation |
| --- | --- | --- |
| Current behavior | Current source and focused regression tests, explained by topic articles | Implemented behavior can include known defects; a passing test covers only its assertions. |
| Accepted design | Architecture decisions and the contract named by the relevant plan | Selected intent can remain unimplemented. ADR-001 owns the stack; the engine boundary owns integration. |
| Planned work | [Current sandbox scope](plans/rust-wasm-migration/current-sandbox-scope.md), [phase manifest](data/development-phases.json), [engine work](data/engine-migration-work.json), [fire work](data/fire-combustion-work.json) and their controlling plans | Current sandbox scope owns the active release boundary. Manifests own status/dependencies; plans own work and exit gates within that boundary. Deferred phases require separate authorization. |
| Evidence | Source, tests or a dated run with provenance and limitations | Static inspection, executed regression tests, characterization and corrected acceptance are different claims. |
| History | Preserved archives, audit results and superseded descriptions | Valid only for the recorded snapshot; historical text does not override current instructions. |

When sources disagree, inspect the relevant code, test and contract and state the
discrepancy. Do not silently treat an implementation defect as intended design or
an accepted design as delivered behavior. Update status only with supporting
evidence. Generated HTML is a view of its canonical Markdown, not a second source.

## Current system

- [Project structure](project-structure.md): current module ownership, browser/engine boundary and proposed Rust placement.
- [Existing sandbox](legacy-sandbox.md): controls, tools, materials and current TypeScript implementation.
- [Physical model](model.md): units, state ownership, conservation, solver limitations and explicitly planned model changes.
- [Thermodynamics](thermodynamics.md): isolated E09 water closure, IF97 source, generated table, source heat and unsupported domains.
- [Combustion](combustion.md): ignition, generic fuels, explicit oxidation, tick ordering, known defects and regression coverage.
- [Elements](elements.md): eight documented legacy elemental models and a separately labelled future roster. The [roster manifest](data/element-roadmap.json) and [schema](data/element-roadmap.schema.json) are planning data, not runtime properties.

## Architecture and decisions

- [ADR-001](architecture/adr-001-rust-wasm-wgpu.md): selected stack, rationale, costs and non-goals.
- [Engine boundary](architecture/engine-boundary.md): planned state authority, commands, asynchronous probes, ABI, rendering and recovery.
- [Matter model](architecture/matter-model.md): planned elements, nuclides, species, materials and composition contracts.

## Planned work

[Human-readable phases and milestones](PHASES.md) explains the three active P1
checkpoints and retains P2-P9 as deferred plans. All other planning documents keep
their technical contracts and acceptance detail.

[Roadmap](roadmap.md) explains phase order; [active work](remaining-work.md) maps
backlog items to deliverables. [START_HERE.md](START_HERE.md) describes the migration
workflow. The phase manifest owns the next phase/milestone, and the engine-work
manifest owns the next engine work item; consult them rather than copying status here.

| Scope | Controlling knowledge |
| --- | --- |
| Active P1 current-feature migration | [Current sandbox scope](plans/rust-wasm-migration/current-sandbox-scope.md), [Rust/WASM work packages](plans/rust-wasm-migration/plan.md), [fluid/GPU equations and gates](plans/fluid-gpu-redesign/plan.md) |
| Combustion within P1 and P4 | [Fire integration](plans/fire-combustion/plan.md), [corrected FIRE-A acceptance](validation/fire-combustion.md#corrected-gates) |
| Deferred P2 pressure and breakage | [Pressure waves and material breakage](plans/pressure-waves-breakage/plan.md) |
| Deferred P3-P5 materials and chemistry | [Materials/chemistry](plans/materials-and-chemistry/plan.md) |
| Deferred P6-P9 nuclear work and expanded-product validation | [Nuclear simulation](plans/nuclear-simulation/plan.md), [acceptance](validation/acceptance.md), [backend migration](validation/backend-migration.md) |

## Evidence and history

- [Acceptance](validation/acceptance.md) and [backend migration](validation/backend-migration.md) specify required checks. Their existence does not establish a pass; use [phase evidence](templates/phase-evidence.md) to record actual results.
- [Static source review](validation/source-review-2026-09-21.md) and [source manifest](data/source-review-manifest.json) describe an uploaded source snapshot, not a fresh runtime check.
- [Fire characterization and corrected gates](validation/fire-combustion.md) links the [preserved report](evidence/fire-review-2026-09-23/FIRE_REVIEW.md) to future acceptance. [Fire provenance](data/fire-review-manifest.json) identifies the original files and execution scope.
- [Sources](sources.md) records external references; [change log](CHANGELOG.md) records documentation revisions; [history](history/README.md) identifies immutable archives and superseded assumptions.
- [Earlier package checks](validation/package-checks.txt) belong to their historical revision. Run current checks rather than quoting them as new results.

## Maintain the knowledge base

Use [combustion](combustion.md) as an example when a subject needs its own article.
Extend an existing topic before creating another file. Keep one coherent subject
per article, with descriptive headings, exact identifiers and ordinary Markdown links.

1. State purpose, backend/scope and status at the top. Date a source review or measurement only when it actually occurred; distinguish inspection from execution.
2. Explain behavior, units, invariants and exceptions. Keep rationale beside the relevant rule or link to its decision.
3. Link current source files and named symbols, plus the tests that exercise the claim. Links are checked for existence; symbol names and scientific correctness still need review.
4. Separate known defects, unknowns and future corrections. Link the controlling plan or evidence instead of duplicating its requirements, results or status.
5. Update the owning article in the same change as behavior or path changes, and keep this index discoverable. Preserve historical evidence byte-for-byte and regenerate affected HTML.

From the repository root, run `mise run docs:check`. The existing validator checks
authored Markdown/HTML, root guidance, local links/anchors, synchronized manifests,
generated HTML freshness and historical evidence integrity. It does not execute
application tests or establish physical correctness. Use the topic's verification
instructions for behavior and `mise run ci` for full repository checks.

Current source means the checked-out files, including uncommitted changes. Source
and test links resolve directly into the working tree; no source ZIP is needed.
Historical source manifests describe their original audit snapshot and must not
be used to freeze or validate the contents of today's source tree. For new
measurements, record the Git commit and working-tree changes using the
[evidence template](templates/phase-evidence.md).

`mise run docs:render` regenerates the GPU plan's HTML.
