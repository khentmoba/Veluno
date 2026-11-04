# Zen-style features

Captured from an isolated headless debug build (`target/debug/veluno.exe`) driving the local test fixture through WebView2 DevTools. Sites are synthetic fixture pages.

- `sidebar-before.png`: default settings: neutral accent, no workspace icon.
- `sidebar-after.png`: Amethyst accent, a 📚 workspace icon in the heading and switcher, and two tabs marked as a Split View pair.
- `settings-accent.png`: Settings > General with the new Sidebar on the right switch and Accent color choices.
- `glance.png`: the page shown in the Glance preview window after Alt+Click, captured by `tests/native_regressions.py`.

These are per-WebView captures. They do not show Windows composition, so the side-by-side Split View panes and the side panel are proven by the native checks in `tests/native_controls.mjs` (pane widths and visibility) rather than by a whole-window image.
