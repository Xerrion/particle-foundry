# Particle Foundry development guide

## AI workflow

Load `i-have-adhd`, `engineering-philosophy`, `asd-ste100` and `humanizer` before work.
Read only the engineering references required for the assigned activity.
Use `asd-ste100` for instructions and responses. Use `humanizer` for human-readable prose.
Search installed skill locations if a required skill is missing. Report any unresolved gap.

1. Establish the current state. Inspect the branch, working-tree changes and relevant Git history.
   For continued work, check the associated PR and live Linear issue when they exist.
   Preserve unrelated changes. Read `engine/AGENTS.override.md` before editing `engine/`.
2. Bound the task. Use `docs/README.md` to find the controlling topic, contract and acceptance checks.
   Identify one deliverable and its dependencies. A plan or backlog does not authorize additional work.
   For migration work, use the current-sandbox checklist and the assigned engineering item.
3. Implement the bounded change. Keep one owner for each state and module responsibility.
   Run focused checks during implementation. Update the owning article when behavior, paths or commands change.
   Record independent improvements separately unless they are necessary for the assigned outcome.
4. Verify the final state. Inspect the complete diff, including untracked files and generated changes.
   Run the applicable checks below. Record the revision, commands, results and unavailable hardware checks.
   Investigate review findings and verify each correction before delivery.
5. Deliver and record progress. Report changed behavior, verification, limitations and the delivery state.
   Update the assigned Linear issue with evidence and the PR link when applicable.
   For migration sequences, pause after each project checkpoint and wait for authorization to resume.

When recovering lost sessions, inspect Git history, worktrees, stashes and PRs before choosing a starting point.
Before rebasing recovered work, preserve a Git bundle and local-change snapshots in ignored `artifacts/`.
Exclude credentials from snapshots. Use the current project layout when integrating old commits.
Record a short handoff with the branch, commit, dirty state, completed checks and next bounded action.

Use subagents only when the user or applicable instructions authorize delegation.
Give each agent a bounded task, file ownership and required evidence.
Tell agents that they share the checkout and must preserve other agents' changes.
Keep dependent edits sequential and review the combined diff before delivery.

## Communication and tracking

- Lead user updates with the current result or next action. Keep instructions short and concrete.
  Use numbered steps for sequences. Ask one concise question when missing information blocks a material decision.
- Use plain language in `docs/PHASES.md` and user-facing project summaries.
  Keep other technical documents precise and suitable for agent use.
  Do not use long dashes as punctuation.
- Use checkpoint names when speaking to the user. `P` means phase, `M` means milestone and `E` means engineering item.
  These are repository identifiers, not a Linear or Jira convention.
