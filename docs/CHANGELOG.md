# Documentation change log

## 23 September 2026: fire-audit integration into the GPU-first plan

Added a dedicated bounded combustion plan, nine FIRE-W tasks and fourteen corrected FIRE-A fixture specifications. Linked them to the existing E00-E14 work and P1/M0/M1/M4/M6/M7, with one extension task in P4/C5. M3 remains nonreactive; the selected TypeScript/Rust/WASM/wgpu split, P1-P9 sequence and 40/83/103/118 element cohorts are unchanged.

Made closed-boundary O2, shared finite reactants, cold-FIRE ignition, product/energy accounting, condensed-fuel release, amount/thermal water suppression, physical-time rates and persistent smoke explicit requirements. Derived particle/shader visuals and the funded Fire brush remain supported without becoming physical state authorities. Added FIRE-PRECONDITIONS, FIRE-M6 and FIRE-M7, including hot-property-domain and transactional recovery requirements.

Corrected active model text that implied the legacy boundary flag sealed combustion oxygen or that all atmospheric inputs were ledgered. Preserved the distinction between two full-pipeline bugs, composition issues, documented limitations, the reaction-only water test and positive controls. Scalar mass/energy conservation is not presented as proof of chemical correctness.

Imported all five fire-review files and the original review ZIP unchanged, with hash/source-provenance verification. Their twelve checks remain prior characterization evidence, not tests rerun in this revision or passes of the corrected specification. Existing static-source and historical audit files, element roster/schema/guide and original archives remain unchanged.

Updated entrypoint, roadmap, active work, architecture, GPU milestones, migration work, materials plan, validation, evidence template and source provenance. Regenerated the GPU HTML and extended documentation validation to fire work/fixture references, parent milestones, gate coverage and imported audit integrity. No application source, build, Rust/WASM compilation, numerical simulation, fire audit or GPU test was executed by this docs-only change.

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
