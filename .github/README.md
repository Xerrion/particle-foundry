# GitHub merge policy

## Repository settings

Configured and read back from GitHub on 2026-09-27:

- Squash merge is the only allowed merge method, keeping one commit per PR.
- Squash commits use the PR title and body. Use a conventional PR title such as
  `fix: preserve ambient air movement` and describe the change in the body.
- GitHub automatically deletes merged head branches and offers the Update branch
  button so contributors can refresh a PR before merging.

## Current limitations

The repository remains private. As of 2026-09-27, GitHub rejects rulesets and
classic branch protection with HTTP 403 because private repository protection
requires GitHub Pro. Auto-merge also remains disabled. Only settings available on
the current plan are configured.

The merge settings above do not block direct pushes, force pushes, default branch
deletion, or merging failed checks. Required approvals and conversation resolution
are not enforced either.

Until branch protection is available, use pull requests and manually confirm
that the `verify` job in [ci.yml](workflows/ci.yml) passes and review conversations
are resolved before squash merging. This is a contributor convention, not an
enforced GitHub gate.

CI runs once for each pull request update and once after a push to `main`.
The documentation check always runs. Changes confined to `docs/` skip the web,
Rust and browser jobs; every other change runs all three in parallel. The final
`verify` job fails if any applicable job fails or is missing. `mise run ci` remains
the complete local check. Splitting jobs gives earlier results, but the Rust
refinement fixture can still determine the time until a code change is fully green.
