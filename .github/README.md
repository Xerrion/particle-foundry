# GitHub CI and merge policy

The primary repository is
[Xerrion/particle-foundry](https://github.com/Xerrion/particle-foundry).
GitHub Actions runs the pinned mise tasks. Former Forgejo configuration remains
in the [archived source history](https://github.com/Xerrion/particle-foundry/tree/codex/forgejo-main-archive/.forgejo).

## Repository settings

Verified against GitHub on 2026-10-05:

- The repository is public.
- Pull requests from external contributors require approval before workflows run.
  Pushes and all pull requests use standard GitHub-hosted Ubuntu runners.
- Squash merge is the only allowed merge method, keeping one commit per PR.
- Squash commits use the PR title and body. Use a conventional PR title such as
  `fix: preserve ambient air movement` and describe the change in the body.
- GitHub automatically deletes merged head branches and offers the Update branch
  button so contributors can refresh a PR before merging.
- The active [default-branch ruleset](https://github.com/Xerrion/particle-foundry/rules/24074360)
  requires a pull request, resolved review threads, linear history and the
  `verify` status check. It blocks branch deletion and force pushes.

## Checks and merge requirements

The ruleset requires no approving review, and auto-merge is disabled. Confirm
that `verify` passes and review conversations are resolved before squash merging.

CI runs once for each pull request update and once after a push to `main`.
The documentation check always runs. Changes confined to `docs/` skip the web,
Rust and browser jobs; every other change enables all three. GitHub schedules
the independent jobs on its hosted runners. The final
`verify` job fails if any applicable job fails or is missing. The web, Rust and
browser jobs use `ci:web`, `ci:rust` and `ci:browser`. `mise run ci` remains
the complete local check. The Rust test
profile optimizes the long numerical refinement fixture while keeping debug
assertions and overflow checks; a cold runner must still compile its test binaries.

Use a feature branch and a conventional PR title. Review the complete change.
Passing checks do not authorize a merge or deployment.

Coolify's GitHub App integration owns the intended production and native PR
preview triggers. Production auto-deploy follows a merge to protected `main`.
GitHub Actions does not need a copied Coolify API token for this setup.
See [Coolify deployment](../docs/deployment.md) for configuration and verification.

## GitHub-hosted runners

All six jobs use `ubuntu-latest`, including same-repository pull requests.
Each job starts in a clean, temporary GitHub-managed environment. Mise installs
the pinned project tools; the hosted Ubuntu image supplies Chrome. No job selects
the persistent Charles runners.

External contributor workflows still require approval. Review workflow changes
before approving external runs. Persistent self-hosted runners can retain a
compromise between jobs; an event-based routing expression is not an access
boundary because a fork can edit its workflow. See
[GitHub's security guidance](https://docs.github.com/en/actions/reference/security/secure-use#hardening-for-self-hosted-runners).

The former two-slot Charles configuration is retained for recovery, with its
[runner guide](runners/README.md). Keep it stopped after hosted CI is adopted on
`main` and its main-branch check passes.

## Browser evidence

The browser job runs on `ubuntu-latest`. Its smoke test passes when no
WebGPU adapter exists, and the report records `gpu.status: unavailable`. A green
browser job therefore verifies WASM loading and browser fallback behavior, not
GPU simulation or rendering. Ordinary smoke uses Chrome's default headless
graphics selection. Only `--require-gpu` forces hardware rendering, which can
require an X11 display on Linux. See
[Chromium's headless GPU guidance](https://chromium.googlesource.com/chromium/src/+/HEAD/docs/gpu/using-gpu-hardware-in-headless-chrome.md).
Use
`mise run test:browser artifacts/validation/browser/<new-run> --require-gpu` on a
machine with a real GPU for the hardware gate. Record that result separately
before claiming GPU validation. No paid GitHub GPU runner is configured. The
browser artifact expires after one day to keep storage use small. Standard
GitHub-hosted runner time is [free for public repositories](https://docs.github.com/en/billing/concepts/product-billing/github-actions);
larger GPU runners are billed separately.
