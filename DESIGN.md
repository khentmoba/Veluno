# Veluno interface

## Direction

Operate mode: a Zen-inspired desktop sidebar, not a browser feature clone. Keep the page primary, keep navigation and workspace tabs uncluttered, and put workspace switching at the bottom. Retain native Windows/system typography, offline assets, and the lightweight Rust shell.

## Surfaces

- `src/ui.html`: 260px expanded sidebar; 56px compact rail. Navigation, address field, workspace heading, new-tab/search actions, pinned and ordinary tab groups, sleep state, and bottom workspace dock.
- `src/start_page.html`: quiet, left-aligned startup/Home page with eight shortcuts ranked by user visit frequency (`LOCALAPPDATA/Veluno/shortcuts.json`) and padded with default sites. Shortcuts use two columns of compact icon-and-label rows, collapsing to one below 360px; scrolling remains available at the minimum desktop window size.
- `src/new_tab.html`: Zen-style floating new-tab search in a separate transparent, normally hidden WebView. It covers the client area without replacing or modifying the current page.
- `src/browser.js`: shared address resolution and keyboard shortcuts. Embedded into both local pages and initialized before remote content. Management IPC from content requires the injected trusted-key handler's per-process token; ordinary page scripts do not receive that token through the public helper.

## Visual system

User-requested black theme: sidebar `#101011`, page `#0c0c0e`, neutral accent `#dedee3`, primary text `#f5f5f7`. Sidebar surfaces `#1b1b1e`, active tab `#2e2e33`, secondary text `#b2b2b8`. The native Windows title bar also uses dark mode. The floating search panel uses `#171719` over a translucent black scrim. Use solid fills rather than decorative gradients/glows. Mostly 8–10px control corners, 40px tab rows, 32px navigation buttons, and consistent authored stroke icons. Active state comes from fill and weight, not color alone. Focus rings are 2px; address/search fields instead highlight the containing form. Loading animation respects reduced-motion preferences.

## Interaction contracts

- Ctrl+T and the sidebar New tab action open the floating search palette, focused immediately, without creating a tab or reloading the current page. Enter creates a tab for the typed address or Brave Search query. Open-tab suggestions can be selected with arrows/Enter or a click to switch instead. Escape, the close button, or the backdrop dismisses without creating a blank tab and returns focus to content. Ctrl+Shift+T still restores immediately. The startup/Home page remains available separately. Closing its tab suppresses Home on later startup.
- The palette owns native overlay bounds and blocks navigation to remote documents. Its canvas must stay transparent: root color-scheme is normal, with dark color-scheme scoped to the opaque panel. Reopening clears the draft; IME composition must not accidentally submit.
- The sidebar stays above the search overlay, retaining hover and clickable New tab controls. Opening search does not count as leaving the sidebar; moving the pointer away still hides a floating sidebar after its normal delay.

- Blurred HTTPS addresses display the complete hostname; focus exposes and selects the full URL. HTTP keeps its scheme visible. State updates must not overwrite an in-progress address edit or discard a focused tab button.
- The sidebar address field shows local suggestions for open tabs, bookmarks, history, and shortcuts on focus and while typing. Arrow keys and pointer hover select a result; Enter or a click switches to an existing tab or navigates the current tab. Escape or leaving the field dismisses the list. IME composition never submits a draft.
- Pinning is workspace-scoped and protects a tab from Close Others. Closing others affects only the current workspace and retains restoration history.
- Search filters tab titles and URLs within the selected workspace; Escape clears the filter and returns focus to its trigger.
- The compact rail uses title tooltips. Opening its actions menu expands the rail first rather than clipping a menu into a narrow WebView. Ctrl+L also expands it and focuses the address field.
- Back and Forward reflect native history availability. Loading changes Reload to Stop. Sleeping tabs use subdued text and a moon icon; never report invented MB savings. Selecting a failed tab recreates its WebView and retries its saved address; healthy tabs retain their page state.
- The top bar has an empty drag area and only Minimize, Maximize/Restore, and Close controls. Site titles and addresses stay in the sidebar. Sidebar favicons render at 16px with their natural transparency, without background tiles. Missing or broken saved icons fall back to the site's own favicon, then a neutral globe.
- After an in-app update, the restored active HTTP(S) page revalidates its cached document on its initial navigation. It loads once, without requiring a manual reload or clearing site storage.
- The user-highlighted four-tile Essentials dock is removed entirely, including compact-mode markup/styles. The separate Home page shortcuts remain.

## Research used

Two Gemini 3.8 Flash research subagents investigated mainstream browsers and Zen. Applied the structural patterns, not every proposed capability or unverified implementation detail.

