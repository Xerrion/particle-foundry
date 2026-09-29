# GitHub merge policy

## Repository settings

Verified against GitHub on 2026-09-30:

- The repository is public.
- Squash merge is the only allowed merge method, keeping one commit per PR.
- Squash commits use the PR title and body. Use a conventional PR title such as
  `fix: preserve ambient air movement` and describe the change in the body.
- GitHub automatically deletes merged head branches and offers the Update branch
  button so contributors can refresh a PR before merging.
- The active [default-branch ruleset](https://github.com/Xerrion/particle-foundry/rules/24074360)
  requires a pull request, resolved review threads, linear history and the
  `verify` status check. It blocks branch deletion and force pushes.

## Current limitations and CI coverage

The ruleset requires no approving review, and auto-merge is disabled. Confirm
that `verify` passes and review conversations are resolved before squash merging.

CI runs once for each pull request update and once after a push to `main`.
The documentation check always runs. Changes confined to `docs/` skip the web,
Rust and browser jobs; every other change runs all three in parallel. The final
`verify` job fails if any applicable job fails or is missing. `mise run ci` remains
the complete local check. Splitting jobs gives earlier results, but the Rust
refinement fixture can still determine the time until a code change is fully green.

The browser job runs on standard `ubuntu-latest`. Its smoke test passes when no
WebGPU adapter exists, and the report records `gpu.status: unavailable`. A green
browser job therefore verifies WASM loading and browser fallback behavior, not
GPU simulation or rendering. Use
`mise run test:browser artifacts/validation/browser/<new-run> --require-gpu` on a
machine with a real GPU for the hardware gate. Record that result separately
before claiming GPU validation. No paid GitHub GPU runner is configured. The
browser artifact expires after one day to keep storage use small. Standard
GitHub-hosted runner time is [free for public repositories](https://docs.github.com/en/billing/concepts/product-billing/github-actions);
larger GPU runners are billed separately.