- Track migration work in the existing
  [Linear project](https://linear.app/xerrion/project/particle-foundry-move-the-existing-sandbox-to-rustgpu-b424bc2ac381).
  Check live status and dependencies before changing an issue. Preserve assignee, labels, priority and one-week cycle planning.
  Post updates at meaningful progress points, blockers, review fixes and checkpoint completion.
- Keep evidence and status aligned. Distinguish local verification, a pushed PR and merged `main`.
  Mark an issue Done only when its acceptance checks pass. Create or restructure backlog items only within authorized planning work.

## Security and Git

- NEVER hardcode secrets or commit `.env`, `.env.local` or other credential files.
  MUST use environment variables or a secret manager for sensitive values.
  NEVER log credentials, tokens, passwords or personally identifiable information.
- Ask permission before installing a missing tool or dependency. Existing authorization does not require a second confirmation.
- Work on feature branches. Preserve unrelated tracked and untracked changes.
  Use small, atomic commits with conventional prefixes such as `feat:`, `fix:`, `docs:` and `refactor:`.
  NEVER commit a change that knowingly breaks existing tests.
- Use `gh` for GitHub operations. Follow the [merge policy](.github/README.md).
  Create commits and draft PRs when the assigned task includes delivery.
  Passing checks do not authorize a merge or deployment.

## Project map

Particle Foundry is a browser falling-sand game. TypeScript owns the browser interface.
The default sandbox uses the legacy TypeScript engine. Rust owns the independent f64
CPU fluid reference and the experimental GPU engine. The GPU path uses WASM, wgpu
and WGSL, with direct rendering from GPU state at `/gpu.html`.
The experimental scene covers limited liquid, carrier-gas, wall and marker behavior.
It does not cover the complete existing sandbox. The default grid is 480 x 270 cells.

P1 migrates the existing sandbox, including its eight elemental models, tools and interactions.
The default switch requires complete feature coverage and recovery evidence.
Use the [migration brief](docs/START_HERE.md) and
[current-sandbox checklist](docs/plans/rust-wasm-migration/current-sandbox-scope.md) for assigned work.
Use the work manifests and live Linear project for status. Keep test results in their owning validation articles.

| Project checkpoint | Required outcome |
| --- | --- |
| Working GPU scene | Prove the Rust fluid reference and a matching experimental browser GPU scene. |
| Existing sandbox features | Migrate every required current material, interaction, circuit, scene, tool and control. |
| Verified default switch | Pass complete coverage, browser/device, performance, snapshot and recovery gates before promotion. |

| Location | Responsibility |
| --- | --- |
| `web/src/app/`, `web/src/main.ts`, `web/src/styles/` | Browser startup, DOM, controls, viewport, clock and CSS |
| `web/src/engine-client/` | Browser-facing simulation entrypoint; currently delegates to the legacy backend |
| `web/src/legacy/` | Existing TypeScript state, physics, renderer, tools and scene construction |
| `web/src/materials/` | Single authored legacy catalogue, validation, presentation and element reference |
| `web/tests/`, `web/benchmarks/` | TypeScript verification; backend-specific tests and fixtures live under `tests/legacy/` |
| `web/` configuration | Bun package/lockfile, Vite, TypeScript, Biome and browser HTML |
| `engine/` | Rust workspace, portable contracts and experimental GPU scene; see `engine/AGENTS.override.md` |
| `docs/README.md` | Knowledge map by question and topic; `docs/START_HERE.md` is the engine migration brief |
| `mise.toml`, `.github/workflows/` | Root toolchain/task orchestration and CI |

## Architecture boundaries

- Keep authored TypeScript application code in `web/` and Rust/WGSL code in
  `engine/`. Do not reintroduce root `src/`, `tests/`, `crates/` or `shaders/` trees.
- Browser modules import simulation operations, observations and view metadata through `web/src/engine-client/`.
  The default sandbox uses `index.ts`; the experimental GPU page uses the dedicated `wasm.ts` host.
  Do not import legacy backend internals or generated WASM directly from the UI. Biome enforces this boundary.
- The default entrypoint re-exports the synchronous legacy API.
  The GPU preview uses a dedicated adapter; the general E03 session contract currently has mock owners.
  Add registries or shared wrappers only when a real caller requires them.
  The legacy Canvas 2D API must not constrain the asynchronous GPU contract.
- Keep one authoritative state and transport owner per live scene. GPU scenes
  must use bounded commands and asynchronous tick/epoch-stamped observations;
  do not copy the whole world into JavaScript each frame or maintain two ticking worlds.
- Keep one authored material catalogue at `web/src/materials/definitions.ts`.
  Generate validated Rust/WGSL projections when needed; never maintain handwritten
  copies. An element reference entry is not evidence of implemented physics.

## Rust migration placement

The Cargo workspace, lockfile and pinned toolchain live inside `engine/`.
Use `engine/crates/sim/` for portable IDs, units and contracts; `sim-cpu/` for
Rust f64 references and bounded CPU algorithms; `sim-gpu/` for wgpu state,
scheduling and rendering; and `wasm/` for browser bindings and construction.
CPU/GPU crates depend on portable contracts, never the reverse. Browser/DOM
dependencies belong in the bridge, not the portable core.

WGSL belongs in `engine/crates/sim-gpu/shaders/`, beside its pipeline and ABI owner.
Crate tests stay in their crate. Generate browser glue, declarations and WASM into
ignored `web/generated/wasm/`; only the engine client consumes them. Ignore
`engine/target/` and keep lockfiles tracked. Create modules when their milestone
has a caller; do not add empty crates or future shader trees for appearance.

Retain legacy scenes and their regressions until supported-scene conversion and
promotion gates pass. New numerical references are Rust, not another TypeScript
implementation to port later. Follow E00-E14 within P1 against the required
`docs/plans/rust-wasm-migration/current-sandbox-scope.md` checklist. Every current
feature, including the eight existing elemental models, must pass before the full
GPU default switch. P2-P9, FIRE-W08 and optional worker/product expansion are
deferred; completing P1 does not activate them. Keep their specifications without
implementing them unless the user separately selects that work.

## Commands and verification

Run tasks from the repository root. `mise.toml` owns tool versions and commands;
application tasks run in `web/` automatically. Do not add duplicate package scripts
or another workspace/task framework.

| Command | Purpose |
| --- | --- |
| `mise install` then `mise run setup` | Install pinned tools and dependencies; Bun uses `web/bun.lock` with `--frozen-lockfile` |
| `mise run dev` | Start Vite; production output is `web/dist/` |
| `mise run docs:check` | Validate guidance, links, manifests and preserved evidence |
| `mise run lint`, `mise run typecheck`, `mise run test` | Check browser code, types and regressions |
| `mise run rust:fmt`, `mise run rust:clippy`, `mise run rust:test` | Check Rust formatting, lint and workspace tests |
| `mise run build:wasm`, `mise run check:wasm` | Generate bindings and check their freshness |
| `mise run build:catalogue`, `mise run check:catalogue` | Generate and check the single-source material projection |
| `mise run test:browser` | Run isolated WASM/browser smoke with optional GPU availability |
| `mise run ci` | Lint, TypeScript/Rust checks, tests, WASM freshness, builds, browser smoke and docs |
| `mise run bench` | Run existing physics benchmarks when performance work requires them |
| `mise run docs:render` | Regenerate the GPU HTML after changing its canonical Markdown |

Use existing behavior tests for structural changes. Preserve deterministic seeds,
fixed-step timing, source accounting and conservation assertions. Derived floating
point values need tolerance-based assertions. Keep the ambient-air movement
regression in `web/tests/legacy/physics/motion.test.ts`.

For documentation-only edits, run `mise run docs:check` and inspect the diff.
Use focused behavior tests for code changes. Run `mise run ci` before delivering application, engine or build changes.
Do not add tests that merely repeat implementation details. Keep meaningful regression assertions and numerical tolerances.

Native, WASM and real browser checks are wired into mise and CI.
The default browser smoke can pass with `gpu.status: unavailable`.
That result validates the executed browser/WASM checks, not GPU simulation or rendering.
For browser GPU evidence, run
`mise run test:browser artifacts/validation/browser/<new-run> --require-gpu` on a machine with a real adapter.
Add `--sustained-gpu` when the assigned acceptance gate requires the sustained fixture.
Native GPU success does not validate browser behavior. Default Rust tests omit explicitly ignored hardware fixtures.
Report hardware-dependent or unrun checks explicitly. Do not report planned migration gates as passed.

GitHub CI uses standard hosted runners and separate docs, web, Rust and browser jobs.
The final `verify` job requires all applicable jobs. See the workflow and merge policy for path-filter behavior.
Do not configure paid or larger GPU runners without explicit user approval.
Keep required GPU validation as separate local hardware evidence.

## Documentation and change discipline

Use [the knowledge map](docs/README.md) to read only the topics relevant to the
user's task. It defines document authority and the article maintenance convention.
Keep current behavior, accepted design, planned work and historical evidence
distinct. Reading a plan does not expand the assigned scope or authorize its backlog.

`docs/project-structure.md` owns current module placement; root README is the
quick start; `docs/legacy-sandbox.md` describes existing controls. Topic articles
link source symbols, regression tests, limitations and design rationale. Update
the owning article with behavior changes; link to canonical facts instead of
copying status or contracts. Investigate code/document conflicts before resolving them.

Keep qualitative references, such as Sandspiel smoke, in `docs/sources.md` and the relevant deferred plan.
Inspiration does not add a current-feature requirement or authorize new gameplay work.

Use the current working tree, including uncommitted changes, as the source for
current behavior. Historical source ZIPs and their hashes are audit provenance,
not a development dependency. Record new evidence against the Git commit and
working-tree changes; do not require a replacement source archive.

Update active documentation and commands when paths or behavior change. The
2026-09-27 `web/` and `engine/` layout supersedes earlier instructions to retain
root `src/`. Historical source paths and line numbers remain historical: preserve
`docs/history/`, `docs/evidence/`, source/fire provenance manifests and original
audit files byte-for-byte. Planning data under `docs/data/` is not runtime data.

Preserve unrelated tracked/untracked work. Keep transient logs, profiles,
screenshots and recoverable local snapshots in ignored `artifacts/`; never commit
credentials or generated build outputs. Keep changes focused and verify the
actual working tree before reporting completion.
