<p align="center">
  <img src="assets/logo-wordmark.svg" alt="Veluno Browser" width="320">
</p>

<p align="center">
  <strong>A smooth, lightweight, and easy-to-use modern browser built in Rust.</strong><br>
  Designed from the ground up to be ultra-fast, gentle on PC hardware, and free of background bloat.
</p>

---

## Highlights

- **Rust desktop shell:** Tao handles the Windows window and Wry hosts WebView2.
- **Independent live tabs:** Switching tabs preserves page state and native back/forward history while the tabs stay awake.
- **Floating sign-in & OAuth popups:** Login, OAuth, and confirmation popups open in a Zen-style floating window that shares your profile and `window.opener` session, with a compact read-only URL bar and one-click promotion to a full tab.
- **Glance link previews:** `Alt + Click` any web link to preview it in a large floating window centered over the browser, without leaving or changing the current tab. Press `Esc`, the close button, or click back into the browser to dismiss it, or choose **Open as tab** to keep it. Previews share the current tab's sign-ins (private tabs preview privately).
- **Split View:** right-click a tab in the sidebar and choose **Split with current tab** to show both side by side. Selecting either tab shows the pair again, clicking into a pane makes it the active tab for the address bar, and **Close split view** in the same menu returns to one tab. Split tabs stay awake; the pair is not restored after a restart.
- **Session recovery:** Tabs, workspace selection, pins, and recent closed tabs are saved locally, with a backup for recovery. Site storage uses a persistent WebView2 profile.
- **Windows integration:** Setup wizard with uninstaller, Start menu and desktop shortcuts, Installed apps registration, default-browser eligibility, and link handoff to the running instance.
- **Native downloads:** WebView2 provides download progress and file actions; Veluno reports completion and warns before closing active downloads.
- **Automatic tab sleeping:** Targets 12 live pages by unloading older background tabs. Pinned tabs stay awake, as do active downloads, playing media, capture sessions, and pages with detected edits or unload handlers. Pages with frames or failed safety checks stay awake too, so protected work can exceed the budget. Manual sleeping is still available. Selecting a sleeping tab reloads its saved URL; unsaved form state and page history do not survive sleeping or an app restart.
- **Ad blocking built in:** uBlock Origin Lite is downloaded on first launch (SHA-256 checked) and loaded into every tab as a WebView2 extension. Turn it off in **Settings > Privacy and security**.
- **Bookmarks:** Press `Ctrl + D` or the star in the address bar. Bookmarks appear first in new-tab suggestions and at the top of History, where you can remove them. **Settings > General > Import bookmarks** copies web bookmarks from Chrome, Edge, Brave, Vivaldi, Opera, or Opera GX on the same computer, reading their local bookmark files only.
- **Reader view:** Press `F9` (or **Reader view** in the app menu) on an article to read it in a clean, dark serif layout with adjustable text size. It runs only when you ask for it, and closing it leaves the page exactly as it was.
- **Accent colors:** **Settings > General > Accent color** switches Veluno's own interface between Neutral, Amethyst, Glacier, Emerald, Amber, and Rose. The change applies immediately and never restyles websites.
- **Sidebar position:** **Settings > General > Sidebar on the right** moves the tab sidebar to the right edge. It applies immediately, including the compact hover rail.
- **Workspace icons:** pick an emoji for each regular workspace from the workspace menu. It appears beside the workspace name in the sidebar heading and the switcher.
- **Tracking prevention and privacy controls:** WebView2's built-in tracking prevention (Basic, Balanced, or Strict) works with or without the ad blocker. Password saving is off by default, form autofill can be turned off, and `Ctrl + Shift + Delete` opens **Clear browsing data** for history, cookies and site data, cached files, or passwords and form data over the last hour, day, week, four weeks, or all time.
- **Screenshots:** **Save screenshot** in the app menu saves the visible page as a PNG in `Pictures\Veluno`.
- **Private tabs:** `Ctrl + Shift + N` (or **New private tab** in the app menu) opens a tab in the Private workspace. Its pages use an InPrivate WebView2 profile, and none of their tabs, history, or top sites are saved. Ad blocking may not apply to private pages.
- **Connection security:** The address bar shows a lock for HTTPS pages and a warning for plain HTTP. WebView2 blocks pages with certificate errors.
- **App menu and Settings:** The button at the top-left of the sidebar opens the app menu (new tab, downloads, reader view, screenshot, print, zoom, settings, updates, exit). Settings (`veluno://settings`) covers search engine, tracking prevention, passwords and autofill, clearing browsing data, bookmark import, ad blocker, updates, extensions, and shortcuts.
- **Built-in Upstream Sync & Conflict Inspector:** Includes an automated local tool and agent skill to fetch upstream sources, compare local changes, and simulate pre-merge conflict status in memory before applying.

