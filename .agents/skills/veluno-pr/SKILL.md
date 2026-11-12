---
name: veluno-pr
description: Open or update requested Veluno pull requests with Windows/WebView2 evidence and checks for the current commit. Use for release PR preparation only when the user explicitly requests a release PR; releases use discord-release.
---

# Veluno pull requests

Create or update the PR, register it with the current T3 thread, and verify its
current head's checks. PR creation authorizes committing and pushing the scoped
change. Merging, tagging, publication, and Discord announcements require separate
user authorization.

## Establish the branch

Read the checkout's `AGENTS.md`, `.github/workflows/ci.yml`, and the changed scripts.
Inspect status, remotes, and existing PRs for this branch. Use the existing topic
branch and PR when they belong to the task; otherwise create a topic branch from
the intended base. Target `main` unless the user requested a different base.
Preserve unrelated edits and other worktrees.

Fetch before the final diff review. Integrate incoming base changes when needed,
preserving both branches' behavior, then validate the combined result. Review
`git diff <base>...HEAD` so already-merged work is not presented as new work.

PRs targeting `main` start Windows CI. It builds an installer, exercises a temporary
installation, and uploads artifacts. Disclose those effects before opening the PR.
CI does not publish GitHub releases or send Discord messages.

Done when the base, branch, task files, and CI effects are established.

## Validate and collect evidence

Choose checks for the change. For Rust or browser behavior, the normal local gate is:

```powershell
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
node tests/browser.test.cjs
python -B tests/release.test.py
```

For resource/version changes, run `cargo build --locked` and check the built
executable's file version. Do not write or run UI tests; see `AGENTS.md`.
Use the skill-creator validator for skill changes when that skill is available.

`.github/workflows/ci.yml` is the authority for the full remote gate. Its
`package` job runs `scripts/package.ps1`: release build, package metadata, and
installer lifecycle checks. Record local packaging as unrun when CI will cover it;
do not mistake a validation build or CI artifact for a publishable release.

For a bug fix, record the failing case and the same check passing after the fix.
For intermittent failures, retain the original failure log. A green rerun does
not establish a cause.
Report unresolved failures explicitly; do not replace assertions with delays.

For visible UI changes, check the behavior with a headless run of the app (see
`AGENTS.md`), keeping the driver outside the repo. Describe what you observed. Mark
screenshots not captured unless you took one; the user may add them.
For tooling or documentation changes with no visible UI change, use execution
evidence and mark screenshots not applicable. Exclude profiles and private data.

Run no-comments on the task diff, then unslop on the title and body when those
global skills are available. Apply blast-radius to identify and execute the core
safety check; state whether that fact is proven or unproven.

Done when relevant results, known failures, and evidence limits are recorded for
the final change. Recheck changed behavior after resolving conflicts.

## Open and verify the PR

Use this body structure, with a small diff sketch or call tree in the summary:

```markdown
## Summary

<concrete problem, resulting behavior, and smallest useful sketch>

## Evidence

<before/after execution or screenshots; exact commands and results; checks not run>

## Merge danger

Door: <one-way or two-way, with reason>
Blast radius: <affected behavior and the executed safety proof, or Unproven>
```

Stage explicit task files, commit, and push the topic branch. Use a temporary body
file with `gh pr create --body-file` or `gh pr edit --body-file`. Verify the PR's
base, head, and file list. Immediately call T3 `link_pull_request` with its full URL
when available. Before handoff, use `list_thread_pull_requests` and link any PR
from this task that is missing.

Inspect checks for the current head SHA. An empty check list is not a pass. Locate
the actual PR run; if CI cannot start because of a conflict, resolve the conflict
and check the new head. Fix failures within scope and rerun the affected checks.
Use T3 `watch_pull_request` when available to continue monitoring, then end the turn
with the PR and pending run linked. On a wake, verify readiness against the current
head. Stop watching with `unwatch_pull_request` when handing back a settled result.
Without T3 watching, use bounded waits on a known run and report pending status if
it remains unfinished.

Done when the PR is linked and current-head CI is passed, failed, pending, or
explicitly blocked. Return its URL, check state, and material merge risks.

## Explicitly requested release PRs

Release and Discord announcement requests follow
[discord-release](../discord-release/SKILL.md), which pushes release metadata
directly to `main`. Use this PR workflow for a release only when the user explicitly
requests a release PR. In that case, prepare `Cargo.toml`, `Cargo.lock`, and
`version.json` before committing or running a version-dependent build, and include
the target version and post-merge publication steps in the PR.
