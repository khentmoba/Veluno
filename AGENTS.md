# Veluno

For releases or Discord release announcements, read [.agents/skills/discord-release/SKILL.md](.agents/skills/discord-release/SKILL.md).

For PR creation or updates, read [.agents/skills/veluno-pr/SKILL.md](.agents/skills/veluno-pr/SKILL.md).

## Tests

Do not write, run, or restore UI tests of any kind: no WebView2/CDP drivers, DOM
fixtures, screenshot or visual captures, native window or animation checks, and
no launching the app to click through it. They were removed for being slow and
flaky. Verify behavior with Rust unit tests (`cargo test --locked`) or the pure
JS checks in `tests/browser.test.cjs`, and move logic out of UI code when it
needs a test. Do not add tests that grep source files for strings.

Local gate: `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`,
`cargo test --locked`, `node tests/browser.test.cjs`, `python -B tests/release.test.py`.