---

## Architecture

```
┌────────────────────────────────────────────────────────┐
│  VELUNO (Rust Application Core)                       │
│                                                        │
│  Zen-inspired sidebar (260px / 56px compact rail)       │
│  • Navigation and address / search                     │
│  • Pinned tabs and searchable workspace tabs           │
│  • Workspace picker and tab actions                      │
│                                                        │
│  ┌─ Content Viewport (Hardware-Accelerated) ────────┐  │
│  │                                                  │  │
│  │   Web content rendered at native display refresh │  │
│  │                                                  │  │
│  └──────────────────────────────────────────────────┘  │
│                                                        │
│  Rust Event Loop & State:                              │
│  • High-DPI dynamic viewport auto-resizing             │
│  • Memory-efficient tab state & history tracking       │
│  • Fast IPC bridge between Chrome UI and Rust          │
└────────────────────────────────────────────────────────┘
```

---

## Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `Ctrl + T` | Open floating new-tab search |
| `Ctrl + W` | Close active tab |
| `Ctrl + L` | Focus and highlight address / search bar |
| `Ctrl + R` / `F5` | Reload current page |
| `Ctrl + Shift + R` / `Ctrl + F5` | Refresh this site's cached files and reload, keeping sign-in and saved site data |
| `Alt + Left` | Navigate back |
| `Alt + Right` | Navigate forward |
| `Ctrl + Shift + T` | Reopen last closed tab |
| `Ctrl + Tab` / `Ctrl + Shift + Tab` | Cycle tabs within the workspace |
| `Alt + 1` to `9` | Switch workspace |
| `Alt + Shift + Up` / `Down` | Reorder a focused sidebar tab within its pinned or ordinary group |
| `Ctrl + Shift + B` | Expand / collapse sidebar |
| `Ctrl + Shift + S` | Unload background tabs (except active downloads) |
| `Ctrl + H` | Open History and bookmarks |
| `Ctrl + D` | Bookmark / unbookmark the current page |
| `Ctrl + Shift + N` | Open a private tab |
| `Ctrl + F` | Find in page |
| `F9` | Open or close reader view |
| `Ctrl + Shift + Delete` | Clear browsing data |
| `Ctrl + J` | Open Downloads |
| `Ctrl + +` / `Ctrl + -` / `Ctrl + 0` | Zoom in, zoom out, or reset zoom |
| `F11` | Toggle full screen |
| `Alt + Click` (on a link) | Preview the link in Glance |
| `Esc` (in Glance) | Close the preview |
| `Enter` (in URL bar) | Smart navigate (detects URLs vs. web search) |

**New tabs work like Zen:** press `Ctrl+T` or click **New tab** to open a floating search/address bar over the current page. Enter opens the destination in a new tab; matching open tabs, history, and top shortcuts can be selected with the arrow keys. Escape, the close button, or clicking outside cancels without creating an empty tab or reloading the page. The startup/Home page remains available separately. The sidebar, Home page, native title bar, and palette share a black/charcoal theme.

The sidebar has no fixed shortcut-tile dock. Search opens/closes with short, interruptible motion, and tab and workspace changes retain visual continuity. Reduced-motion preferences skip spatial effects. Home-page shortcuts rank your most-visited sites (such as YouTube or X) first and fill any remaining slots from the default 8 shortcuts.

Web searches use **Brave Search** by default (Settings can switch to DuckDuckGo, Google, or Bing) throughout the sidebar, Home page, and floating search bar. Suggestions come from your local open tabs, bookmarks, history, and shortcuts; no remote autocomplete requests are sent while typing. Strong site or title matches appear before web search and are selected when you press Enter. Ordinary queries and weak matches keep web search first, and an explicit web search choice is always available.

**Search keywords:** start an address with `@yt`, `@gh`, `@w` (Wikipedia), `@r` (Reddit), `@maps`, `@g`, `@b`, `@ddg`, or `@brave` to search that site directly, for example `@yt lofi beats`. A keyword on its own opens the site.

