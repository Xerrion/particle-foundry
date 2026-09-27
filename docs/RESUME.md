# Resume the WASM/GPU port

**Updated 27 September 2026 after the M0 commit.** The user resumed work and narrowed it to the WASM/GPU port plus necessary supporting contracts and references. The earlier full-roadmap brief is context, not authorization to expand this task.

**Next work item: P1 / M1 / E02.** E00/FIRE-W00 baseline capture and E01 bootstrap are validated. E02-E14 remain planned; M1 and P1 are not complete. No numerical solver, command queue, GPU rendering or scene promotion has been delivered. Stop after each completed roadmap phase, as requested.

## Checkout and delivery

| Item | Verified state |
| --- | --- |
| Branch | `codex/p1-rust-wasm` |
| Isolated worktree | `/Users/lasn/.codex/worktrees/16ee1bfb-b941-405f-8290-e37decc87d34/particle-foundry` |
| M0 commit | `ff3969b398883c4010e9695889aea34f4650fe0c` - baseline evidence and fixtures |
| E01 commit | Check `git log -1` and `git status`; this handoff records validation and recovery, not a self-referential commit hash. |
| Signing | The configured 1Password SSH signer accepted the M0 commit. Signing was not disabled. |

The earlier M0 commit attempt had returned:

```text
error: 1Password: agent returned an error
fatal: failed to write commit object
```

The retry succeeded without changing the signing policy. E01 delivery is a separate signed commit. No push or PR was created.

## Recovery

| Snapshot | Content |
| --- | --- |
| `/Users/lasn/Projects/particle-foundry-pre-p1.D8ISVe` | Original main/worktree tracked and untracked content, verified Git bundle, index/working patches and deletion-aware status |
| `/Users/lasn/Projects/particle-foundry-paused-p1.9ZYSru` | Paused partial E01 and validation artifacts |
| `/Users/lasn/Projects/particle-foundry-resume-p1.kFDUew` | Tracked/untracked snapshot before resumed edits; checksum reverified |
| `/Users/lasn/Projects/particle-foundry-e01-validated.qATbZm` | Validated E01/handoff tracked and untracked content, separate index/working patches, status and checksums |

The main checkout at `/Users/lasn/Projects/particle-foundry` was not edited. Its pre-existing deleted `.idea/workspace.xml` and untracked `particle-foundry.zip` were preserved in the original snapshot. See [M0 recovery details](validation/p1-m0.md). Never reset or clean either checkout to make the index simpler.

## Completed evidence

[M0 evidence](validation/p1-m0.md) and its [portable provenance](evidence/p1-m0-2026-09-26/provenance.json) remain unchanged. The fresh legacy audit was 25 pass / 10 fail; all 20 C controls pass. All imported F01-F12 observations reproduced. Corrected zero-O2 closed-boundary burning and cold-FIRE ignition assertions still fail. The four passing control subsets are not corrected FIRE-A aggregate acceptance; F06 remains reaction-only.

[E01 evidence](validation/p1-m1.md) records the final run at `artifacts/validation/p1-m1/20260926-e01-final/` and portable results under `docs/evidence/p1-m1-2026-09-26/verified/`:

- Frozen Bun install, Rust formatting, workspace Clippy, WASM release generation, production build, both TypeScript checks and frontend lint passed; **443 frontend tests and 4 native bootstrap tests passed**.
- Real production-bundled browser WASM passed three init/dispose cycles, 31 malformed-input checks, two concurrent sessions and explicit missing-WebGPU rejection. The hashed WASM asset loaded under `/engine-smoke/` with `application/wasm`.
- Actual wgpu initialization passed on native **Metal** and browser **BrowserWebGpu**. These are device/lifecycle checks, not compute, ABI, rendering or numerical acceptance.
- Generated output freshness passed for all four assets. An intentionally stale declaration was rejected; regeneration restored it and freshness passed again.

The existing TypeScript UI, scene, physics and renderer are unchanged. Normal production output still uses legacy only; the smoke build independently bundles the experimental engine. No full-world transfers or second ticking world were added. The host remains a thin generated-binding loader; `free()` now also disposes the Rust engine's GPU owner.

## Reproduce the bootstrap checks

Pins: Rust **1.98.1**, wgpu **29.0.4**, wasm-bindgen library/CLI **0.2.108**, wasm-bindgen-futures **0.4.58**, Bun **1.3.12**, TypeScript **7.0.2**, Vite **8.3.0**. Generated files in `src/generated/wasm/` and `target/` are ignored. After installing Rustup and Bun:

```sh
bun install --frozen-lockfile
rustup show
cargo install wasm-bindgen-cli --version 0.2.108 --locked
bun run build:wasm
bun run check:wasm
bun run check
```

This Mac's default macOS 27 SDK is incompatible with its installed linker. Use the already-installed SDK per command; do not change global settings:

```sh
SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk cargo test --locked --workspace
SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk cargo clippy --locked --workspace --all-targets -- -D warnings
SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk bun run build
```

The dedicated headless shell works here; the earlier full Chrome executables did not produce a completed DOM result. `CVDisplayLinkCreateWithCGDisplay failed` also occurs in successful runs, so it is not a sufficient cause. The runner now waits for a real-time completion report instead of virtual-time DOM dumping, enables the headless GPU, uses an isolated temporary profile and cleans up its processes/profile. Use a new evidence directory for every run:

```sh
CHROME_BIN='/Users/lasn/Library/Caches/ms-playwright/chromium_headless_shell-1243/chrome-headless-shell-mac-arm64/chrome-headless-shell' \
  bun run test:browser artifacts/validation/p1-m1/next-run --require-gpu
```

The verified shell is **153.0.8010.12** on macOS **26.6.2**, arm64. No browser dependency was installed or added to the repository. Without `--require-gpu`, unavailable GPU support is reported separately, not counted as a GPU pass. CI is configured but has not run remotely.

## Next ready work, bounded by current scope

1. Verify E01's signed commit in the branch history and a clean worktree before new implementation edits.
2. Implement E02's minimal state/identity/units, active-component, command/status, snapshot and shader-packing contracts. Include FIRE-W01's small inventory/source/visual seam only; do not activate chemistry or allocate 118 components per cell.
3. Implement E03's bounded command lifecycle, explicit supported-scene preflight, epoch-stamped async observations and one generated catalogue projection. Retain one authoritative state and transport owner; legacy remains separate.
4. Build E05/E06 in Rust f64. Only port the numerical solver to WGSL after its reference gates pass. M3 stays one liquid, carrier gas, fixed walls and passive markers, with resident state and same-device direct rendering.
5. Keep E04 circuits and later chemistry/nuclear/catalogue expansion deferred unless needed for an explicitly selected scene. Do not promote fire scenes without FIRE-PRECONDITIONS, FIRE-M6 and FIRE-M7, or advance to P2-P9 automatically. Partial port progress is not full P1 completion.

Unrun/not implemented: shader ABI/sentinel round trip, numerical reference/parity/performance gates, commands/probes/snapshots, device-loss recovery, direct rendering, default-backend promotion and corrected fire acceptance. No speedup is claimed. No background continuation is scheduled.