- [Zen Workspaces](https://docs.zen-browser.app/user-manual/workspaces): bottom workspace switching and scoped tabs.
- [Zen Compact Mode](https://docs.zen-browser.app/user-manual/compact-mode): a content-first compact rail; full floating/edge-hover behavior is not implemented across Veluno's native child WebViews.
- [Zen desktop source](https://github.com/zen-browser/desktop): essentials/pinned/ordinary tab hierarchy as the reference, without importing Zen branding or source assets.
- [Chromium omnibox principles](https://www.chromium.org/user-experience/omnibox/): one search/address entry point and familiar keyboard behavior.
- [Edge sleeping tabs](https://blogs.windows.com/msedgedev/2020/12/09/sleeping-tabs-beta-performance/): quiet dormant-tab treatment instead of noisy resource claims.

## Workspace and site controls

Regular tab groups are labelled Workspaces throughout the UI. Their cookies and website logins remain shared. The workspace picker supports renaming and deletion; deletion explicitly chooses another regular workspace and moves live tabs, pins, and recently closed tabs without rebuilding healthy WebViews. The final regular workspace and Private cannot be deleted through this control; Private cannot be renamed or used as a move destination.

Tabs reorder by drag within their pinned or ordinary group, through the tab menu, or with Alt+Shift+Up/Down while a sidebar tab has focus. The saved tab order persists. Cross-workspace and cross-pin-group reorder requests are ignored.

Native playing-audio events show a speaker control in tab rows. Mute remains available through the tab menu for silent tabs. Muting changes the native WebView, persists with the tab, and applies before loading recreated pages. The current playing state is refreshed from the native view rather than restored from a session.

Address suggestions display the site's saved favicon, reusing a known icon from tabs or shortcuts for matching hosts, then trying the site's own `/favicon.ico`. Failed loads retain the suggestion-type icon. Search actions keep the search icon; internal targets do not fetch site icons. Icons preserve transparency and use no referrer.

The address connection icon opens site controls with the complete origin, an HTTPS/HTTP explanation, and a two-step clear-data action. Clearing validates the active tab and origin, removes that origin's cookies and stored data, then reloads the matching page after completion. The confirmation explains shared workspace data and cookies shared with related subdomains. Permission decisions persist in the profile so granted or blocked sites do not prompt on repeat visits.

Status and undo feedback share one compact area. Routine download completion, clear-data, and up-to-date messages expire; closing a tab offers a short-lived Undo row, separated from any persistent message by a divider; persistent messages stay until dismissed and can be expanded to read the full text. Settings categories retain their text labels and accessible names at the minimum window size. Both History and Settings confirm clearing history.

A download-only navigation with no committed document removes its unpinned empty tab and selects its surviving opener within the same workspace. The native download owner stays hidden until the transfer can be released; active/resumable owners and at most one completed owner remain available to Downloads. Existing documents, including Home, stay open and recover their committed address after a download navigation. Automatic cleanup never creates a recently closed tab or an Undo message.

## Glance

Alt+Click on an http(s) link previews it in the popup window instead of navigating or downloading it (Chromium's default for Alt+Click). The page script reports the link with the tab's per-process token, so ordinary page scripts cannot open previews. Glance sizes the window to about 82% × 86% of the browser and centers it there. Its WebView uses the tab profile and environment options, so sign-ins carry over; previews from private tabs are InPrivate. The bar shows the read-only address, Open as tab, and Close. Escape or Ctrl+W in the preview, the close button, the window's own close, or reactivating the main window dismiss it, unless the page already handled the key. Sign-in auto-promotion applies only to page-opened popups, never to Glance. The popup is a separate owned window rather than an overlay over the content WebView. Native child WebViews cannot be layered with a scrim the way Zen does it.

## Split View

A split pairs two tabs from one workspace. While either is active, the content area is divided into two equal panes with a window-margin gap; each pane keeps its own WebView, rounded region, and history. Focusing a pane's WebView makes it the active tab, so the address bar and shortcuts follow the pane in use. Sleep never unloads the visible partner. The pair ends when either tab closes, sleeps, or moves to another workspace, and it is not persisted.

## Side web panel

One docked site, taken from a regular (non-private) http(s) tab. It is a separate WebView on the tab profile, placed at the content edge away from the sidebar, 380px wide or at most 45% of the content area; tabs and split panes use the remaining width. Hiding keeps the WebView and its page state; changing the docked site replaces it. The URL and open state persist in the session.

## Motion

The focal interaction is the floating search palette: each invocation resolves into focus over a dimmed page in 280ms, and exits in 100ms before the native overlay hides. The scrim animates opacity on its own noninteractive layer; the panel uses transform and opacity without animated blur. Dismissal starts at the current scrim opacity, including during an interrupted entrance. A new invocation continues from the visible panel and scrim state, and cancels a pending exit so stale dismiss/submit IPC cannot fire. Reduced-motion mode skips these spatial effects and never delays submission or cancellation.

Tab rows preserve identity and use 260ms position transitions when the list changes; workspace entries stagger at most 48ms, holding their initial state during the delay. Title, favicon, audio, and sleep updates preserve running tab transitions when the visible order and pin groups stay the same, and skip position measurements. Active/hover fills take 160–180ms with a decelerating curve; press feedback takes 70ms and releases in 180ms. Loading state updates do not rebuild unchanged tab or workspace controls. Menus and feedback toasts have short, bounded reveals. Native WebView resizing and remote websites are not animated; there are no new dependencies or idle animation loops. Top-bar and popup loading indicators animate transform and opacity, preserve running progress through duplicate state updates, and finish from their current position. They become static under reduced motion, including when the preference changes during loading.

## Deliberate limits

No new UI dependency, fake settings, permission badges, private mode, bookmarks backend, or floating edge-hover sidebar. Only the new-tab palette is a floating overlay; suggestions use open tabs, bookmarks, saved history, and shortcuts; no network autocomplete is used. Closing Home suppresses it on later startup. Live tabs use independent WebViews; sleeping unloads a page and waking reloads its saved URL. Native WebView2 downloads provide progress and file actions. Sessions persist URLs, pins, workspaces, and recent closed tabs; unsaved forms and native page history do not survive restart. Site permissions use origin-labelled native prompts, and page errors provide Retry. Status messages persist until dismissed. Downloads and update activity disable conflicting actions. The Windows title follows the active tab.