Shortcuts work while browser chrome or page content has focus. In pages they are caught natively before the page sees the key, so sites and focused iframes can't swallow them; `Ctrl + S`, `Ctrl + F`, and other keys not listed stay with the page. Right-clicking any tab in the sidebar opens quick actions to pin/unpin, duplicate, sleep/wake, move to another workspace, or close it. The bottom sidebar menu contains pin/unpin, reopen, and close-other-tabs actions. Close Others keeps pinned tabs, affects only the current workspace, and can be undone one tab at a time with Reopen.

**Workspaces organize tabs, not accounts:** regular workspaces share cookies and website logins. Create, rename, or delete them from the bottom workspace picker. Deletion moves open tabs, pins, and recently closed tabs to the regular workspace you choose, without reloading live pages. Private remains separate and is never saved.

Drag sidebar tabs to reorder them within their pinned or ordinary group, or focus a tab and press `Alt+Shift+Up/Down`. Audio-playing tabs show a speaker button; click it to mute or unmute. The tab menu also offers mute controls, including for silent tabs, and mute choices survive sleep and restart.

Links that open only a download dismiss their empty tab and return to the originating page while the transfer continues. Downloads started from an existing page keep that page, its address, and its form state. Active and resumable downloads retain their native owner in the background; at most one completed download owner remains for the Downloads dialog. Automatically dismissed download tabs do not enter recently closed tabs or trigger Undo.

Click the connection icon beside a website address for its full origin, connection information, and **Clear cookies and site data**. Clearing requires confirmation, clears the selected origin's cookies and stored data, and reloads that page. Regular workspaces share the cleared data; cookies shared with related subdomains may also be removed. Routine download feedback expires, while persistent messages can be expanded or dismissed in the compact feedback area.

For pages stuck during startup, choose **Reload fresh site files** in the app menu or press **Ctrl+Shift+R**. This resets the current origin's service worker and resource cache, then reloads while bypassing the HTTP cache. Cookies, local storage, IndexedDB, and permission decisions are kept. When a same-origin script fails on a service-worker-controlled page, Veluno reports it and gives the next normal **Reload** the same recovery behavior. Resource failures do not reload a page automatically.

**Current engine limits:** Awake tabs preserve page state and navigation history. Sleeping tabs and restored sessions reload saved URLs; unsaved forms and per-tab history do not survive unloading or restart. Automatic sleeping uses conservative checks, but custom app state cannot always be detected; pin tabs whose state must be preserved.

See [DESIGN.md](DESIGN.md) for the interface decisions and research references.

---

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (edition 2024 / rustc 1.85+)
- Windows 10/11 (WebView2 runtime is pre-installed)

### Build & Run

```bash
# Run in development mode
cargo run

# Build optimized production release (< 3 MB binary)
cargo build --release
```

The compiled release executable will be located at:
```
target/release/veluno.exe
```

Development builds use line-table debug information to reduce compilation and linking
work while preserving source locations in backtraces. For full debugger support,
including local variables and types, override it in PowerShell:

```powershell
$env:CARGO_PROFILE_DEV_DEBUG = '2'
cargo run
Remove-Item Env:CARGO_PROFILE_DEV_DEBUG
```

Cargo already enables incremental compilation for `cargo run`. `cargo-blaze` 0.0.1
currently supports macOS and Linux and requires Rust 1.99; it cannot accelerate
Veluno's Windows/MSVC build. See [the benchmark evidence](docs/pr-evidence/search-and-build-performance/README.md)
for measured rebuild and suggestion timings.

---

## UI Verification

```bash
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
node tests/browser.test.cjs
```

For the DOM interaction checks, serve the repository with `python -m http.server 0` (using an ephemeral port so worktrees do not collide), then open `http://localhost:<port>/tests/ui.html`. This is a fixture with sample tabs; its IPC messages are recorded rather than sent to Rust. Test both 1380×860 and the minimum 640×480 desktop window; the compact rail can be inspected by setting `fixtureState.sidebar_collapsed = true`, `sidebar.style.width = '56px'`, and calling `sidebar.contentWindow.updateState(fixtureState)` in the test page console.

To exercise actual WebView2 keyboard input and Rust IPC on Windows, build with `cargo build` and run the self-contained native smoke test with Python 3.11+ and Node 22+. The Node command delegates to the headless regression harness, which copies the executable, uses an isolated profile and dynamic debugging port, and cleans up the child process. It also runs the DOM checks at 1380×860 and 640×480, including reduced motion:

```bash
cargo build
node tests/native-smoke.cjs
```

Native results and diagnostic logs are saved under `target/native-regressions/`. On failure, `failure.json` and `debugger.json` record the active tab, focus, sidebar state, recent keyboard input, and isolated debugger targets. Windows CI retains the top-level native logs and JSON reports for seven days after a failure; browser profiles are excluded. The same suite is mandatory during packaging and CI.

