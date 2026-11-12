# Veluno

For releases or Discord release announcements, read [.agents/skills/discord-release/SKILL.md](.agents/skills/discord-release/SKILL.md).

For PR creation or updates, read [.agents/skills/veluno-pr/SKILL.md](.agents/skills/veluno-pr/SKILL.md).

## Tests

Do not write, run, or restore UI tests in the repo: no WebView2/CDP drivers, DOM
fixtures, screenshot or visual-capture harnesses, or native window or animation
checks. They were removed for being slow and flaky. Verify behavior with Rust unit
tests (`cargo test --locked`) or the pure JS checks in `tests/browser.test.cjs`, and
move logic out of UI code when it needs a test. Do not add tests that grep source
files for strings.

To check a visible behavior by hand, you may launch the app headless: set
`VELUNO_SMOKE_TEST=1` (the window stays hidden and unfocused) and point
`VELUNO_PROFILE_DIR` at a scratch folder so the run doesn't join an open Veluno.
Keep any driver script outside the repo, don't commit it, and close the app you started.

Local gate: `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`,
`cargo test --locked`, `node tests/browser.test.cjs`, `python -B tests/release.test.py`.
