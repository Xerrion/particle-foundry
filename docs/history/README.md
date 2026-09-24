# Original input and historical evidence

This documentation revision was made from the uploaded `docs.zip`. No application source was included, and no application checks or GPU measurements were run here.

The complete original archive is preserved byte-for-byte as [original-docs-2026-09-21.zip](original-docs-2026-09-21.zip). It contains the original current-model description, Danish roadmap and delivered history, original plans, HTML presentation, and audit files. It is a preservation copy, not the developer entrypoint.

Original archive SHA-256:

```text
f0c353534cca10736d98c449fa96b5e2ed07c241a3ba7cd4463f897ab9860eb5
```

[original-file-hashes.json](original-file-hashes.json) records every original file's SHA-256 using paths relative to `docs/`.

The following files remain byte-for-byte unchanged at their original paths:

- [audit.cjs](../plans/fluid-gpu-redesign/audit.cjs)
- [current-audit-output.txt](../plans/fluid-gpu-redesign/current-audit-output.txt)
- [current-audit-results.json](../plans/fluid-gpu-redesign/current-audit-results.json)

The historical JSON reports 35 checks: 24 passing and 11 failing. Tool versions, historical source hashes, original path labels, and any first-person measurement statements belong to that earlier audit. They must not be represented as results of this documentation update. New baseline runs use a separate evidence directory and fresh provenance.

Current implementation descriptions remain qualified as documented by the supplied files until a developer confirms them against source. The updated roadmap and matter/nuclear designs supersede the original planning order, not the preserved historical measurements.

## Previous element-roadmap revision

[previous-elements-roadmap.zip](previous-elements-roadmap.zip) preserves the exact documentation package supplied before this Rust/WASM update. Its SHA-256 is in [previous-revision.json](previous-revision.json). It includes the original input archive and audit; neither was altered.

That previous package's TypeScript-reference defaults and proposed src/fluid paths are superseded by ADR-001 in the active docs. Archived text is historical, not a competing instruction set. The source upload itself is not duplicated into this documentation ZIP; its read-only hashes/locations are recorded in the active source manifest.

## Fire review preserved in this revision

[fire-review-2026-09-23.zip](fire-review-2026-09-23.zip) preserves the supplied fire-audit archive byte-for-byte. Its five extracted files are available under [evidence/fire-review-2026-09-23](../evidence/fire-review-2026-09-23/FIRE_REVIEW.md), with archive/file hashes in [the fire review manifest](../data/fire-review-manifest.json).

The report's statements that no existing docs archive was rewritten describe the original audit session. This later documentation revision does integrate its findings into the active plan. The 12 checks were run during that prior review, not during this edit. Their assertions reproduce observations, including bugs; FIRE-A gates separately define corrected acceptance.

Existing static-source records and the older 35-check audit remain unchanged. No application source is modified or copied into this package. The active fire plan supersedes incomplete legacy fire descriptions; archived copies preserve what was previously written and are not competing developer instructions.