Suggestion ranking can be compared against an earlier commit with `node tests/suggestions.bench.cjs --baseline <git-ref>`. After building, `python -B tests/native_suggestions.py --baseline <git-ref>` runs the same comparison in a hidden WebView2 app with an isolated profile. Both check identical results before timing six workloads with 2,000 history entries, including replaced history objects. Results are medians of seven alternating samples and measure suggestion computation, not page loading or rendering.

For repeatable visual captures, run `python -B tests/native_capture.py` after building. It launches its own hidden app against local fixtures, uses a separate executable, profile, and debugging ports, and stops only its owned process tree. PNGs and a target manifest are saved under `target/native-capture/`. These are WebView captures; they do not prove native window composition or animation smoothness. The regression suite separately checks native sidebar stacking order and stable width during reveal, hide, and reversal, and menu opacity throughout its opening animation.

`python -B tests/native_tools.test.py` verifies that capture cleanup preserves a second running app, deliberately reintroduces the menu fade, and omits Ctrl+Shift+B input to prove both the guards and failure diagnostics. `powershell -NoProfile -File tests/version.test.ps1` builds two successive Windows resource versions without cleaning. Both checks also run in CI.


## Upstream Synchronization & Conflict Detection

Veluno includes a built-in upstream inspector that simulates upstream branch merges in-memory without dirtying your working tree:

```bash
# Using Python
python scripts/upstream_sync.py --upstream <remote_or_url> --branch main

# Or via PowerShell wrapper
.\scripts\upstream-sync.ps1 -Upstream <remote_or_url> -Branch main
```

A dedicated agent skill is available at `.agents/skills/upstream-sync/SKILL.md` to guide automated synchronization and conflict audits.

---

## Installation & Updates

### Windows App & Setup

Veluno is a Rust desktop app using Tao and Wry (Windows WebView2). Release
builds include an Explorer icon, Windows product/version details, and a taskbar
identity, and open without a console window.

