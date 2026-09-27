# Resume the Rust/WASM migration

**Recovered 27 September 2026.** The original work survives on
`codex/p1-rust-wasm`. The recovery branch, `codex/wasm-port-new-structure`, starts
from reorganized `main` at `c0295a3c047bc064c11ced251b71db7fdfc86099` and carries the
same E00-E02 work in the current `web/` and `engine/` layout.

The recovery restored E00-E02. E03 now has separate contract verification. E05 is
next within the scoped GPU port; E04 circuits remain planned. M1 and P1 remain
incomplete.
The sandbox still runs the legacy TypeScript simulation. There is no Rust
numerical solver or command executor, GPU renderer or promoted scene yet.

## Recovered history

| Original commit | Recovered work |
| --- | --- |
| `ff3969b398883c4010e9695889aea34f4650fe0c` | E00/FIRE-W00 baseline, evidence runners and reference conventions |
| `b9c7eae011a442169dcfd18b96c9d6eccc164b08` | E01 Rust/WASM bootstrap and native/browser device lifecycle |
| `778e348a923ef12a210235f5c5e5f03c6551c1d2` | E02/FIRE-W01 identity, state, checkpoint and scalar GPU contracts |

E02 was local-only when recovery began. The interrupted recovery worktree already
contained staged copies of the Rust sources, scripts and evidence. Integration
restores the mise tasks, browser fixture, module paths and documentation status
on top of current main. The original branch remains available for comparison.

Before recovery edits, all Git refs were bundled and the interrupted worktree's
tracked/untracked files, staged patch, unstaged patch and status were saved under
ignored `artifacts/recovery/20260927-rust-rebase/` in the primary checkout.
This backup lives outside `.codex/`. The bundle and archive are local recovery
files; the original commits and this handoff are the durable repository record.

## Current ownership

- `engine/` owns Cargo files, the pinned Rust toolchain, four crates and reference
  fixture conventions. Rust source and its contract tests retain the original
  E02 implementation.
- `web/src/engine-client/` owns generated-binding access. The public entrypoint
  retains the synchronous legacy API. The separate WASM adapter lets the browser
  fixture test initialization and Rust rejection paths without bundling the
  experimental engine into the normal application.
- `web/generated/wasm/` and `engine/target/` are ignored generated output.
- `web/scripts/` owns WASM generation, browser smoke and legacy evidence runners.
  Root `mise.toml` owns their commands; `web/package.json` has no duplicate scripts.
- The [project structure](project-structure.md) links implementation and tests.
  Existing `docs/history/`, `docs/evidence/` and original audit files are preserved
  byte-for-byte. Earlier commands and timings remain historical.

## Run from the repository root

Install Rustup and mise, then:

```sh
mise trust
mise install
mise run setup
mise run ci
```

`engine/rust-toolchain.toml` pins Rust 1.98.1 and the WASM target. The workspace
locks wgpu 29.0.4, wasm-bindgen 0.2.108 and wasm-bindgen-futures 0.4.58. Setup
installs the matching wasm-bindgen CLI. `mise run ci` builds bindings before their
consumers and checks freshness after the build checks finish.

Useful focused checks:

```sh
mise run rust:test
mise run rust:clippy
mise run build:wasm
mise run check:wasm
mise run rust:device-smoke
mise run test:browser artifacts/validation/browser/new-run --require-gpu
```

The browser test needs Chrome. Set `CHROME_BIN` to a compatible executable when
the default is unavailable. Every run needs a new evidence directory, resolved
from the repository root. The test creates its own browser profile and reports
GPU absence separately; only `--require-gpu` makes missing GPU support fail.
Native device success does not establish browser GPU success.

Earlier native runs on this Mac needed
`SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk` per command because
of an SDK/linker mismatch. Use that override only if the same problem occurs;
do not change the global SDK.

## Evidence and limits

[E00](validation/p1-m0.md), [E01](validation/p1-m1.md) and
[E02](validation/p1-m1-e02.md) describe the original runs. Fresh recovery results
below apply to the relocated checkout based on `c0295a3` plus the recovery changes.
The [recovery provenance](evidence/rust-structure-recovery-2026-09-27/provenance.json)
records commands and hashes for the engine, scripts and build configuration.

| Recovery check | Result |
| --- | --- |
| `mise run ci` | Passed: 443 TypeScript tests, 11 Rust tests, both TypeScript checks, Rust formatting/Clippy, WASM build/freshness, browser smoke and docs |
| `mise run validate:baseline <fresh-directory>` after the final browser-import change | Passed collection, lint, both type checks, 443 tests, build and docs; legacy audit remained 25 pass / 10 fail, all 12 F observations reproduced, corrected fire assertions remained 4 pass / 2 fail |
| `mise run rust:device-smoke` | Native Metal device initialized |
| `mise run test:browser <fresh-directory> --require-gpu` | BrowserWebGpu passed in HeadlessChrome 154; 3 lifecycle cycles, 31 invalid inputs, 2 isolated sessions and explicit missing-GPU rejection; [result](evidence/rust-structure-recovery-2026-09-27/browser-result.json) |
| Normal production build inspection | No experimental WASM asset in the legacy app bundle |
| Byte comparison with original commits | All 13 Rust crate files, Cargo files, pinned toolchain, reference fixture and recovered historical evidence matched; main's history/evidence also remained unchanged |

Raw logs remain in ignored `artifacts/recovery/20260927-rust-rebase/` in the
primary checkout. The baseline's failing audit and corrected-fire subprocesses
are expected recorded defects; collection success does not make them physical
acceptance. Native/browser checks establish device lifecycle only. Numerical
references, shader sentinel tests, runtime recovery and scene promotion remain
unimplemented or unrun. Remote GitHub Actions was not used for these local results.

The [engine tracker](data/engine-migration-work.json) retains E00-E02 validation
with the original evidence. E03 has separate contract evidence; E04-E14 remain
planned. The
[fire tracker](data/fire-combustion-work.json) retains FIRE-W00/W01 baseline and
contract validation; corrected FIRE-A gates remain planned. No chemistry is
enabled, and the known legacy fire failures are not reclassified as passes.

## Next assigned work

E05 starts the corrected Rust f64 fluid reference. Read its acceptance criteria
in the [migration plan](plans/rust-wasm-migration/plan.md) and preserve the
[engine boundary](architecture/engine-boundary.md). E04 circuits remain planned
as an independent item.

Later numerical work uses Rust f64 references before WGSL. Preserve the restricted
M3 scene scope and the P2-P9 gates. Reading this handoff does not authorize starting
the remaining roadmap.

## E03 verification

[E03 evidence](validation/p1-m1-e03.md) records the experimental queued session,
exclusive canvas selection and generated candidate catalogue projection.
The legacy UI remains on its existing synchronous backend. The E03 owner used
by contract tests is a mock; no Rust physical scene is enabled.

`mise run ci` passed on the E03 source based on `a9c9834`: 451 TypeScript tests,
11 Rust tests, both type checks,
Rust tests/format/Clippy, WASM and catalogue freshness, production build, browser
smoke and documentation validation. The generated catalogue file lives under
ignored `web/generated/catalogue/`; the source SHA-256 and candidate SI fields
are checked against the authored catalogue. Local logs are in ignored
`artifacts/e03-ci-final.log`. E03 does not establish fluid parity, shader execution,
save/load or GPU performance.
