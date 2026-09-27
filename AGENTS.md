# Particle Foundry development guide

## Project map

Particle Foundry is a TypeScript browser sandbox. The Rust/WASM bootstrap and
portable state contracts are implemented through E02; the live sandbox still uses
the legacy backend. GPU simulation and rendering remain planned.

| Location | Responsibility |
| --- | --- |
| `web/src/app/`, `web/src/main.ts`, `web/src/styles/` | Browser startup, DOM, controls, viewport, clock and CSS |
| `web/src/engine-client/` | Browser-facing simulation entrypoint; currently delegates to the legacy backend |
| `web/src/legacy/` | Existing TypeScript state, physics, renderer, tools and scene construction |
| `web/src/materials/` | Single authored legacy catalogue, validation, presentation and element reference |
| `web/tests/`, `web/benchmarks/` | TypeScript verification; backend-specific tests and fixtures live under `tests/legacy/` |
| `web/` configuration | Bun package/lockfile, Vite, TypeScript, Biome and browser HTML |
| `engine/` | Rust workspace, portable contracts and lifecycle bootstrap; see `engine/AGENTS.override.md` |
| `docs/README.md` | Knowledge map by question and topic; `docs/START_HERE.md` is the engine migration brief |
| `mise.toml`, `.github/workflows/` | Root toolchain/task orchestration and CI |

## Architecture boundaries

- Keep authored TypeScript application code in `web/` and future Rust/WGSL code in
  `engine/`. Do not reintroduce root `src/`, `tests/`, `crates/` or `shaders/` trees.
- Browser modules import simulation operations, observations and view metadata
  through `web/src/engine-client/index.ts`. Do not import legacy backend internals
  or generated WASM directly from the UI. Biome enforces this boundary.
- The current entrypoint is a small re-export of the existing synchronous API.
  Do not invent backend registries, async wrappers or parallel implementations
  before a real second backend needs them. E03 introduces the async GPU contract;
  the legacy Canvas 2D API must not constrain it.
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
implementation to port later. Follow E00-E14 within P1 and preserve the later
P2-P9 gates in `docs/plans/rust-wasm-migration/plan.md`.

## Commands and verification

Run tasks from the repository root. `mise.toml` owns tool versions and commands;
application tasks run in `web/` automatically. Do not add duplicate package scripts
or another workspace/task framework.

| Command | Purpose |
| --- | --- |
| `mise install` then `mise run setup` | Install pinned tools and dependencies; Bun uses `web/bun.lock` with `--frozen-lockfile` |
| `mise run dev` | Start Vite; production output is `web/dist/` |
| `mise run ci` | Lint, TypeScript/Rust checks, tests, WASM freshness, builds, browser smoke and docs |
| `mise run bench` | Run existing physics benchmarks when performance work requires them |
| `mise run docs:render` | Regenerate the GPU HTML after changing its canonical Markdown |

Use existing behavior tests for structural changes. Preserve deterministic seeds,
fixed-step timing, source accounting and conservation assertions. Derived floating
point values need tolerance-based assertions. Keep the ambient-air movement
regression in `web/tests/legacy/physics/motion.test.ts`.

Native, WASM and real browser checks are wired into mise and CI. Native
GPU success does not validate browser behavior. Report hardware-dependent or
unrun checks explicitly; do not report planned migration gates as passed.

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