Build the app and its installer locally (needs [Inno Setup 6](https://jrsoftware.org/isinfo.php):
`winget install JRSoftware.InnoSetup`):

```powershell
.\scripts\package.ps1
```

Run `target\release\veluno-setup.exe` to install for the current user: a
standard setup wizard with Start menu and optional desktop shortcuts, an entry in
Windows Installed apps with a real uninstaller, and no administrator prompt. The
source is `installer\veluno.iss`. For unattended installs use
`veluno-setup.exe /VERYSILENT /NOLAUNCH`; add `/DIR="path"` to choose a folder.
The portable app, `target\release\veluno.exe`, runs from anywhere without installing.
After installation, choose **Set default browser?** from the sidebar menu, then
select Veluno in Windows Settings. Veluno registers HTTP, HTTPS, and HTML
file support; Windows keeps the final choice with you.
Uninstall from Windows Settings to remove the app and shortcuts. Browser data
is preserved. Running the new setup over an install from before the wizard
replaces it and removes the old Installed apps entry.

### Verification and signing

`package.ps1` verifies Windows icons/version resources, installs and uninstalls the built
setup silently, and runs the release app
headlessly against local fixtures using a temporary profile. The tests cover
live tab form retention, native history, pop-ups, downloads, error/retry,
address and palette drafts, second-launch link handoff, forced-exit recovery,
and fallback from a corrupt session file. It then writes `target/release/SHA256SUMS`.
The same verification runs in Windows CI, which retains the app, setup, and checksums.

To sign with an existing certificate in your Windows user certificate store:

```powershell
.\scripts\package.ps1 -CertificateThumbprint YOUR_THUMBPRINT -TimestampServer YOUR_TIMESTAMP_URL
```

Without a signing certificate, the package remains unsigned. Checksums detect
file changes; they do not establish publisher identity. Packaging does not
publish a GitHub release or send a Discord message.

### Extensions and ad blocking

Veluno stores unpacked extensions in `%LOCALAPPDATA%\Veluno\extensions` and loads every folder there at startup. On first launch it downloads the latest uBlock Origin Lite (the Manifest V3 build; WebView2 no longer runs the Manifest V2 uBlock Origin) from `uBlockOrigin/uBOL-home`, checks the release's SHA-256, and unpacks it to `extensions\ublock-origin-lite`. If the first launch is offline, it retries on the next start or from **Settings > Extensions**. A new installation activates after restart so open pages keep their forms and history. Preferences live in `settings.json` beside `session.json`.

### Browser data and recovery

Browser data is stored in `%LOCALAPPDATA%\Veluno`. On first launch, Veluno moves an existing `%LOCALAPPDATA%\Felaris` profile there without overwriting a current Veluno profile. If Windows prevents moving the old folder, Veluno continues using it. Saved `felaris://` pages are migrated to `veluno://` pages. `session.json` and
`session.json.bak` retain tab URLs, profiles, pins, and recently closed tabs.
`WebView2` contains persistent site storage. Session changes are batched every
500 ms and written by one background writer. Clean exits and updates flush the
latest state; an abrupt exit can lose changes since the last completed save.
Restarting restores URLs, rather
than unsaved forms or per-tab navigation history. Uninstall preserves this data.

Page failures retain WebView2's native error page and navigation history; Reload
retries the address. Renderer failures preserve tab URLs for reload.
Camera, microphone, location, notifications, and clipboard-read requests show
an origin-labelled native prompt. Unknown permission types are denied. Standard
file pickers and downloads use WebView2's native interfaces.

For an isolated headless development check after `cargo build --locked`:

```powershell
python tests/native_smoke.py
python tests/native_regressions.py
```

The regression suite also exercises 100, 300, and 500 saved tabs with lazy restore, a 12-view budget, protected drafts/media/pins/downloads, waking sleepers, and recovery after forced exit. It records startup and tab-switch timings and the process tree working set in `capacity.json`. These local fixtures do not establish a RAM guarantee for arbitrary websites.

The regression suite requires Node 22+ and checks normalized addresses, SPA
routes, Back after page failures, sleeping-tab replacement, and the production
Close Other Tabs command while a download is running. It also exercises native
keyboard input and the DOM fixture at desktop and minimum sizes. Both suites
use isolated profiles and hidden windows.

### One-Line Install (PowerShell)

Download the latest `veluno-setup.exe`, check it against the release `SHA256SUMS`,
and install it silently (add `-NoLaunch` to skip opening Veluno):

```powershell
irm https://raw.githubusercontent.com/khentmoba/Veluno/main/scripts/install.ps1 | iex
```

### Automatic Updates

- Veluno checks for a new version shortly after launch and every 6 hours while it runs.
  `veluno.exe --check-update` checks from the command line.
- When one is available, an "Update Available" banner appears in the sidebar.
- **Update now** saves your session, downloads the release, verifies it against the
  release `SHA256SUMS`, and restarts Veluno on the new version.
  - Installed copies run `veluno-setup.exe` silently, so Windows' version record stays correct.
  - Portable copies swap `veluno.exe` in place and restore the previous one if that fails.

---

## Releases & Distribution Pipeline

Veluno automates releases to both GitHub Releases and Discord (`#veluno-releases`). New releases publish `veluno.exe` and `veluno-setup.exe`, plus identical `felaris.exe` and `felaris-setup.exe` compatibility copies and checksums for older updaters. The manifest retains the legacy repository redirect and executable filename so existing installs can upgrade. Historical release binaries retain their original names.

Release commands:

```bash
# Test Discord webhook connectivity
python scripts/notify_discord.py --test

# Prepare metadata before committing and pushing it directly to main
python scripts/build.py --prepare -s "Release summary"

# From a clean checkout of the checked commit on origin/main
python scripts/build.py --dry-run
python scripts/build.py
```

Python 3.11+ is required. Set the release version in `Cargo.toml` and update
`Cargo.lock` before preparation. Starting from the latest `origin/main`, commit
only `Cargo.toml`, `Cargo.lock`, and `version.json`, then push directly with
`git push origin HEAD:main` and wait for Windows push CI. Releases use this direct
path; a release PR is used only when explicitly requested.
Publication checks the checkout against the current remote
`main`, requires successful Windows CI for that exact SHA, verifies the release
package, and pins the GitHub tag to the built SHA.
An existing tag, dirty checkout, mismatched metadata, or failed GitHub publication
stops the release. Discord runs only after GitHub succeeds, unless `--skip-github`
was explicitly requested. `--dry-run` only validates and prints the plan.
The PowerShell entry point forwards to this same implementation.

Webhook configuration is resolved from `DISCORD_WEBHOOK_URL`, `scripts/discord_webhook.txt` (gitignored), or `%LOCALAPPDATA%\Veluno\discord_webhook.txt` (shared across worktrees). See `.agents/skills/discord-release/SKILL.md`.

---

## License

MIT OR Apache-2.0
