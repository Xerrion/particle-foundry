# Documentation change log

## 21 September 2026: Rust/WASM + wgpu migration revision

Selected ADR-001: retain the TypeScript application, use Rust references/engine ownership, WASM browser bindings, and Rust wgpu-managed WGSL compute/direct rendering. This replaces the previous TypeScript CPU-reference default without changing P1-P9 order or the 40/83/103/118 roster.

Added an engine-boundary contract, E00-E14 work packages and machine-readable dependency tracker, backend-migration validation, and a hash/line-based static review of src(3).zip. The review records four unconditional pressure derivations per tick including the nested gas step, plus a possible fifth after venting; historical benchmark values remain attributed and unverified for this upload.

Updated the entrypoint, roadmap, active backlog, source/target module map, GPU milestones, matter architecture, later phase plans, evidence template and source register. Added async initialization/probes, accepted-time/commit semantics, canvas selection, one wgpu device, generated data/ABI checks, bounded transfers and explicit snapshot/loss recovery. Independent M1 circuit reference work does not block the M2 fluid-contract branch.

Preserved every original path, the original input archive, all three audit files, the previous documentation package, and the entire element roster unchanged. Regenerated the GPU HTML from Markdown. All implementation work remains planned. No application source, Rust/WASM implementation, numerical test, benchmark or GPU run was produced by this docs-only revision.

## 21 September 2026: GPU-first roadmap and all-element/nuclear expansion

This is a documentation revision made from the supplied archive. It does not implement the simulation, run the application test suite, reproduce the numerical audit, or measure GPU hardware.

### Added

- `START_HERE.md`: one developer entrypoint, copy-paste implementation brief, first M0-M3 work slice, and continuation through all P1-P9 phases.
- A canonical phase manifest and exact 118-element JSON roster/schema with 40/43/20/15 cohorts and cumulative targets 40/83/103/118.
- Matter architecture separating element, nuclide, chemical species, material form, recipe, component population and physical state.
- Materials/chemistry C0-C9 plan, including essential compounds, partial concentrations, shared material mechanisms, and source-qualified support.
- Nuclear N0-N8 plan, including decay, radiation, explicit fusion/fission networks, mass/energy conventions, supported host coupling, and exotic creative-mode behavior.
- Cross-phase acceptance and Q0-Q2 release gates, a milestone evidence template, source register, documentation index, and validation/rendering scripts.
- Exact original input archive and per-file hashes for historical preservation.

### Updated existing documentation

- Replaced the old partly delivered phase ordering with a GPU-first P1-P9 implementation roadmap while retaining original legacy history in the preserved archive.
- Rewrote the current model and active backlog in English, qualifying source/test claims as reported by the supplied documentation.
- Retained the eight element-model specifications and their original constants/limits; added complete cohort tables and separate material/chemical/nuclear/creative support semantics.
- Corrected the stale five-element reference and the implication that no element had any implemented model. The documented legacy count is eight; the other 110 identities are reference-only there.
- Extended GPU M1 with the minimal identity/storage contract needed by later chemistry and nuclear work. The M3 demonstrator scope remains unchanged.
- Clarified active-component GPU storage, snapshot/network versioning, explicit capacity failures, and physical-model versus execution-backend selection.
- Added W0-W4 IDs to the pressure/breakage plan and requirements to retain composition through fragments and later source coupling.
- Qualified the historical audit as historical; a fresh M0 run must compare against, not overwrite or force, its 24-pass/11-fail result.
- Regenerated the GPU HTML view from the revised Markdown, removing stale paths and independently drifting presentation text.

### Preserved unchanged

`plans/fluid-gpu-redesign/audit.cjs`, `current-audit-output.txt`, and `current-audit-results.json` are byte-for-byte unchanged. The full input is preserved in `history/original-docs-2026-09-21.zip`. Its source provenance, tool versions, numbers and old path labels were not rewritten.

### Interpretation corrections

The 40/83/103/118 milestones are development cohorts, not stable-element classifications or claims that every identity has measured bulk properties. Unknown data stays unknown; predicted data and fictional gameplay values have different labels. Nuclear channels use nuclides and states, not one element-level radioactive flag. A reaction network does not imply a validated plasma, reactor or shock model.

### Verification scope

The delivered package is checked for local documentation links/anchors, roster identity/count consistency, phase ordering, JSON schema validity, generated HTML freshness, archive integrity, and preservation of original audit bytes. Application tests and physical/GPU validation remain the developer's phase work.
