# UI contrast and visual polish evidence

Captured from `tests/native_ui.mjs` and `tests/native_controls.mjs` during the headless WebView2 regression run (`target/debug/veluno.exe`, isolated temporary profile, loopback fixture server on `127.0.0.1`). All page content and origins are synthetic.

- `sidebar-after.png`: Expanded sidebar (`260 x 450`) with WCAG AAA text tokens (`#f8f8fa` primary, `#c8c8d2` secondary), distinct surface elevation (`#1c1c21` / `#232329`), `#3b3b44` borders, active tab inner highlight, and split-view 3px accent edge marker. Compare against `../zen-features/sidebar-before.png`.
- `settings-privacy-after.png`: Settings (`640 x 480`) with high-contrast labels, card borders (`#3f3f49`), and active category treatment. Compare against `../browser-privacy-reader/settings-privacy.png`.
- `history-after.png`: History and bookmarks (`640 x 480`) with sharper section headings, search bar border, and card elevation. Compare against `../history-site-logos/history-icons.png`.
- `permission-after.png`: Site permission dialog with `#f8f8fa` / `#c8c8d2` typography and `#484854` card/button borders. Compare against `../sidebar-permissions/permission.png`.
- `reader-view-after.png`: Reader view over a local fixture article with `#f5f5f7` body text and `#c8c8d2` metadata. Compare against `../browser-privacy-reader/reader-view.png`.

These per-WebView PNGs prove rendered content and contrast; native Windows composition and stacking order are verified separately by `tests/native_regressions.py`.
