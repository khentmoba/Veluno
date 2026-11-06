use crate::{
    desktop, extensions, history, instance::Instance, permission, profile, recovery,
    session::SessionWriter, settings, start_page, state::BrowserState, ui, updater,
};
use serde::Deserialize;
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
};
use tao::platform::windows::{WindowBuilderExtWindows, WindowExtWindows};
use tao::{
    dpi::{LogicalPosition, LogicalSize, Position, Size},
    event::{ElementState, Event, MouseButton, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
    window::{CursorIcon, Fullscreen, Icon, ResizeDirection, Window, WindowBuilder},
};
use wry::{NewWindowResponse, PageLoadEvent, Rect, WebContext, WebView, WebViewBuilder};

/// Hidden tabs past this sleep even under the live-WebView budget, releasing their renderer.
const IDLE_SLEEP: std::time::Duration = std::time::Duration::from_secs(30 * 60);
/// Background tabs past this have their renderer memory trimmed.
const MEMORY_TRIM_DELAY: std::time::Duration = std::time::Duration::from_secs(30);

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Command {
    Navigate {
        url: String,
    },
    Back,
    Forward,
    Reload,
    ReloadFresh,
    ResourceLoadFailed {
        document_url: String,
        resource_url: String,
    },
    Stop,
    NewTab {
        url: Option<String>,
    },
    SwitchTab {
        id: usize,
    },
    CloseTab {
        id: usize,
    },
    ToggleSleep {
        id: usize,
    },
    SleepInactiveTabs,
    ClearUnpinnedTabs,
    ToggleSidebar,
    Peek {
        open: bool,
    },
    TopbarPeek {
        open: bool,
    },
    WindowMinimize,
    WindowMaximize,
    WindowClose,
    DragWindow,
    SwitchProfile {
        index: usize,
    },
    CreateProfile {
        name: String,
    },
    SwitchWorkspace {
        id: usize,
    },
    RestoreClosedTab,
    TogglePin {
        id: usize,
    },
    DuplicateTab {
        id: usize,
    },
    MoveTabToWorkspace {
        id: usize,
        workspace_id: usize,
    },
    ReorderTab {
        id: usize,
        target: usize,
        after: bool,
    },
    RenameWorkspace {
        id: usize,
        name: String,
    },
    DeleteWorkspace {
        id: usize,
        destination: usize,
    },
    SetWorkspaceEmoji {
        id: usize,
        emoji: String,
    },
    ToggleMute {
        id: usize,
    },
    ClearSiteData {
        id: usize,
        origin: String,
    },
    FocusAddress,
    AddressSuggestions,
    CloseActiveTab,
    CycleTab {
        backwards: bool,
    },
    ToggleFullscreen,
    ResizeWindow {
        direction: String,
    },
    ExpandPopup {
        url: String,
    },
    ClosePopup,
    SplitTab {
        id: usize,
    },
    OpenInSidePanel {
        id: usize,
    },
    ToggleSidePanel,
    Unsplit,
    Glance {
        url: String,
    },
    CheckUpdates,
    ApplyUpdate,
    DismissUpdate {
        version: String,
    },
    CloseDownloads,
    ToggleDownloads,
    ClearDownloads,
    OpenDownload {
        id: usize,
    },
    CancelDownload {
        id: usize,
    },
    RevealDownload {
        id: usize,
    },
    OpenDownloads,
    DefaultBrowser,
    OpenSettings {
        section: Option<String>,
    },
    OpenHistory,
    DeleteHistory {
        url: String,
    },
    ClearHistory,
    SetSetting {
        key: String,
        value: serde_json::Value,
    },
    InstallAdBlocker,
    OpenExtensionsFolder,
    ClearBrowsingData {
        range: String,
        kinds: desktop::DataKinds,
    },
    ImportBookmarks {
        browser: String,
    },
    ToggleReader,
    Screenshot,
    Zoom {
        delta: i32,
    },
    Print,
    Exit,
    DismissNewTab,
    SubmitNewTab {
        url: String,
    },
    Retry,
    ToggleBookmark {
        url: Option<String>,
    },
    NewPrivateTab,
}

/// Browser shortcuts by Windows virtual-key code, matched before the page sees the key.
pub fn shortcut(key: u32, ctrl: bool, shift: bool, alt: bool) -> Option<Command> {
    let letter = char::from_u32(key).filter(char::is_ascii_alphanumeric);
    Some(match (ctrl, alt) {
        (true, false) => match (letter, key, shift) {
            (Some('T'), ..) if shift => Command::RestoreClosedTab,
            (Some('T'), ..) => Command::NewTab { url: None },
            (Some('B'), _, true) => Command::ToggleSidebar,
            (Some('S'), _, true) => Command::SleepInactiveTabs,
            (Some('N'), _, true) => Command::NewPrivateTab,
            (Some('W'), _, false) => Command::CloseActiveTab,
            (Some('L'), _, false) => Command::FocusAddress,
            (Some('J'), _, false) => Command::OpenDownloads,
            (Some('H'), _, false) => Command::OpenHistory,
            (Some('R'), _, false) => Command::Reload,
            (Some('R'), _, true) | (_, 0x74, _) => Command::ReloadFresh,
            (Some('D'), _, false) => Command::ToggleBookmark { url: None },
            (_, 0x09, _) => Command::CycleTab { backwards: shift },
            (_, 0x21, _) => Command::CycleTab { backwards: true },
            (_, 0x22, _) => Command::CycleTab { backwards: false },
            (_, 0x2E, true) => Command::OpenSettings {
                section: Some("privacy".into()),
            },
            (_, 0xBB | 0x6B, _) => Command::Zoom { delta: 10 },
            (_, 0xBD | 0x6D, _) => Command::Zoom { delta: -10 },
            (Some('0'), _, false) | (_, 0x60, false) => Command::Zoom { delta: 0 },
            _ => return None,
        },
        (false, true) if !shift => match (letter, key) {
            (_, 0x25) => Command::Back,
            (_, 0x27) => Command::Forward,
            (Some('D'), _) => Command::FocusAddress,
            (Some(digit @ '1'..='9'), _) => Command::SwitchProfile {
                index: digit as usize - '0' as usize,
            },
            _ => return None,
        },
        (false, false) if !shift => match key {
            0x74 => Command::Reload,
            0x78 => Command::ToggleReader,
            0x7A => Command::ToggleFullscreen,
            _ => return None,
        },
        _ => return None,
    })
}

#[derive(Deserialize)]
struct ContentCommand {
    #[serde(flatten)]
    command: Command,
    shortcut_token: Option<String>,
}

#[derive(Debug)]
pub enum UserEvent {
    Audio(usize, u64, bool, bool),
    SiteDataCleared(usize, u64, String, Result<(), String>),
    SiteFilesCleared(recovery::Page, Result<(), String>),
    FreshReloadFailed(recovery::Page, String),
    ResourceLoadFailed(usize, u64, String, String),
    Maintenance,
    SessionSaved(Result<(), String>),
    SleepChecked(usize, u64, u64, String, bool),
    CaptureAllowed(usize, u64),
    PermissionRequested,
    PermissionReady,
    PermissionResponse(u64, bool),
    SmokeTick,
    SmokeResult(usize, bool),
    Command(Command),
    Ready,
    PaletteReady,
    TopbarReady,
    PopupReady,
    PopupTitle(String),
    PopupState(String, bool),
    PopupClosed,
    Frame,
    External(Vec<String>),
    Title(usize, u64, String),
    PageLoad(usize, u64, bool, String),
    History(usize, u64, bool, bool),
    Source(usize, u64, String),
    IncompleteLoad(usize, u64, String),
    Failed(usize, u64, String, String),
    Crashed(usize, u64),
    Focused(usize, u64),
    Favicon(usize, u64, String),
    DownloadsChanged,
    DownloadStarted(usize, u64, bool, Option<String>),
    DownloadFinished(usize, u64, desktop::DownloadOutcome),
    UpdateChecked(Result<Option<updater::UpdateManifest>, String>),
    Status(String),
    UpdateFailed(String),
    AdBlockerInstalled(Result<(), String>),
    BrowsingDataCleared(Result<(), String>, &'static str),
    ScreenshotSaved(Result<std::path::PathBuf, String>),
    ReaderToggled(usize, String),
    ContentFullscreen(usize, u64, bool),
}

/// Fullscreens the window. Windows pins a maximized window's client area to the work area, so the
/// maximized state has to be cleared first or the page keeps the taskbar-sized gap it had.
fn fullscreen_window(window: &Window, maximize_after: &mut bool) {
    *maximize_after = window.is_maximized();
    if *maximize_after {
        window.set_maximized(false);
    }
    window.set_fullscreen(Some(Fullscreen::Borderless(None)));
}

/// Returns the window to its normal frame and to the maximized state it had before fullscreen.
fn restore_window(window: &Window, maximize_after: &mut bool) {
    window.set_fullscreen(None);
    if std::mem::take(maximize_after) {
        window.set_maximized(true);
    }
}

/// Leaves HTML fullscreen for the tab that entered it. The window restores only if the app fullscreened it.
fn leave_content_fullscreen(
    window: &Window,
    views: &HashMap<usize, WebView>,
    content: &mut Option<(usize, bool)>,
    maximize_after: &mut bool,
) {
    let Some((id, owns_window)) = content.take() else {
        return;
    };
    if owns_window {
        restore_window(window, maximize_after);
    }
    if let Some(view) = views.get(&id) {
        let _ = view.evaluate_script("if (document.fullscreenElement) document.exitFullscreen()");
    }
}

fn bounds(window: &Window, state: &BrowserState, side: f64, top: f64) -> (Rect, Rect, Rect, Rect) {
    let size = window.inner_size().to_logical::<f64>(window.scale_factor());
    let full = ui::SIDEBAR_WIDTH_EXPANDED;
    let hot = ui::SIDEBAR_HOT_ZONE;
    let (side_x, side_width) = if !state.sidebar_collapsed {
        (0., full)
    } else {
        let right = (full * side).max(hot);
        // Keep WebView2 at full width so revealing it never resizes its surface.
        (right - full, full)
    };
    let right = settings::sidebar_right();
    // The right-hand layout mirrors the left one horizontally.
    let side_x = if right {
        size.width - side_x - side_width
    } else {
        side_x
    };
    let top_left = if state.sidebar_collapsed || right {
        0.
    } else {
        full
    };
    let top_width = if right && !state.sidebar_collapsed {
        size.width - full
    } else {
        size.width - top_left
    };
    let margin = if window.is_maximized() || window.fullscreen().is_some() {
        0.
    } else {
        ui::WINDOW_MARGIN
    };
    let content_left = if state.sidebar_collapsed || right {
        margin
    } else {
        full + margin
    };
    let content_right = if state.sidebar_collapsed || !right {
        margin
    } else {
        full + margin
    };
    let top_height = ui::TOPBAR_HOT_ZONE + (ui::TOPBAR_EXPANDED - ui::TOPBAR_HOT_ZONE) * top;
    (
        Rect {
            position: Position::Logical(LogicalPosition::new(side_x, 0.)),
            size: Size::Logical(LogicalSize::new(side_width, size.height)),
        },
        Rect {
            position: Position::Logical(LogicalPosition::new(content_left, margin)),
            size: Size::Logical(LogicalSize::new(
                (size.width - content_left - content_right).max(10.),
                (size.height - margin * 2.).max(10.),
            )),
        },
        Rect {
            position: Position::Logical(LogicalPosition::new(0., 0.)),
            size: Size::Logical(size),
        },
        Rect {
            position: Position::Logical(LogicalPosition::new(top_left, 0.)),
            size: Size::Logical(LogicalSize::new(top_width.max(10.), top_height)),
        },
    )
}

/// Takes the side web panel from the content edge away from the sidebar.
fn side_panel_bounds(content: Rect, scale: f64, sidebar_right: bool) -> (Rect, Rect) {
    let position = content.position.to_logical::<f64>(scale);
    let size = content.size.to_logical::<f64>(scale);
    let width = ui::SIDE_PANEL_WIDTH.min(size.width * 0.45);
    let rest = (size.width - width - ui::WINDOW_MARGIN).max(10.);
    let (panel_x, content_x) = if sidebar_right {
        (position.x, position.x + width + ui::WINDOW_MARGIN)
    } else {
        (position.x + rest + ui::WINDOW_MARGIN, position.x)
    };
    let rect = |x: f64, width: f64| Rect {
        position: Position::Logical(LogicalPosition::new(x, position.y)),
        size: Size::Logical(LogicalSize::new(width, size.height)),
    };
    (rect(content_x, rest), rect(panel_x, width))
}

#[cfg(windows)]
fn content_browser_arguments() -> String {
    let mut args = String::from(
        "--enable-features=BackForwardCache,ParallelDownloading,CanvasOopRasterization,IntensiveWakeUpThrottling,ThrottleDisplayNoneAndVisibilityHiddenCrossOriginIframes,CalculateNativeWinOcclusion,WebRtcHideLocalIpsWithMdns \
         --disable-features=Translate,OptimizationHints,MediaRouter \
         --enable-gpu-rasterization \
         --enable-zero-copy \
         --enable-smooth-scrolling \
         --enable-hardware-overlays=\"single-fullscreen,single-on-top,underlay\" \
         --disk-cache-size=1073741824 \
         --media-cache-size=268435456 \
         --disable-background-timer-throttling=false \
         --disable-renderer-backgrounding=false",
    );
    if crate::settings::performance_mode() {
        args.push_str(" --enable-low-end-device-mode");
    }
    if let Ok(extra) = std::env::var("VELUNO_SMOKE_CONTENT_BROWSER_ARGUMENTS") {
        args.push(' ');
        args.push_str(&extra);
    }
    args
}

#[cfg(windows)]
pub(crate) fn chrome_browser_arguments() -> String {
    let mut args = String::from(
        "--enable-gpu-rasterization \
         --enable-zero-copy \
         --enable-smooth-scrolling \
         --disable-features=Translate,OptimizationHints,MediaRouter",
    );
    if let Ok(extra) = std::env::var("VELUNO_SMOKE_BROWSER_ARGUMENTS") {
        args.push(' ');
        args.push_str(&extra);
    }
    args
}

/// The docked side web panel. It shares the tab profile, so sign-ins carry over.
fn side_panel(
    window: &Window,
    context: &mut WebContext,
    url: &str,
    bounds: Rect,
    proxy: &EventLoopProxy<UserEvent>,
) -> wry::Result<WebView> {
    let nav_proxy = proxy.clone();
    let tab_proxy = proxy.clone();
    let mut builder = WebViewBuilder::new_with_web_context(context)
        .with_bounds(bounds)
        .with_focused(false)
        .with_clipboard(true)
        .with_navigation_handler(move |target| {
            let allowed = profile::allowed_url(&target)
                || target == "about:blank"
                || target.starts_with("data:text/html;");
            if !allowed {
                let _ = nav_proxy.send_event(UserEvent::Status(
                    "This link uses an unsupported application protocol.".into(),
                ));
            }
            allowed
        })
        .with_new_window_req_handler(move |target, _| {
            if profile::allowed_url(&target) {
                let _ =
                    tab_proxy.send_event(UserEvent::Command(Command::NewTab { url: Some(target) }));
            }
            NewWindowResponse::Deny
        });
    // Match the tab profile's environment options, as Glance does.
    #[cfg(windows)]
    {
        use wry::WebViewBuilderExtWindows;
        builder = builder.with_additional_browser_args(content_browser_arguments());
        if extensions::enabled_at_startup() {
            builder = builder.with_browser_extensions_enabled(true);
        }
    }
    let view = builder.with_url(url).build_as_child(window)?;
    desktop::block_view_ads(&view);
    let _ = desktop::apply_privacy(&view, &settings::get());
    Ok(view)
}

/// Splits the content area into two side-by-side panes with a window-margin gap.
fn split_bounds(content: Rect, scale: f64) -> (Rect, Rect) {
    let position = content.position.to_logical::<f64>(scale);
    let size = content.size.to_logical::<f64>(scale);
    let width = ((size.width - ui::WINDOW_MARGIN) / 2.).max(10.);
    let pane = |x: f64| Rect {
        position: Position::Logical(LogicalPosition::new(x, position.y)),
        size: Size::Logical(LogicalSize::new(width, size.height)),
    };
    (
        pane(position.x),
        pane(position.x + width + ui::WINDOW_MARGIN),
    )
}

fn step(progress: f64, target: f64, dt: f64, snap: bool) -> f64 {
    if snap {
        return target;
    }
    let amount = dt / if target > progress { 0.30 } else { 0.22 };
    if target > progress {
        (progress + amount).min(target)
    } else {
        (progress - amount).max(target)
    }
}

fn ease(progress: f64) -> f64 {
    1. - (1. - progress).powi(3)
}

#[derive(Clone)]
struct PopupHost {
    main: Rc<Window>,
    window: Rc<Window>,
    view: Rc<RefCell<Option<WebView>>>,
    url: Rc<RefCell<String>>,
    auth_seen: Rc<Cell<bool>>,
    /// The window holds a Glance link preview rather than a page-opened popup.
    glance: Rc<Cell<bool>>,
    headless: bool,
}

fn popup_bounds(window: &Window) -> (Rect, Rect) {
    let size = window.inner_size().to_logical::<f64>(window.scale_factor());
    let bar = ui::POPUP_BAR_HEIGHT;
    (
        Rect {
            position: Position::Logical(LogicalPosition::new(0., 0.)),
            size: Size::Logical(LogicalSize::new(size.width.max(10.), bar)),
        },
        Rect {
            position: Position::Logical(LogicalPosition::new(0., bar)),
            size: Size::Logical(LogicalSize::new(
                size.width.max(10.),
                (size.height - bar).max(10.),
            )),
        },
    )
}

fn parse_resize_direction(direction: &str) -> Option<ResizeDirection> {
    match direction {
        "n" => Some(ResizeDirection::North),
        "s" => Some(ResizeDirection::South),
        "e" => Some(ResizeDirection::East),
        "w" => Some(ResizeDirection::West),
        "ne" => Some(ResizeDirection::NorthEast),
        "nw" => Some(ResizeDirection::NorthWest),
        "se" => Some(ResizeDirection::SouthEast),
        "sw" => Some(ResizeDirection::SouthWest),
        _ => None,
    }
}

fn border_resize_direction(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Option<(ResizeDirection, CursorIcon)> {
    let border = ui::WINDOW_MARGIN;
    let corner = 12.;
    let left = x < border;
    let right = x >= width - border;
    let top = y < border;
    let bottom = y >= height - border;
    if !(left || right || top || bottom) {
        return None;
    }
    let near_left = x < corner;
    let near_right = x >= width - corner;
    let near_top = y < corner;
    let near_bottom = y >= height - corner;
    Some(match (near_left, near_right, near_top, near_bottom) {
        (true, _, true, _) => (ResizeDirection::NorthWest, CursorIcon::NwResize),
        (_, true, true, _) => (ResizeDirection::NorthEast, CursorIcon::NeResize),
        (true, _, _, true) => (ResizeDirection::SouthWest, CursorIcon::SwResize),
        (_, true, _, true) => (ResizeDirection::SouthEast, CursorIcon::SeResize),
        _ if left => (ResizeDirection::West, CursorIcon::WResize),
        _ if right => (ResizeDirection::East, CursorIcon::EResize),
        _ if top => (ResizeDirection::North, CursorIcon::NResize),
        _ => (ResizeDirection::South, CursorIcon::SResize),
    })
}

#[cfg(windows)]
fn open_popup(
    host: &PopupHost,
    url: &str,
    features: &wry::NewWindowFeatures,
    private: bool,
    proxy: &EventLoopProxy<UserEvent>,
) -> Option<NewWindowResponse> {
    use wry::{WebViewBuilderExtWindows, WebViewExtWindows};
    let size = features
        .size
        .map(|s| LogicalSize::new(s.width.clamp(380., 920.), s.height.clamp(440., 860.)))
        .unwrap_or_else(|| LogicalSize::new(500., 660.));
    host.window.set_inner_size(size);
    if let Some(pos) = features.position {
        host.window.set_outer_position(pos);
    } else if let Ok(outer) = host.main.outer_position() {
        let scale = host.main.scale_factor();
        let main_pos = outer.to_logical::<f64>(scale);
        let main_size = host.main.inner_size().to_logical::<f64>(scale);
        let x = main_pos.x + ((main_size.width - size.width) * 0.5).max(24.);
        let y = main_pos.y + ((main_size.height - size.height) * 0.5).max(32.);
        host.window.set_outer_position(LogicalPosition::new(x, y));
    }
    host.window.set_title("Sign in — Veluno");
    host.glance.set(false);
    if !host.headless {
        host.window.set_minimized(false);
        host.window.set_visible(true);
        host.window.set_focus();
    }
    *host.url.borrow_mut() = url.to_string();
    host.auth_seen.set(false);
    let title_proxy = proxy.clone();
    let nav_proxy = proxy.clone();
    let tab_proxy = proxy.clone();
    let (_, content_rect) = popup_bounds(&host.window);
    let mut builder = WebViewBuilder::new()
        .with_environment(features.opener.environment.clone())
        .with_incognito(private)
        .with_bounds(content_rect)
        .with_focused(!host.headless)
        .with_clipboard(true)
        .with_document_title_changed_handler(move |text| {
            let _ = title_proxy.send_event(UserEvent::PopupTitle(text));
        })
        .with_navigation_handler(move |target| {
            let allowed = profile::allowed_url(&target)
                || target == "about:blank"
                || target.starts_with("data:text/html;");
            if !allowed {
                let _ = nav_proxy.send_event(UserEvent::Status(
                    "This link uses an unsupported application protocol.".into(),
                ));
            }
            allowed
        })
        .with_new_window_req_handler(move |target, _| {
            if profile::allowed_url(&target) {
                let _ =
                    tab_proxy.send_event(UserEvent::Command(Command::NewTab { url: Some(target) }));
            }
            NewWindowResponse::Deny
        });
    #[cfg(windows)]
    {
        builder = builder.with_additional_browser_args(content_browser_arguments());
    }
    let view = builder.build_as_child(&*host.window).ok()?;
    let _ = desktop::attach_popup(&view, proxy.clone());
    let raw = view.webview();
    *host.view.borrow_mut() = Some(view);
    let _ = proxy.send_event(UserEvent::PopupState(url.to_string(), true));
    Some(NewWindowResponse::Create { webview: raw })
}

/// Lets Escape or Ctrl+W inside a Glance preview dismiss it, unless the page handles the key.
const GLANCE_SCRIPT: &str = r#"addEventListener('keydown',e=>{if(!e.isTrusted||e.defaultPrevented||e.isComposing||window.top!==window)return;const k=e.key.toLowerCase(),plain=!e.ctrlKey&&!e.altKey&&!e.shiftKey&&!e.metaKey;if((plain&&k==='escape')||(e.ctrlKey&&!e.altKey&&!e.shiftKey&&k==='w')){e.preventDefault();(window.chrome?.webview??window.ipc)?.postMessage('{"action":"close_popup"}');}},true);"#;

/// Opens a link preview in the popup window, centered over the browser and sharing the tab profile.
fn open_glance(
    host: &PopupHost,
    context: &mut WebContext,
    url: &str,
    private: bool,
    proxy: &EventLoopProxy<UserEvent>,
) -> wry::Result<()> {
    *host.view.borrow_mut() = None;
    let scale = host.main.scale_factor();
    let main_size = host.main.inner_size().to_logical::<f64>(scale);
    let size = LogicalSize::new(
        (main_size.width * 0.82).max(360.),
        (main_size.height * 0.86).max(420.),
    );
    host.window.set_inner_size(size);
    if let Ok(outer) = host.main.outer_position() {
        let main_pos = outer.to_logical::<f64>(scale);
        host.window.set_outer_position(LogicalPosition::new(
            main_pos.x + (main_size.width - size.width) * 0.5,
            main_pos.y + (main_size.height - size.height) * 0.5,
        ));
    }
    host.window.set_title("Glance — Veluno");
    host.glance.set(true);
    host.auth_seen.set(false);
    *host.url.borrow_mut() = url.to_string();
    if !host.headless {
        host.window.set_minimized(false);
        host.window.set_visible(true);
        host.window.set_focus();
    }
    let title_proxy = proxy.clone();
    let nav_proxy = proxy.clone();
    let tab_proxy = proxy.clone();
    let ipc_proxy = proxy.clone();
    let mut builder = WebViewBuilder::new_with_web_context(context)
        .with_incognito(private)
        .with_bounds(popup_bounds(&host.window).1)
        .with_focused(!host.headless)
        .with_clipboard(true)
        .with_initialization_script(GLANCE_SCRIPT)
        .with_document_title_changed_handler(move |text| {
            let _ = title_proxy.send_event(UserEvent::PopupTitle(text));
        })
        .with_navigation_handler(move |target| {
            let allowed = profile::allowed_url(&target)
                || target == "about:blank"
                || target.starts_with("data:text/html;");
            if !allowed {
                let _ = nav_proxy.send_event(UserEvent::Status(
                    "This link uses an unsupported application protocol.".into(),
                ));
            }
            allowed
        })
        .with_new_window_req_handler(move |target, _| {
            if profile::allowed_url(&target) {
                let _ =
                    tab_proxy.send_event(UserEvent::Command(Command::NewTab { url: Some(target) }));
            }
            NewWindowResponse::Deny
        });
    let handle_ipc = move |_: &str, body: &str| {
        if let Ok(Command::ClosePopup) = serde_json::from_str::<Command>(body) {
            let _ = ipc_proxy.send_event(UserEvent::Command(Command::ClosePopup));
        }
    };
    // WebView2 rejects a second view on the tab profile unless its environment options match.
    #[cfg(windows)]
    {
        use wry::WebViewBuilderExtWindows;
        builder = builder.with_additional_browser_args(content_browser_arguments());
        if extensions::enabled_at_startup() {
            builder = builder.with_browser_extensions_enabled(true);
        }
    }
    let view = builder.with_url(url).build_as_child(&*host.window)?;
    let _ = desktop::attach_ipc(&view, handle_ipc);
    let _ = desktop::attach_popup(&view, proxy.clone());
    let _ = desktop::apply_privacy(&view, &settings::get());
    *host.view.borrow_mut() = Some(view);
    let _ = proxy.send_event(UserEvent::PopupState(url.to_string(), true));
    Ok(())
}

#[cfg(not(windows))]
fn open_popup(
    _: &PopupHost,
    _: &str,
    _: &wry::NewWindowFeatures,
    _: bool,
    _: &EventLoopProxy<UserEvent>,
) -> Option<NewWindowResponse> {
    None
}

fn load(view: &WebView, state: &BrowserState, url: &str) -> wry::Result<()> {
    if url == "veluno://newtab" {
        view.load_html(&start_page::get_start_page_html(state))
    } else if url == "veluno://blank" {
        view.load_html(&format!("<!DOCTYPE html><html lang=\"en\"><meta charset=\"UTF-8\"><title>New Tab</title><style>:root{{color-scheme:dark}}body{{margin:0;background:#0c0c0e}}</style><script>{}</script></html>", ui::script()))
    } else if url == "veluno://settings" {
        view.load_html(&ui::get_settings_html())
    } else if url == "veluno://history" {
        view.load_html(&ui::get_history_html(&state.bookmarks, &state.shortcuts))
    } else {
        view.load_url(url)
    }
}

fn content(
    window: &Window,
    context: &mut WebContext,
    state: &mut BrowserState,
    id: usize,
    proxy: &EventLoopProxy<UserEvent>,
    popup_host: &PopupHost,
    fresh: bool,
) -> wry::Result<WebView> {
    static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let generation = GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let private = state.is_private(id);
    let tab = state
        .tabs
        .iter_mut()
        .find(|tab| tab.id == id)
        .ok_or_else(|| std::io::Error::other("This tab no longer exists."))?;
    let url = tab.url.clone();
    let failed = tab.error.is_some();
    let zoom = tab.zoom;
    tab.generation = generation;
    tab.has_capture = false;
    let token = generation.to_string();
    let script = ui::script().replace("__SHORTCUT_TOKEN__", &token);
    let ipc = proxy.clone();
    let title = proxy.clone();
    let popup = proxy.clone();
    let popup_host = popup_host.clone();
    let navigation = proxy.clone();
    let mut builder = WebViewBuilder::new_with_web_context(context)
        .with_bounds(bounds(window, state, 0., 0.).1)
        .with_visible(false)
        .with_focused(false)
        .with_incognito(private)
        .with_initialization_script(&script)
        .with_initialization_script(include_str!("sleep.js"))
        .with_document_title_changed_handler(move |text| {
            let _ = title.send_event(UserEvent::Title(id, generation, text));
        })
        .with_navigation_handler(move |url| {
            let allowed = profile::allowed_url(&url)
                || url == "about:blank"
                || url.starts_with("data:text/html;");
            if !allowed {
                let _ = navigation.send_event(UserEvent::Status(
                    "This link uses an unsupported application protocol.".into(),
                ));
            }
            allowed
        })
        .with_new_window_req_handler(move |url, features| {
            let is_popup = features.size.is_some()
                || features.position.is_some()
                || profile::is_auth_url(&url);
            if is_popup
                && (url == "about:blank" || profile::allowed_url(&url))
                && let Some(response) = open_popup(&popup_host, &url, &features, private, &popup)
            {
                return response;
            }
            if profile::allowed_url(&url) {
                let _ = popup.send_event(UserEvent::Command(Command::NewTab { url: Some(url) }));
            }
            NewWindowResponse::Deny
        });
    let handle_ipc = move |uri: &str, body: &str| {
        if let Ok(message) = serde_json::from_str::<ContentCommand>(body) {
            let local = url::Url::parse(uri).is_ok_and(|url| url.scheme() == "data")
                || uri == "about:blank";
            let trusted = message.shortcut_token.as_deref() == Some(&token);
            if trusted
                && let Command::ResourceLoadFailed {
                    document_url,
                    resource_url,
                } = &message.command
            {
                let _ = ipc.send_event(UserEvent::ResourceLoadFailed(
                    id,
                    generation,
                    document_url.clone(),
                    resource_url.clone(),
                ));
                return;
            }
            if (local
                && matches!(
                    message.command,
                    Command::Navigate { .. }
                        | Command::Retry
                        | Command::SetSetting { .. }
                        | Command::DeleteHistory { .. }
                        | Command::ClearHistory
                        | Command::InstallAdBlocker
                        | Command::OpenExtensionsFolder
                        | Command::DefaultBrowser
                        | Command::CheckUpdates
                        | Command::ClearBrowsingData { .. }
                        | Command::ImportBookmarks { .. }
                        | Command::ToggleBookmark { url: Some(_) }
                ))
                || (trusted
                    && matches!(
                        message.command,
                        Command::Back
                            | Command::Forward
                            | Command::Reload
                            | Command::ReloadFresh
                            | Command::Stop
                            | Command::NewTab { .. }
                            | Command::CloseActiveTab
                            | Command::FocusAddress
                            | Command::ToggleSidebar
                            | Command::RestoreClosedTab
                            | Command::SleepInactiveTabs
                            | Command::CycleTab { .. }
                            | Command::SwitchProfile { .. }
                            | Command::OpenDownloads
                            | Command::ToggleDownloads
                            | Command::OpenHistory
                            | Command::Zoom { .. }
                            | Command::ToggleFullscreen
                            | Command::ToggleBookmark { url: None }
                            | Command::NewPrivateTab
                            | Command::ToggleReader
                            | Command::OpenSettings { .. }
                            | Command::Glance { .. }
                    ))
            {
                let _ = ipc.send_event(UserEvent::Command(message.command));
            }
        }
    };
    #[cfg(windows)]
    {
        use wry::WebViewBuilderExtWindows;
        builder = builder.with_additional_browser_args(content_browser_arguments());
    }
    if std::env::var_os("VELUNO_SMOKE_TEST").is_some() {
        builder = builder.with_download_started_handler(|_, path| {
            *path = profile::directory().join("smoke-download.txt");
            true
        });
    }
    #[cfg(windows)]
    if extensions::enabled_at_startup() {
        use wry::WebViewBuilderExtWindows;
        let _ = std::fs::create_dir_all(extensions::directory());
        builder = builder.with_browser_extensions_enabled(true);
    }
    let view = builder.build_as_child(window)?;
    if zoom != 100 {
        let _ = view.zoom(f64::from(zoom) / 100.);
    }
    let _ = desktop::attach_ipc(&view, handle_ipc);
    desktop::attach(&view, id, generation, proxy.clone())?;
    if !private && extensions::enabled_at_startup() {
        let _ = desktop::load_extensions(&view, &extensions::directory());
    }
    // Older WebView2 runtimes lack some of these settings; pages still work without them.
    let _ = desktop::apply_privacy(&view, &settings::get());
    if let Some(tab) = state.tabs.iter().find(|tab| tab.id == id) {
        let _ = desktop::mute(&view, tab.is_muted);
    }
    if failed {
        view.load_html(include_str!("error.html"))?;
    } else {
        if fresh && (url.starts_with("https://") || url.starts_with("http://")) {
            let mut headers = wry::http::HeaderMap::new();
            headers.insert(
                wry::http::header::CACHE_CONTROL,
                wry::http::HeaderValue::from_static("no-cache"),
            );
            view.load_url_with_headers(&url, headers)?;
        } else {
            load(&view, state, &url)?;
        }
        if let Some(tab) = state.tabs.iter_mut().find(|tab| tab.id == id) {
            tab.is_loading = true;
        }
    }
    Ok(view)
}

/// Applies a page event from one tab's WebView, ignoring events from a replaced WebView.
fn tab_event(
    state: &mut BrowserState,
    views: &mut HashMap<usize, WebView>,
    prompt: &mut permission::Prompt,
    event: UserEvent,
) {
    let current = |state: &BrowserState, id, generation| {
        state
            .tabs
            .iter()
            .position(|tab| tab.id == id && tab.generation == generation)
    };
    match event {
        UserEvent::Title(id, generation, title) => {
            let private = state.is_private(id);
            if let Some(index) = current(state, id, generation)
                && state.tabs[index].error.is_none()
                && !title.is_empty()
            {
                if !private {
                    history::retitle(&state.tabs[index].url, &title);
                }
                state.tabs[index].title = title;
            }
        }
        UserEvent::PageLoad(id, generation, loading, url) => {
            let private = state.is_private(id);
            let Some(index) = current(state, id, generation) else {
                return;
            };
            let tab = &mut state.tabs[index];
            if loading {
                prompt.cancel_tab(id);
                if tab.pending_url.as_ref().is_some_and(|expected| {
                    expected != &url && !(profile::is_internal(expected) && url == "about:blank")
                }) {
                    return;
                }
                tab.pending_url = None;
                if let Some(url) = profile::normalize_url(&url) {
                    tab.url = url;
                    tab.error = None;
                } else if tab.error.is_none()
                    && (url == "about:blank" || url.starts_with("data:text/html;"))
                    && !profile::is_internal(&tab.url)
                {
                    tab.url = "veluno://newtab".into();
                }
                tab.is_loading = true;
            } else {
                if tab.pending_url.as_ref().is_some_and(|expected| {
                    expected != &url && !(profile::is_internal(expected) && url == "about:blank")
                }) {
                    return;
                }
                let failed = tab.error.is_some();
                tab.is_loading = false;
                tab.pending_url = None;
                if !failed
                    && !profile::is_internal(&tab.url)
                    && let Some(normalized) = profile::normalize_url(&url)
                {
                    tab.url = normalized;
                }
                if !failed && (tab.url == url || profile::is_internal(&tab.url)) {
                    tab.document_url = Some(tab.url.clone());
                }
                if tab.url == url && profile::allowed_url(&url) {
                    tab.error = None;
                }
                if tab.url == url && !failed && !private {
                    history::visit(&tab.url, &tab.title, tab.favicon.as_deref());
                }
            }
        }
        UserEvent::Source(id, generation, url) => {
            if let Some(index) = current(state, id, generation)
                && state.tabs[index].pending_url.is_none()
                && let Some(url) = profile::normalize_url(&url)
            {
                state.tabs[index].url = url;
            }
        }
        UserEvent::History(id, generation, back, forward) => {
            if let Some(index) = current(state, id, generation) {
                state.tabs[index].can_go_back = back;
                state.tabs[index].can_go_forward = forward;
            }
        }
        UserEvent::Failed(id, generation, url, message) => {
            if let Some(index) = current(state, id, generation) {
                let tab = &mut state.tabs[index];
                if tab
                    .pending_url
                    .as_ref()
                    .is_none_or(|pending| pending == &url)
                    && tab.url == url
                {
                    tab.error = Some(message);
                    tab.pending_url = None;
                    tab.is_loading = false;
                }
            }
        }
        UserEvent::Favicon(id, generation, uri) => {
            if let Some(index) = current(state, id, generation) {
                let web = uri.starts_with("https://") || uri.starts_with("http://");
                if web && !state.is_private(id) {
                    history::set_favicon(&state.tabs[index].url, &uri);
                }
                state.set_favicon(id, web.then_some(uri));
            }
        }
        UserEvent::Crashed(id, generation) => {
            if let Some(index) = current(state, id, generation) {
                views.remove(&id);
                let tab = &mut state.tabs[index];
                tab.error = Some("This tab stopped responding. Reload it to continue.".into());
                tab.is_loading = false;
            }
        }
        _ => {}
    }
}

/// A Clear browsing data range: its length in milliseconds (`None` is all time) and a label.
fn clear_range(range: &str) -> Option<(Option<u64>, &'static str)> {
    const HOUR: u64 = 60 * 60 * 1000;
    Some(match range {
        "hour" => (Some(HOUR), "the last hour"),
        "day" => (Some(24 * HOUR), "the last 24 hours"),
        "week" => (Some(7 * 24 * HOUR), "the last 7 days"),
        "month" => (Some(28 * 24 * HOUR), "the last 4 weeks"),
        "all" => (None, "all time"),
        _ => return None,
    })
}

fn may_exit(state: &BrowserState, session: &SessionWriter) -> bool {
    history::flush();
    if state.active_downloads > 0
        && !desktop::message(
            "Downloads in progress",
            "Closing Veluno will cancel active downloads. Close anyway?",
            true,
        )
    {
        return false;
    }
    if let Err(error) = session.flush(state)
        && !desktop::message(
            "Session could not be saved",
            &format!("{error}\n\nClose without saving this session?"),
            true,
        )
    {
        return false;
    }
    true
}

fn take_tabs_to_trim(
    hidden_at: &mut HashMap<usize, (std::time::Instant, bool)>,
    active_tab_id: usize,
) -> Vec<usize> {
    let mut tabs = Vec::new();
    for (id, (at, trimmed)) in hidden_at.iter_mut() {
        if !*trimmed && at.elapsed() >= MEMORY_TRIM_DELAY && *id != active_tab_id {
            *trimmed = true;
            tabs.push(*id);
        }
    }
    tabs
}

fn build_chrome_views(
    window: &Window,
    popup_window: &Window,
    state: &BrowserState,
    chrome_context: &mut WebContext,
    proxy: &EventLoopProxy<UserEvent>,
    headless: bool,
) -> wry::Result<(WebView, WebView, permission::Prompt, WebView, WebView)> {
    let ui_proxy = proxy.clone();
    let ready = proxy.clone();
    let mut sidebar_builder = WebViewBuilder::new_with_web_context(chrome_context)
        .with_bounds(bounds(window, state, 0., 0.).0)
        .with_html(ui::get_ui_html())
        .with_focused(false)
        .with_clipboard(true)
        .with_navigation_handler(|url| url == "about:blank" || url.starts_with("data:text/html;"))
        .with_on_page_load_handler(move |event, _| {
            if matches!(event, PageLoadEvent::Finished) {
                let _ = ready.send_event(UserEvent::Ready);
            }
        })
        .with_ipc_handler(move |request| {
            if let Ok(command) = serde_json::from_str::<Command>(request.body()) {
                let _ = ui_proxy.send_event(UserEvent::Command(command));
            }
        });
    #[cfg(windows)]
    {
        use wry::WebViewBuilderExtWindows;
        sidebar_builder = sidebar_builder.with_additional_browser_args(chrome_browser_arguments());
    }
    let sidebar = sidebar_builder.build_as_child(window)?;
    let palette_proxy = proxy.clone();
    let palette_ready = proxy.clone();
    let mut palette_builder = WebViewBuilder::new_with_web_context(chrome_context)
        .with_bounds(bounds(window, state, 0., 0.).2)
        .with_visible(false)
        .with_focused(false)
        .with_transparent(true)
        .with_background_color((0, 0, 0, 0))
        .with_html(ui::get_new_tab_html())
        .with_navigation_handler(|url| url == "about:blank" || url.starts_with("data:text/html;"))
        .with_on_page_load_handler(move |event, _| {
            if matches!(event, PageLoadEvent::Finished) {
                let _ = palette_ready.send_event(UserEvent::PaletteReady);
            }
        })
        .with_ipc_handler(move |request| {
            if let Ok(
                command @ (Command::SubmitNewTab { .. }
                | Command::DismissNewTab
                | Command::SwitchTab { .. }),
            ) = serde_json::from_str::<Command>(request.body())
            {
                let _ = palette_proxy.send_event(UserEvent::Command(command));
            }
        });
    #[cfg(windows)]
    {
        use wry::WebViewBuilderExtWindows;
        palette_builder = palette_builder.with_additional_browser_args(chrome_browser_arguments());
    }
    let palette = palette_builder.build_as_child(window)?;
    let permission_prompt =
        permission::Prompt::new(window, chrome_context, proxy.clone(), headless)?;
    let topbar_ready = proxy.clone();
    let topbar_proxy = proxy.clone();
    let mut topbar_builder = WebViewBuilder::new_with_web_context(chrome_context)
        .with_bounds(bounds(window, state, 0., 0.).3)
        .with_html(ui::get_topbar_html())
        .with_focused(false)
        .with_navigation_handler(|url| url == "about:blank" || url.starts_with("data:text/html;"))
        .with_on_page_load_handler(move |event, _| {
            if matches!(event, PageLoadEvent::Finished) {
                let _ = topbar_ready.send_event(UserEvent::TopbarReady);
            }
        })
        .with_ipc_handler(move |request| {
            if let Ok(
                command @ (Command::TopbarPeek { .. }
                | Command::WindowMinimize
                | Command::WindowMaximize
                | Command::WindowClose
                | Command::DragWindow
                | Command::ResizeWindow { .. }),
            ) = serde_json::from_str::<Command>(request.body())
            {
                let _ = topbar_proxy.send_event(UserEvent::Command(command));
            }
        });
    #[cfg(windows)]
    {
        use wry::WebViewBuilderExtWindows;
        topbar_builder = topbar_builder.with_additional_browser_args(chrome_browser_arguments());
    }
    let topbar = topbar_builder.build_as_child(window)?;
    let popup_ready = proxy.clone();
    let popup_bar_proxy = proxy.clone();
    let mut popup_bar_builder = WebViewBuilder::new_with_web_context(chrome_context)
        .with_bounds(popup_bounds(popup_window).0)
        .with_html(ui::get_popup_html())
        .with_focused(false)
        .with_navigation_handler(|url| url == "about:blank" || url.starts_with("data:text/html;"))
        .with_on_page_load_handler(move |event, _| {
            if matches!(event, PageLoadEvent::Finished) {
                let _ = popup_ready.send_event(UserEvent::PopupReady);
            }
        })
        .with_ipc_handler(move |request| {
            if let Ok(command @ (Command::ExpandPopup { .. } | Command::ClosePopup)) =
                serde_json::from_str::<Command>(request.body())
            {
                let _ = popup_bar_proxy.send_event(UserEvent::Command(command));
            }
        });
    #[cfg(windows)]
    {
        use wry::WebViewBuilderExtWindows;
        popup_bar_builder =
            popup_bar_builder.with_additional_browser_args(chrome_browser_arguments());
    }
    let popup_bar = popup_bar_builder.build_as_child(popup_window)?;
    Ok((sidebar, palette, permission_prompt, topbar, popup_bar))
}

fn start_ad_blocker_install(proxy: &EventLoopProxy<UserEvent>) {
    let proxy = proxy.clone();
    extensions::install_in_background(move |result| {
        let _ = proxy.send_event(UserEvent::AdBlockerInstalled(result));
    });
}

pub fn run(args: &[String]) -> wry::Result<()> {
    if extensions::enabled_at_startup() {
        extensions::prepare();
    }
    let _ = extensions::active_at_startup();
    let headless = std::env::var_os("VELUNO_SMOKE_TEST").is_some();
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let urls: Vec<_> = args
        .iter()
        .skip(1)
        .filter_map(|arg| profile::startup_url(arg))
        .collect();
    let directory = profile::directory();
    let instance =
        match Instance::acquire(&directory, &urls, args.iter().any(|arg| arg == "--updated")) {
            Ok(Some(instance)) => instance,
            Ok(None) => return Ok(()),
            Err(error) => {
                desktop::message("Veluno", &error, false);
                return Ok(());
            }
        };
    let external = proxy.clone();
    if let Err(error) = instance.listen(move |urls| {
        let _ = external.send_event(UserEvent::External(urls));
    }) {
        desktop::message("Veluno", &error, false);
        return Ok(());
    }
    updater::cleanup_old_version();
    let icon = Icon::from_rgba(include_bytes!("../assets/icon_64.rgba").to_vec(), 64, 64).ok();
    let window = Rc::new(
        WindowBuilder::new()
            .with_title("Veluno")
            .with_visible(!headless)
            .with_focused(!headless)
            .with_theme(Some(tao::window::Theme::Dark))
            .with_decorations(false)
            .with_undecorated_shadow(true)
            .with_background_color((16, 16, 17, 255))
            .with_window_icon(icon.clone())
            .with_inner_size(if std::env::var_os("VELUNO_SMOKE_TEST").is_some() {
                LogicalSize::new(640., 480.)
            } else {
                LogicalSize::new(1380., 860.)
            })
            .with_min_inner_size(LogicalSize::new(640., 480.))
            .build(&event_loop)
            .map_err(|error| wry::Error::Io(std::io::Error::other(error)))?,
    );
    let mut popup_window_builder = WindowBuilder::new()
        .with_title("Sign in — Veluno")
        .with_visible(false)
        .with_focused(false)
        .with_theme(Some(tao::window::Theme::Dark))
        .with_decorations(true)
        .with_background_color((23, 23, 25, 255))
        .with_window_icon(icon)
        .with_inner_size(LogicalSize::new(500., 660.))
        .with_min_inner_size(LogicalSize::new(360., 420.));
    #[cfg(windows)]
    {
        popup_window_builder = popup_window_builder.with_owner_window(window.hwnd());
    }
    let popup_window = Rc::new(
        popup_window_builder
            .build(&event_loop)
            .map_err(|error| wry::Error::Io(std::io::Error::other(error)))?,
    );
    let popup_host = PopupHost {
        main: window.clone(),
        window: popup_window.clone(),
        view: Rc::new(RefCell::new(None)),
        url: Rc::new(RefCell::new(String::new())),
        auth_seen: Rc::new(Cell::new(false)),
        glance: Rc::new(Cell::new(false)),
        headless,
    };
    let mut state = BrowserState::restore();
    let refresh_after_update = args.iter().any(|arg| arg == "--updated") && urls.is_empty();
    let saved = proxy.clone();
    let session = SessionWriter::new(move |result| {
        let _ = saved.send_event(UserEvent::SessionSaved(result));
    });
    let mut session_dirty = true;
    let maintenance = proxy.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            history::flush();
            if maintenance.send_event(UserEvent::Maintenance).is_err() {
                break;
            }
        }
    });
    state.sidebar_collapsed = !headless;
    state.sidebar_peek = false;
    state.is_default_browser = desktop::is_default_browser();
    for url in urls {
        state.add_tab(Some(url), false);
    }
    let mut context = WebContext::new(Some(directory.join("WebView2")));
    let mut chrome_context = WebContext::new(Some(directory.join("Chrome")));
    let (sidebar, palette, mut permission_prompt, topbar, popup_bar) = build_chrome_views(
        &window,
        &popup_window,
        &state,
        &mut chrome_context,
        &proxy,
        headless,
    )?;
    let mut last_loading = false;
    let mut cursor_pos = (100., 100.);
    let mut popup_loading = false;
    let mut side_progress = if state.sidebar_collapsed { 0. } else { 1. };
    let mut top_progress = 0.;
    let mut animating = false;
    let animating_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let frame_thread = {
        let flag = animating_flag.clone();
        let frames = proxy.clone();
        std::thread::spawn(move || {
            loop {
                while !flag.load(std::sync::atomic::Ordering::Relaxed) {
                    std::thread::park();
                }
                desktop::vsync();
                if frames.send_event(UserEvent::Frame).is_err() {
                    break;
                }
            }
        })
    };
    let mut last_frame = std::time::Instant::now();
    let mut rounded: HashMap<usize, (i32, i32, i32)> = HashMap::new();
    let mut views = HashMap::new();
    let initial_id = state.active_tab_id;
    views.insert(
        initial_id,
        content(
            &window,
            &mut context,
            &mut state,
            initial_id,
            &proxy,
            &popup_host,
            refresh_after_update,
        )?,
    );
    let mut palette_open = false;
    let mut palette_shown = true;
    let mut palette_needs_open = false;
    let mut visible_id = None;
    let mut placed: HashMap<usize, Rect> = HashMap::new();
    let mut panel: Option<(WebView, String)> = None;
    let mut panel_placed: Option<Rect> = None;
    let mut panel_shown = false;
    let mut shown: Vec<usize> = Vec::new();
    let mut last_sidebar_state = String::new();
    let mut sleep_pending = HashSet::new();
    let mut recovery = recovery::Recovery::default();
    let mut hidden_at: HashMap<usize, (std::time::Instant, bool)> = HashMap::new();
    let mut focus_address = false;
    let mut downloads: HashMap<usize, usize> = HashMap::new();
    let mut download_only = HashSet::new();
    let mut last_download_host = None;
    let mut check_busy = false;

    // The tab whose page is in HTML fullscreen, and whether the app made the window fullscreen for it.
    let mut content_fullscreen: Option<(usize, bool)> = None;
    // Whether the window was maximized before the app fullscreened it.
    let mut maximize_after_fullscreen = false;
    let mut smoke = crate::smoke::Smoke::new();
    if smoke.is_some() {
        let proxy = proxy.clone();
        std::thread::spawn(move || {
            for _ in 0..300 {
                std::thread::sleep(std::time::Duration::from_millis(100));
                if proxy.send_event(UserEvent::SmokeTick).is_err() {
                    break;
                }
            }
        });
    }
    let background = proxy.clone();
    if !headless {
        if extensions::enabled_at_startup() {
            start_ad_blocker_install(&proxy);
            extensions::update_in_background();
            crate::adblock::update_in_background();
        }
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(3));
            if settings::get().auto_update {
                let _ =
                    background.send_event(UserEvent::UpdateChecked(updater::check_for_updates()));
            }
            loop {
                std::thread::sleep(std::time::Duration::from_secs(6 * 60 * 60));
                if settings::get().auto_update
                    && let Ok(Some(update)) = updater::check_for_updates()
                    && background
                        .send_event(UserEvent::UpdateChecked(Ok(Some(update))))
                        .is_err()
                {
                    break;
                }
            }
        });
    }

    event_loop.run(move |event, _, flow| {
        let _keep_instance = &instance;
        let _keep_chrome_context = &chrome_context;
        *flow = ControlFlow::Wait;
        let tick = matches!(event, Event::UserEvent(UserEvent::Frame));
        match event {
            Event::WindowEvent { window_id, event: WindowEvent::Focused(true), .. } if window_id == window.id() => {
                // Returning to the browser dismisses a Glance preview, like clicking outside it.
                if popup_host.glance.get() && popup_host.view.borrow().is_some() {
                    let _ = proxy.send_event(UserEvent::Command(Command::ClosePopup));
                }
                state.is_default_browser = desktop::is_default_browser();
                for tab in state.tabs.iter().filter(|tab| tab.url == "veluno://settings") {
                    if let Some(view) = views.get(&tab.id) {
                        let _ = view.evaluate_script(&format!("window.setDefaultBrowser?.({})", state.is_default_browser));
                    }
                }
            }
            Event::WindowEvent { window_id, event: WindowEvent::CloseRequested, .. } => {
                if window_id == popup_host.window.id() {
                    *popup_host.view.borrow_mut() = None;
                    popup_host.window.set_visible(false);
                    if !headless { window.set_focus(); }
                    return;
                }
                if may_exit(&state, &session) { *flow = ControlFlow::Exit; }
            }
            Event::UserEvent(UserEvent::Command(Command::WindowClose)) => {
                if may_exit(&state, &session) { *flow = ControlFlow::Exit; }
            }
            Event::NewEvents(StartCause::Init) => {},
            Event::WindowEvent { window_id, event: WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }, .. } => {
                if window_id == popup_host.window.id() {
                    let (bar_rect, content_rect) = popup_bounds(&popup_host.window);
                    let _ = popup_bar.set_bounds(bar_rect);
                    if let Some(view) = popup_host.view.borrow().as_ref() {
                        let _ = view.set_bounds(content_rect);
                    }
                    return;
                }
            },
            Event::WindowEvent { window_id, event: WindowEvent::CursorMoved { position, .. }, .. } if window_id == window.id() => {
                let logical = position.to_logical::<f64>(window.scale_factor());
                cursor_pos = (logical.x, logical.y);
                let size = window.inner_size().to_logical::<f64>(window.scale_factor());
                let icon = if window.is_maximized() || window.fullscreen().is_some() {
                    CursorIcon::Default
                } else {
                    border_resize_direction(cursor_pos.0, cursor_pos.1, size.width, size.height)
                        .map_or(CursorIcon::Default, |(_, cursor)| cursor)
                };
                window.set_cursor_icon(icon);
                return;
            }
            Event::WindowEvent {
                window_id,
                event: WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. },
                ..
            } if window_id == window.id() => {
                if !window.is_maximized() && window.fullscreen().is_none() {
                    let size = window.inner_size().to_logical::<f64>(window.scale_factor());
                    if let Some((dir, _)) = border_resize_direction(cursor_pos.0, cursor_pos.1, size.width, size.height) {
                        let _ = window.drag_resize_window(dir);
                    }
                }
                return;
            }
            Event::UserEvent(event) => {
                if headless && matches!(&event, UserEvent::Title(..) | UserEvent::PageLoad(..) | UserEvent::Source(..)) {
                    eprintln!("{event:?}");
                }
                let persistent = matches!(&event,
                    UserEvent::External(_) | UserEvent::Title(..) | UserEvent::PageLoad(..)
                    | UserEvent::Source(..) | UserEvent::IncompleteLoad(..) | UserEvent::Failed(..)
                    | UserEvent::Favicon(..) | UserEvent::DownloadStarted(..)
                    | UserEvent::Command(Command::Navigate { .. } | Command::NewTab { url: Some(_) }
                        | Command::SubmitNewTab { .. } | Command::SwitchTab { .. } | Command::CloseTab { .. }
                        | Command::ToggleSleep { .. } | Command::SleepInactiveTabs | Command::ClearUnpinnedTabs
                        | Command::ToggleSidebar | Command::SwitchWorkspace { .. } | Command::SwitchProfile { .. } | Command::CreateProfile { .. } | Command::RestoreClosedTab
                        | Command::TogglePin { .. } | Command::CycleTab { .. } | Command::FocusAddress
                        | Command::OpenSettings { .. } | Command::OpenHistory | Command::ImportBookmarks { .. } | Command::DuplicateTab { .. }
                        | Command::MoveTabToWorkspace { .. } | Command::ExpandPopup { .. }
                        | Command::ReorderTab { .. } | Command::RenameWorkspace { .. } | Command::SetWorkspaceEmoji { .. } | Command::DeleteWorkspace { .. } | Command::ToggleMute { .. }
                        | Command::DismissUpdate { .. }
                        | Command::ToggleBookmark { .. } | Command::NewPrivateTab
                        | Command::SplitTab { .. } | Command::Unsplit
                        | Command::OpenInSidePanel { .. } | Command::ToggleSidePanel));
                match event {
                    UserEvent::Audio(id, generation, playing, muted) => {
                        if let Some(tab) = state.tabs.iter_mut().find(|tab| tab.id == id && tab.generation == generation) {
                            tab.is_playing_audio = playing; tab.is_muted = muted;
                        }
                    },
                    UserEvent::SiteDataCleared(id, generation, origin, result) => {
                        state.update_status = Some(match result {
                            Ok(()) => {
                                if let Some(tab) = state.tabs.iter().find(|tab| tab.id == id && tab.generation == generation)
                                    && profile::web_origin(tab.pending_url.as_ref().unwrap_or(&tab.url)).as_deref() == Some(&origin)
                                    && let Some(view) = views.get(&id) { let _ = view.reload(); }
                                format!("Site data cleared for {origin}.")
                            },
                            Err(error) => format!("Could not clear site data: {error}"),
                        });
                    },
                    UserEvent::ResourceLoadFailed(id, generation, document_url, resource_url) => {
                        if let Some(tab) = state.tabs.iter().find(|tab| tab.id == id && tab.generation == generation)
                            && recovery.script_failed(tab, &document_url, &resource_url)
                            && id == state.active_tab_id {
                            state.update_status = Some("Some page scripts failed to load. Reload to refresh this site's cached files.".into());
                        }
                    },
                    UserEvent::SiteFilesCleared(page, result) => {
                        let active = page.id == state.active_tab_id;
                        if !recovery.finish(&page, state.tabs.iter().find(|tab| tab.id == page.id)) { return; }
                        match result {
                            Ok(()) => {
                                if let Some(view) = views.get(&page.id) {
                                    if let Err(error) = desktop::reload_fresh(view, page, proxy.clone()) {
                                        if active { state.update_status = Some(format!("Could not reload fresh site files: {error}")); }
                                    } else if active {
                                        state.update_status = Some("Site files refreshed. Your sign-in and saved site data were kept.".into());
                                    }
                                }
                            },
                            Err(error) => if active { state.update_status = Some(format!("Could not refresh site files: {error}")); },
                        }
                    },
                    UserEvent::FreshReloadFailed(page, error) => {
                        if page.id == state.active_tab_id
                            && recovery.is_current(&page, state.tabs.iter().find(|tab| tab.id == page.id))
                        {
                            state.update_status = Some(format!("Could not reload fresh site files: {error}"));
                        }
                    },
                    UserEvent::IncompleteLoad(id, generation, url) => {
                        let active = id == state.active_tab_id;
                        let retry = if let Some(tab) = state.tabs.iter_mut().find(|tab| {
                            tab.id == id
                                && tab.generation == generation
                                && tab.pending_url.as_ref().is_none_or(|pending| pending == &url)
                                && !profile::is_internal(&tab.url)
                        }) {
                            if let Some(normalized) = profile::normalize_url(&url) {
                                tab.url = normalized;
                            }
                            tab.pending_url = None;
                            tab.is_loading = false;
                            match recovery.incomplete_load(tab) {
                                Some(page) => Some(page),
                                None => {
                                    tab.error = Some("This page did not finish loading. Check your connection, then reload to try again.".into());
                                    if active {
                                        state.update_status = Some("This page did not finish loading. Reload to refresh this site's cached files.".into());
                                    }
                                    None
                                }
                            }
                        } else {
                            None
                        };
                        if let Some(page) = retry
                            && let Some(view) = views.get(&id)
                        {
                            if active {
                                state.update_status = Some("Refreshing this site's cached files. Your sign-in and saved site data are kept.".into());
                            }
                            if let Err(error) = desktop::clear_site_files(view, page.clone(), proxy.clone()) {
                                recovery.finish(&page, state.tabs.iter().find(|tab| tab.id == id));
                                if active {
                                    state.update_status = Some(format!("Could not refresh site files: {error}"));
                                }
                            }
                        }
                    },
                    UserEvent::Maintenance => {
                        recovery.retain(&state);
                        let current_downloads = desktop::download_counts(&views);
                        let downloads_changed = current_downloads != downloads;
                        downloads = current_downloads;
                        if session_dirty {
                            match session.try_save(&state) {
                                Ok(queued) => { session_dirty = !queued; },
                                Err(error) => { let _ = proxy.send_event(UserEvent::Status(format!("Could not save your session: {error}"))); },
                            }
                        }
                        for id in take_tabs_to_trim(&mut hidden_at, state.active_tab_id) {
                            if let Some(view) = views.get(&id) {
                                desktop::trim_memory(view);
                            }
                        }
                        let live: HashSet<_> = views.keys().copied().collect();
                        let idle: HashSet<_> = hidden_at.iter().filter(|(_, (at, _))| at.elapsed() >= IDLE_SLEEP).map(|(id, _)| *id).collect();
                        for tab in state.sleep_candidates(&live, &idle, &downloads, 12) {
                            if !sleep_pending.insert((tab.id, tab.generation)) { continue; }
                            let (id, generation, last_active, url) = (tab.id, tab.generation, tab.last_active, tab.url.clone());
                            let checked = proxy.clone();
                            if let Some(view) = views.get(&id) {
                                if !desktop::quiet(view) { sleep_pending.remove(&(id, generation)); continue; }
                                if view.evaluate_script_with_callback("window.__velunoCanSleep?.() === true", move |result| {
                                    let _ = checked.send_event(UserEvent::SleepChecked(id, generation, last_active, url.clone(), result == "true"));
                                }).is_err() { sleep_pending.remove(&(id, generation)); }
                            }
                        }
                        if !downloads_changed { return; }
                    },
                    UserEvent::SessionSaved(result) => {
                        if let Err(error) = result { session_dirty = true; state.update_status = Some(format!("Could not save your session: {error}")); }
                        else { return; }
                    },
                    UserEvent::SleepChecked(id, generation, last_active, url, safe) => {
                        sleep_pending.remove(&(id, generation));
                        if safe && (views.len() > 12 || hidden_at.get(&id).is_some_and(|(at, _)| at.elapsed() >= IDLE_SLEEP)) && id != state.active_tab_id
                            && downloads.get(&id).copied().unwrap_or(0) == 0
                            && views.get(&id).is_some_and(desktop::quiet)
                            && let Some(tab) = state.tabs.iter_mut().find(|tab| tab.id == id
                                && tab.generation == generation && tab.last_active == last_active && tab.url == url
                                && !tab.is_pinned && !tab.has_capture && !tab.is_loading && !tab.is_sleeping) {
                            tab.is_sleeping = true;
                            session_dirty = true;
                        } else { return; }
                    },
                    UserEvent::CaptureAllowed(id, generation) => {
                        if let Some(tab) = state.tabs.iter_mut().find(|tab| tab.id == id && tab.generation == generation) { tab.has_capture = true; }
                        return;
                    },
                    UserEvent::PermissionRequested => {},
                    UserEvent::PermissionReady => { permission_prompt.ready(); },
                    UserEvent::PermissionResponse(token, allow) => {
                        if permission_prompt.respond(token, allow, &state) && !headless && let Some(view) = views.get(&state.active_tab_id) { let _ = view.focus(); }
                    },
                    UserEvent::SmokeTick => {
                        if let Some(smoke) = &mut smoke && smoke.tick(&state, &views, &sidebar, &palette, &proxy, &session) { *flow = ControlFlow::Exit; }
                    }
                    UserEvent::SmokeResult(phase, success) => {
                        if let Some(smoke) = &mut smoke && smoke.result(phase, success, &proxy) { *flow = ControlFlow::Exit; }
                    }
                    UserEvent::Ready => { last_sidebar_state.clear(); },
                    UserEvent::TopbarReady => { last_loading = false; },
                    UserEvent::PopupReady => {
                        if let Ok(url_json) = serde_json::to_string(&*popup_host.url.borrow()) {
                            let _ = popup_bar.evaluate_script(&format!("window.setPopupState?.({url_json},{popup_loading})"));
                        }
                    },
                    UserEvent::PopupTitle(title) => {
                        if !title.is_empty() {
                            popup_host.window.set_title(&format!("{title} — Veluno"));
                        }
                    },
                    UserEvent::PopupState(url, loading) => {
                        popup_loading = loading;
                        if !url.is_empty() && url != "about:blank" {
                            // A sign-in flow that lands on a normal page instead of closing belongs in the main window.
                            if popup_host.glance.get() {
                                // Glance previews stay put until the user opens them as a tab.
                            } else if profile::is_auth_url(&url) {
                                popup_host.auth_seen.set(true);
                            } else if !loading && popup_host.auth_seen.get() && popup_host.view.borrow().is_some() {
                                let _ = proxy.send_event(UserEvent::Command(Command::ExpandPopup { url: url.clone() }));
                            }
                            *popup_host.url.borrow_mut() = url;
                        }
                        if let Ok(url_json) = serde_json::to_string(&*popup_host.url.borrow()) {
                            let _ = popup_bar.evaluate_script(&format!("window.setPopupState?.({url_json},{loading})"));
                        }
                    },
                    UserEvent::PopupClosed => {
                        *popup_host.view.borrow_mut() = None;
                        popup_host.window.set_visible(false);
                        if !headless { window.set_focus(); }
                    },
                    UserEvent::Frame => {},
                    UserEvent::Focused(id, generation) => {
                        // Clicking into the other split pane makes it the active tab.
                        if id == state.active_tab_id || state.split_partner() != Some(id)
                            || !state.tabs.iter().any(|tab| tab.id == id && tab.generation == generation) { return; }
                        state.switch_tab(id);
                        session_dirty = true;
                    },
                    UserEvent::PaletteReady => { palette_needs_open = palette_open; },
                    UserEvent::External(urls) => {
                        if !headless { window.set_minimized(false); window.set_focus(); }
                        for url in urls { if profile::allowed_url(&url) { state.add_tab(Some(url), false); } }
                        palette_open = false;
                    }
                    UserEvent::Command(command) => {
                        match command {
                            Command::Navigate { url } => if let Some(url) = profile::normalize_url(&url) {
                                state.update_active_tab(url.clone(), None);
                                if let Some(tab) = state.tabs.iter_mut().find(|tab| tab.id == state.active_tab_id) { tab.error = None; tab.pending_url = Some(url.clone()); tab.is_loading = true; }
                                if let Some(view) = views.get(&state.active_tab_id) { let _ = load(view, &state, &url); if !headless { let _ = view.focus(); } }
                            },
                            Command::Back | Command::Forward => if let Some(view) = views.get(&state.active_tab_id) { desktop::navigate(view, matches!(command, Command::Back)); },
                            Command::Reload | Command::Retry => if let Some(tab) = state.active_tab() {
                                if recovery.needs_reload(tab) && !tab.is_loading {
                                    let _ = proxy.send_event(UserEvent::Command(Command::ReloadFresh));
                                    return;
                                }
                                let url = tab.url.clone(); let id = tab.id;
                                recovery.navigation_completed(id, tab.generation);
                                let crashed = tab.error.is_some() && views.get(&id).and_then(|view| view.url().ok()).is_some_and(|url| url == "about:blank" || url.starts_with("data:"));
                                if crashed { views.remove(&id); }
                                if let Some(tab) = state.tabs.iter_mut().find(|tab| tab.id == id) { tab.error = None; tab.pending_url = Some(url.clone()); tab.is_loading = true; }
                                if let Some(view) = views.get(&id) { let _ = if profile::is_internal(&url) { load(view, &state, &url) } else { view.reload() }; }
                            },
                            Command::ReloadFresh => if let Some(tab) = state.active_tab() {
                                let crashed = tab.error.is_some()
                                    && views.get(&tab.id).is_none_or(|view| {
                                        view.url()
                                            .ok()
                                            .is_some_and(|url| url == "about:blank" || url.starts_with("data:"))
                                    });
                                if profile::web_origin(&tab.url).is_none() || crashed {
                                    let _ = proxy.send_event(UserEvent::Command(Command::Reload));
                                } else if let Some(view) = views.get(&tab.id) {
                                    if let Some(tab) = state.tabs.iter_mut().find(|tab| tab.id == state.active_tab_id) {
                                        tab.error = None;
                                    }
                                    if let Some(page) = state.active_tab().and_then(|tab| recovery.begin(tab)) {
                                        state.update_status = Some("Refreshing this site's cached files. Your sign-in and saved site data are kept.".into());
                                        if let Err(error) = desktop::clear_site_files(view, page.clone(), proxy.clone()) {
                                            recovery.finish(&page, state.active_tab());
                                            state.update_status = Some(format!("Could not refresh site files: {error}"));
                                        }
                                    }
                                }
                            },
                            Command::ResourceLoadFailed { .. } => {},
                            Command::Stop => { if let Some(view) = views.get(&state.active_tab_id) { let _ = view.evaluate_script("window.stop()"); }
                                if let Some(tab) = state.tabs.iter_mut().find(|tab| tab.id == state.active_tab_id) { tab.is_loading = false; } },
                            Command::NewTab { url: None } => { palette_open = true; palette_needs_open = true; },
                            Command::NewTab { url: Some(url) } | Command::SubmitNewTab { url } => if profile::allowed_url(&url) { state.add_tab(Some(url), false); palette_open = false; },
                            Command::DismissNewTab => { palette_open = false; if !headless && let Some(view) = views.get(&state.active_tab_id) { let _ = view.focus(); } },
                            Command::SwitchTab { id } => { state.switch_tab(id); palette_open = false; },
                            Command::CloseTab { id } => {
                                downloads = desktop::download_counts(&views);
                                state.active_downloads = downloads.values().sum();
                                state.download_items = desktop::download_items();
                                if downloads.get(&id).copied().unwrap_or(0) == 0 {
                                    if state.close_tab(id).is_none() && state.tabs.is_empty() {
                                        if may_exit(&state, &session) {
                                            *flow = ControlFlow::Exit;
                                            return;
                                        }
                                        state.add_tab(None, false);
                                    }
                                } else {
                                    state.update_status = Some("This tab has an active download. Finish or cancel it in Downloads before closing.".into());
                                }
                            },
                            Command::CloseActiveTab => { let _ = proxy.send_event(UserEvent::Command(Command::CloseTab { id: state.active_tab_id })); },
                            Command::ToggleSleep { id } => { if downloads.get(&id).copied().unwrap_or(0) == 0 { state.toggle_sleep(id); } },
                            Command::SleepInactiveTabs => { state.sleep_inactive_tabs(); for tab in &mut state.tabs { if downloads.get(&tab.id).copied().unwrap_or(0) > 0 { tab.is_sleeping = false; } } },
                            Command::ClearUnpinnedTabs => {
                                let active = state.active_tab_id;
                                let ids: Vec<_> = state.tabs.iter().filter(|tab| tab.id != active && tab.workspace_id == state.active_workspace_id && !tab.is_pinned && downloads.get(&tab.id).copied().unwrap_or(0) == 0).map(|tab| tab.id).collect();
                                for id in ids { state.close_tab(id); }
                            },
                            Command::ToggleSidebar => { state.toggle_sidebar(); },
                            Command::Peek { open } => { state.sidebar_peek = open && state.sidebar_collapsed; },
                            Command::TopbarPeek { open } => { state.topbar_peek = open; },
                            Command::WindowMinimize => { window.set_minimized(true); },
                            Command::WindowMaximize => { window.set_maximized(!window.is_maximized()); },
                            Command::WindowClose => {},
                            Command::DragWindow => { let _ = window.drag_window(); },
                            Command::ResizeWindow { direction } => {
                                if !window.is_maximized()
                                    && window.fullscreen().is_none()
                                    && let Some(dir) = parse_resize_direction(&direction)
                                {
                                    let _ = window.drag_resize_window(dir);
                                }
                            },
                            Command::ToggleFullscreen => {
                                if content_fullscreen.is_some() {
                                    leave_content_fullscreen(&window, &views, &mut content_fullscreen, &mut maximize_after_fullscreen);
                                } else if window.fullscreen().is_some() {
                                    restore_window(&window, &mut maximize_after_fullscreen);
                                } else {
                                    fullscreen_window(&window, &mut maximize_after_fullscreen);
                                }
                            },
                            Command::ExpandPopup { url } => {
                                let target = if profile::allowed_url(&url) {
                                    url
                                } else {
                                    popup_host.url.borrow().clone()
                                };
                                *popup_host.view.borrow_mut() = None;
                                popup_host.window.set_visible(false);
                                if profile::allowed_url(&target) {
                                    state.add_tab(Some(target), false);
                                }
                                if !headless {
                                    window.set_minimized(false);
                                    window.set_focus();
                                }
                            },
                            Command::SplitTab { id } => { if state.split_with(id) { palette_open = false; } },
                            Command::Unsplit => { state.split = None; },
                            Command::OpenInSidePanel { id } => { state.open_in_side_panel(id); },
                            Command::ToggleSidePanel => { state.side_panel_open = !state.side_panel_open && state.side_panel_url.is_some(); },
                            Command::ClosePopup => {
                                *popup_host.view.borrow_mut() = None;
                                popup_host.window.set_visible(false);
                                if !headless {
                                    window.set_focus();
                                    if let Some(view) = views.get(&state.active_tab_id) { let _ = view.focus(); }
                                }
                            },
                            Command::Glance { url } => if profile::allowed_url(&url) && !profile::is_internal(&url) {
                                let private = state.is_private(state.active_tab_id);
                                if let Err(error) = open_glance(&popup_host, &mut context, &url, private, &proxy) {
                                    popup_host.window.set_visible(false);
                                    state.update_status = Some(format!("Could not open Glance: {error}"));
                                }
                            },
                            Command::SwitchProfile { index } => {
                                if let Some(profile) = state.workspaces.get(index.saturating_sub(1)) { let id = profile.id; state.switch_workspace(id); }
                            },
                            Command::CreateProfile { name } => {
                                if let Err(error) = state.create_profile(&name) { state.update_status = Some(error); }
                            },
                            Command::SwitchWorkspace { id } => { state.switch_workspace(id); },
                            Command::RestoreClosedTab => { state.restore_closed_tab(); },
                            Command::NewPrivateTab => { state.open_private_tab(); palette_open = false; },
                            Command::ToggleBookmark { url } => state.toggle_bookmark(url),
                            Command::TogglePin { id } => state.toggle_pin(id),
                            Command::DuplicateTab { id } => { state.duplicate_tab(id); },
                            Command::MoveTabToWorkspace { id, workspace_id } => {
                                state.move_tab_to_workspace(id, workspace_id);
                            },
                            Command::ReorderTab { id, target, after } => state.reorder_tab(id, target, after),
                            Command::RenameWorkspace { id, name } => {
                                if let Err(error) = state.rename_workspace(id, &name) { state.update_status = Some(error); }
                            },
                            Command::SetWorkspaceEmoji { id, emoji } => {
                                if let Err(error) = state.set_workspace_emoji(id, &emoji) { state.update_status = Some(error); }
                            },
                            Command::DeleteWorkspace { id, destination } => {
                                if let Err(error) = state.delete_workspace(id, destination) { state.update_status = Some(error); }
                            },
                            Command::ToggleMute { id } => {
                                if let Some(tab) = state.tabs.iter_mut().find(|tab| tab.id == id) {
                                    let muted = !tab.is_muted;
                                    match views.get(&id).map_or(Ok(()), |view| desktop::mute(view, muted)) {
                                        Ok(()) => tab.is_muted = muted,
                                        Err(error) => state.update_status = Some(format!("Could not change tab audio: {error}")),
                                    }
                                }
                            },
                            Command::ClearSiteData { id, origin } => {
                                if let Some(tab) = state.active_tab().filter(|tab| tab.id == id
                                    && profile::web_origin(tab.pending_url.as_ref().unwrap_or(&tab.url)).as_deref() == Some(&origin))
                                    && let Some(view) = views.get(&id)
                                    && let Err(error) = desktop::clear_site_data(view, id, tab.generation, &origin, proxy.clone()) {
                                        state.update_status = Some(format!("Could not clear site data: {error}"));
                                    }
                            },
                            Command::CycleTab { backwards } => { state.cycle_tab(backwards); },
                            Command::FocusAddress => { state.sidebar_collapsed = false; focus_address = true; },
                            Command::AddressSuggestions => {
                                if let Ok(json) = serde_json::to_string(&history::all()) {
                                    let _ = sidebar.evaluate_script(&format!("window.updateAddressHistory?.({json})"));
                                }
                            },
                            Command::CancelDownload { id } => { if let Err(error) = desktop::cancel_download(id) { state.update_status = Some(error); } },
                            Command::CloseDownloads => { state.downloads_open = false; },
                            Command::ToggleDownloads => {
                                if state.downloads_open {
                                    state.downloads_open = false;
                                } else {
                                    state.sidebar_collapsed = false;
                                    state.downloads_open = true;
                                    let id = downloads.iter().find(|(_, count)| **count > 0).map(|(id,_)| *id)
                                        .or(last_download_host.filter(|id| views.contains_key(id))).unwrap_or(state.active_tab_id);
                                    if let Some(view) = views.get(&id) && let Err(error) = desktop::downloads(view) { state.update_status = Some(error); }
                                }
                            },
                            Command::ClearDownloads => { desktop::clear_downloads(); },
                            Command::OpenDownload { id } => { if let Err(error) = desktop::open_download(id) { state.update_status = Some(error); } },
                            Command::RevealDownload { id } => { if let Err(error) = desktop::reveal_download(id) { state.update_status = Some(error); } },
                            Command::OpenDownloads => {
                                state.sidebar_collapsed = false;
                                state.downloads_open = true;
                                let id = downloads.iter().find(|(_, count)| **count > 0).map(|(id,_)| *id)
                                    .or(last_download_host.filter(|id| views.contains_key(id))).unwrap_or(state.active_tab_id);
                                if let Some(view) = views.get(&id) && let Err(error) = desktop::downloads(view) { state.update_status = Some(error); }
                            },
                            Command::OpenSettings { section } => {
                                let live = state.tabs.iter().find(|tab| tab.url == "veluno://settings").and_then(|tab| views.get(&tab.id));
                                if let Some(section) = section.filter(|section| ui::SETTINGS_SECTIONS.contains(&section.as_str())) {
                                    match live {
                                        Some(view) => { let _ = view.evaluate_script(&format!("window.showSection?.({section:?})")); },
                                        None => ui::open_settings_at(section),
                                    }
                                }
                                state.open_internal("veluno://settings"); palette_open = false;
                            },
                            Command::OpenHistory => { state.open_internal("veluno://history"); palette_open = false; },
                            Command::DeleteHistory { url } => history::remove(&url),
                            Command::ClearHistory => history::clear(),
                            Command::SetSetting { key, value } => match settings::set(&key, &value) {
                                Ok(current) => {
                                    if key == "accent" {
                                        let script = ui::accent_script();
                                        for view in [&sidebar, &palette, &topbar, &popup_bar] { let _ = view.evaluate_script(&script); }
                                        for tab in state.tabs.iter().filter(|tab| profile::is_internal(&tab.url)) {
                                            if let Some(view) = views.get(&tab.id) { let _ = view.evaluate_script(&script); }
                                        }
                                    } else if key == "sidebar_right" {
                                        let _ = sidebar.evaluate_script(&format!("document.body.classList.toggle('right',{})", current.sidebar_right));
                                    } else if key == "search_engine" {
                                        let script = ui::search_script();
                                        let _ = sidebar.evaluate_script(&script);
                                        let _ = palette.evaluate_script(&script);
                                        for view in views.values() { let _ = view.evaluate_script(&script); }
                                    } else if matches!(key.as_str(), "tracking_prevention" | "save_passwords" | "autofill") {
                                        if let Some(error) = views.values().find_map(|view| desktop::apply_privacy(view, &current).err()) {
                                            state.update_status = Some(format!("This setting needs a newer WebView2 Runtime: {error}"));
                                        }
                                    } else if key == "ad_blocker" {
                                        if current.ad_blocker { start_ad_blocker_install(&proxy); }
                                        state.update_status = Some("Ad blocker changes apply after you restart Veluno.".into());
                                    }
                                },
                                Err(error) => { state.update_status = Some(error); },
                            },
                            Command::InstallAdBlocker => { start_ad_blocker_install(&proxy); },
                            Command::ClearBrowsingData { range, kinds } => if let Some((range, label)) = clear_range(&range) {
                                if kinds.history {
                                    history::clear_recent(range);
                                    desktop::clear_downloads();
                                }
                                let profile_data = kinds.site_data || kinds.cache || kinds.passwords;
                                // Private tabs use a separate in-memory profile, so clear through a regular one.
                                let regular = views.iter().find(|(id, _)| !state.is_private(**id)).map(|(_, view)| view);
                                let cleared = proxy.clone();
                                let report = move |result| { let _ = cleared.send_event(UserEvent::BrowsingDataCleared(result, label)); };
                                match regular {
                                    Some(view) => if let Err(error) = desktop::clear_browsing_data(view, kinds, range, report) {
                                        let _ = proxy.send_event(UserEvent::BrowsingDataCleared(Err(error), label));
                                    },
                                    None if profile_data => report(Err("Open a regular tab, then try again.".into())),
                                    None => report(Ok(())),
                                }
                            },
                            Command::ImportBookmarks { browser } => {
                                let name = crate::import::name(&browser).unwrap_or("that browser");
                                state.update_status = Some(match crate::import::read(&browser) {
                                    Ok(found) => {
                                        let total = found.len();
                                        match state.import_bookmarks(found) {
                                            0 if total == 0 => format!("{name} has no web bookmarks to import."),
                                            0 => format!("All {total} bookmarks from {name} are already in Veluno."),
                                            1 => format!("Imported 1 bookmark from {name}."),
                                            added => format!("Imported {added} bookmarks from {name}."),
                                        }
                                    },
                                    Err(error) => error,
                                });
                            },
                            Command::ToggleReader => if let Some(tab) = state.active_tab() {
                                if profile::web_origin(&tab.url).is_none() || tab.error.is_some() {
                                    state.update_status = Some("Reader view works on web articles.".into());
                                } else if let Some(view) = views.get(&tab.id) {
                                    let (id, toggled) = (tab.id, proxy.clone());
                                    let _ = view.evaluate_script_with_callback(include_str!("reader.js"), move |result| {
                                        let _ = toggled.send_event(UserEvent::ReaderToggled(id, result));
                                    });
                                }
                            },
                            Command::Screenshot => if let Some(tab) = state.active_tab() && let Some(view) = views.get(&tab.id) {
                                let saved = proxy.clone();
                                if let Err(error) = desktop::screenshot(view, &tab.title, move |result| { let _ = saved.send_event(UserEvent::ScreenshotSaved(result)); }) {
                                    state.update_status = Some(format!("Could not save a screenshot: {error}"));
                                }
                            },
                            Command::OpenExtensionsFolder => {
                                let _ = std::fs::create_dir_all(extensions::directory());
                                if let Err(error) = std::process::Command::new(updater::system_binary("explorer.exe")).arg(extensions::directory()).spawn() { state.update_status = Some(format!("Could not open the extensions folder: {error}")); }
                            },
                            Command::Zoom { delta } => {
                                let id = state.active_tab_id;
                                if let Some(view) = views.get(&id) {
                                    let current = i32::try_from(state.active_tab().map_or(100, |tab| tab.zoom)).unwrap_or(100);
                                    let next = u32::try_from(if delta == 0 { 100 } else { current.saturating_add(delta).clamp(30, 300) }).unwrap_or(100);
                                    if view.zoom(f64::from(next) / 100.).is_ok()
                                        && let Some(tab) = state.tabs.iter_mut().find(|tab| tab.id == id)
                                    {
                                        tab.zoom = next;
                                    }
                                }
                            },
                            Command::Print => if let Some(view) = views.get(&state.active_tab_id) { let _ = view.print(); },
                            Command::Exit => if may_exit(&state, &session) { *flow = ControlFlow::Exit; },
                            Command::DefaultBrowser => { if let Err(error) = std::process::Command::new(updater::system_binary("explorer.exe")).arg("ms-settings:defaultapps").spawn() { state.update_status = Some(format!("Could not open Windows default apps: {error}")); } },
                            Command::DismissUpdate { version } => { if version.len() <= 100 { state.dismissed_update_version = Some(version); } },
                            Command::CheckUpdates => if !check_busy {
                                check_busy = true; state.dismissed_update_version = None; state.update_status = Some("Checking for updates…".into());
                                let proxy = proxy.clone(); std::thread::spawn(move || { let _ = proxy.send_event(UserEvent::UpdateChecked(updater::check_for_updates())); });
                            },
                            Command::ApplyUpdate => if !state.update_busy && state.active_downloads == 0 && let Some(update) = &state.update_available {
                                let url = update.download_url.clone().unwrap_or_else(|| format!("https://github.com/{}/releases/download/v{}/veluno.exe", updater::GITHUB_REPO, update.version));
                                if session.flush(&state).is_ok() {
                                    state.update_busy = true; state.update_status = Some("Downloading update. Veluno will restart when it is ready.".into());
                                    let proxy = proxy.clone(); std::thread::spawn(move || { if let Err(error) = updater::download_and_apply_update(&url) { let _ = proxy.send_event(UserEvent::UpdateFailed(format!("Update failed: {error}. You can try again."))); } });
                                } else { state.update_status = Some("Save your session successfully before updating. Check available disk space.".into()); }
                            },
                        }
                    }
                    event @ (UserEvent::Title(..) | UserEvent::PageLoad(..) | UserEvent::Source(..)
                        | UserEvent::History(..) | UserEvent::Failed(..) | UserEvent::Favicon(..)
                        | UserEvent::Crashed(..)) => {
                            if let UserEvent::PageLoad(id, generation, loading, _) = &event {
                                if *loading {
                                    recovery.navigation_started(*id, *generation);
                                } else {
                                    recovery.navigation_completed(*id, *generation);
                                }
                            }
                            tab_event(&mut state, &mut views, &mut permission_prompt, event);
                        },
                    UserEvent::DownloadsChanged => {},
                    UserEvent::DownloadStarted(id, generation, blank, document) => {
                        state.downloads_open = true;
                        state.sidebar_collapsed = false;
                        if state.tabs.iter().any(|tab| tab.id == id && tab.generation == generation) {
                            last_download_host = Some(id);
                            if blank && state.discard_download_tab(id) {
                                download_only.insert(id);
                                if let Some(view) = views.get(&id) {
                                    desktop::show(view, false);
                                    desktop::trim_memory(view);
                                }
                            } else if let Some(tab) = state.tabs.iter_mut().find(|tab| tab.id == id) {
                                tab.is_loading = false; tab.pending_url = None; tab.error = None;
                                if let Some(url) = document.or_else(|| tab.document_url.clone()) { tab.url = url; }
                            }
                        }
                    },
                    UserEvent::DownloadFinished(id, _generation, outcome) => {
                        if state.tabs.iter().any(|tab| tab.id == id) || download_only.contains(&id) {
                            if views.contains_key(&id) { last_download_host = Some(id); }
                            state.update_status = Some(match outcome {
                                desktop::DownloadOutcome::Completed => {
                                    "Download complete. Open Downloads to view the file.".into()
                                }
                                desktop::DownloadOutcome::Cancelled => "Download cancelled.".into(),
                                desktop::DownloadOutcome::Interrupted => {
                                    "Download interrupted. Open Downloads to retry or choose another location.".into()
                                }
                            });
                        }
                    },
                    UserEvent::UpdateChecked(result) => { check_busy = false; match result {
                        Ok(Some(update)) => { state.update_available = Some(update); state.update_status = None; },
                        Ok(None) => { state.update_status = Some(format!("Veluno is up to date (v{}).", updater::CURRENT_VERSION)); },
                        Err(error) => { state.update_status = Some(error); }
                    } },
                    UserEvent::Status(status) => { state.update_status = Some(status); },
                    UserEvent::AdBlockerInstalled(result) => match result {
                        Ok(()) => {
                            if extensions::enabled_at_startup()
                                && let Some((_, view)) = views.iter().find(|(id, _)| !state.is_private(**id))
                            {
                                let _ = desktop::load_extensions(view, &extensions::directory());
                            }
                            state.update_status = Some("uBlock Origin is installed. Restart Veluno to start blocking ads.".into());
                        },
                        Err(error) => { state.update_status = Some(format!("Ad blocker install failed: {error} Try again from Settings.")); },
                    },
                    UserEvent::UpdateFailed(status) => { state.update_busy = false; state.update_status = Some(status); },
                    UserEvent::BrowsingDataCleared(result, label) => {
                        let ok = result.is_ok();
                        state.update_status = Some(match result {
                            Ok(()) => format!("Browsing data from {label} was cleared."),
                            Err(error) => format!("Could not clear browsing data: {error}"),
                        });
                        for tab in state.tabs.iter().filter(|tab| tab.url == "veluno://settings") {
                            if let Some(view) = views.get(&tab.id) { let _ = view.evaluate_script(&format!("window.dataCleared?.({ok})")); }
                        }
                    },
                    UserEvent::ScreenshotSaved(result) => {
                        state.update_status = Some(match result {
                            Ok(path) => format!("Screenshot saved to Pictures\\Veluno as {}.", path.file_name().map(|name| name.to_string_lossy()).unwrap_or_default()),
                            Err(error) => format!("Could not save a screenshot: {error}"),
                        });
                    },
                    UserEvent::ContentFullscreen(id, generation, enabled) => {
                        let visible = id == state.active_tab_id || state.split_partner() == Some(id);
                        let current = visible && state.tabs.iter().any(|tab| tab.id == id && tab.generation == generation);
                        if enabled && current && content_fullscreen.is_none() {
                            let owns_window = window.fullscreen().is_none();
                            if owns_window { fullscreen_window(&window, &mut maximize_after_fullscreen); }
                            content_fullscreen = Some((id, owns_window));
                        } else if !enabled && content_fullscreen.is_some_and(|(active, _)| active == id) {
                            leave_content_fullscreen(&window, &views, &mut content_fullscreen, &mut maximize_after_fullscreen);
                        }
                    },
                    UserEvent::ReaderToggled(id, result) => {
                        if id == state.active_tab_id && result.trim_matches('"') == "unavailable" {
                            state.update_status = Some("Reader view isn't available on this page. It works best on articles and blog posts.".into());
                        } else { return; }
                    },
                }
                session_dirty |= persistent;
            }
            Event::LoopDestroyed => { history::flush(); let _ = session.flush(&state); return; },
            _ => return,
        }
        downloads = desktop::download_counts(&views);
        state.active_downloads = downloads.values().sum();
        state.download_items = desktop::download_items();
        let awake: HashSet<_> = state.tabs.iter().filter(|tab| !tab.is_sleeping).map(|tab| tab.id).collect();
        let download_hosts = desktop::download_hosts();
        // Active and resumable transfers retain their owner; keep one completed owner for Downloads.
        download_only.retain(|id| download_hosts.contains(id) || last_download_host == Some(*id));
        views.retain(|id,_| awake.contains(id) || download_only.contains(id));
        for tab in &mut state.tabs { if !views.contains_key(&tab.id) { tab.is_playing_audio = false; } }
        downloads.retain(|id, count| *count > 0 || awake.contains(id));
        sleep_pending.retain(|(id, _)| views.contains_key(id));
        let active_id = state.active_tab_id;
        if content_fullscreen.is_some_and(|(id, _)| {
            !views.contains_key(&id) || (id != active_id && state.split_partner() != Some(id))
        }) {
            leave_content_fullscreen(&window, &views, &mut content_fullscreen, &mut maximize_after_fullscreen);
        }
        if visible_id != Some(active_id)
            && let Some(tab) = state.tabs.iter_mut().find(|tab| tab.id == active_id)
            && tab.error.take().is_some()
        {
            views.remove(&active_id);
        }
        let mut created = false;
        if let std::collections::hash_map::Entry::Vacant(entry) = views.entry(active_id) {
            match content(&window, &mut context, &mut state, active_id, &proxy, &popup_host, false) {
                Ok(view) => {
                    if state.active_tab().is_some_and(|tab| tab.error.is_some()) { let _ = view.load_html(include_str!("error.html")); }
                    entry.insert(view);
                    created = true;

                }
                Err(error) => { state.update_status = Some(format!("Could not open this tab: {error}")); }
            }
        }
        let pair = state.split_pair().filter(|_| content_fullscreen.is_none());
        if let Some(partner) = pair.and_then(|_| state.split_partner())
            && let std::collections::hash_map::Entry::Vacant(entry) = views.entry(partner)
        {
            match content(&window, &mut context, &mut state, partner, &proxy, &popup_host, false) {
                Ok(view) => { entry.insert(view); created = true; },
                Err(error) => { state.split = None; state.update_status = Some(format!("Could not open split view: {error}")); },
            }
        }
        let pair = state.split_pair().filter(|_| content_fullscreen.is_none());
        hidden_at.retain(|id, _| views.contains_key(id));
        state.zoom = state.active_tab().map_or(100, |tab| tab.zoom);
        let now = std::time::Instant::now();
        let dt = if animating { now.duration_since(last_frame).as_secs_f64().min(0.05) } else { 0. };
        last_frame = now;
        let side_target = if !state.sidebar_collapsed || state.sidebar_peek { 1. } else { 0. };
        let top_target = if state.topbar_peek { 1. } else { 0. };
        side_progress = step(side_progress, side_target, dt, !state.sidebar_collapsed);
        top_progress = step(top_progress, top_target, dt, false);
        let was_animating = animating;
        animating = side_progress != side_target || top_progress != top_target;
        if animating != was_animating {
            animating_flag.store(animating, std::sync::atomic::Ordering::Relaxed);
            if animating { frame_thread.thread().unpark(); }
        }
        let (mut sidebar_bounds, mut content_bounds, overlay_bounds, mut topbar_bounds) = bounds(&window, &state, ease(side_progress), ease(top_progress));
        if content_fullscreen.is_some() {
            // The page fills the window while it is fullscreen, so the browser chrome steps aside.
            let size = window.inner_size().to_logical::<f64>(window.scale_factor());
            content_bounds = Rect { position: Position::Logical(LogicalPosition::new(0., 0.)), size: Size::Logical(size) };
            sidebar_bounds = Rect { position: Position::Logical(LogicalPosition::new(0., 0.)), size: Size::Logical(LogicalSize::new(0., 0.)) };
            topbar_bounds = sidebar_bounds;
        }
        let _ = sidebar.set_bounds(sidebar_bounds);
        if tick {
            let _ = topbar.set_bounds(topbar_bounds);
            return;
        }
        let content_bounds = match state.side_panel_url.clone().filter(|_| state.side_panel_open && content_fullscreen.is_none()) {
            Some(url) => {
                let (rest, panel_rect) = side_panel_bounds(content_bounds, window.scale_factor(), settings::sidebar_right());
                if panel.as_ref().is_none_or(|(_, loaded)| loaded != &url) {
                    panel = None;
                    panel_placed = None;
                    match side_panel(&window, &mut context, &url, panel_rect, &proxy) {
                        Ok(view) => panel = Some((view, url)),
                        Err(error) => { state.side_panel_open = false; state.update_status = Some(format!("Could not open the side panel: {error}")); },
                    }
                }
                if let Some((view, _)) = &panel {
                    if panel_placed != Some(panel_rect) {
                        let _ = view.set_bounds(panel_rect);
                        let physical = panel_rect.size.to_physical::<i32>(window.scale_factor());
                        let radius = if window.is_maximized() || window.fullscreen().is_some() { 0 } else { (ui::CONTENT_RADIUS * window.scale_factor()) as i32 };
                        desktop::round(view, physical.width, physical.height, radius);
                        panel_placed = Some(panel_rect);
                    }
                    if !panel_shown { desktop::show(view, true); panel_shown = true; }
                }
                if state.side_panel_open { rest } else { content_bounds }
            }
            None => {
                if panel_shown && let Some((view, _)) = &panel { desktop::show(view, false); }
                panel_shown = false;
                content_bounds
            }
        };
        let panes = split_bounds(content_bounds, window.scale_factor());
        let pane = |id: usize| match pair { Some([left, _]) if id == left => panes.0, Some([_, right]) if id == right => panes.1, _ => content_bounds };
        // A page in HTML fullscreen is the only pane on screen, even when it is the split partner.
        let wanted: Vec<usize> = content_fullscreen.map_or_else(
            || pair.map_or_else(|| vec![active_id], |pair| pair.to_vec()),
            |(id, _)| vec![id],
        );
        if wanted != shown || created {
            for id in shown.iter().filter(|id| !wanted.contains(id)) {
                if let Some(view) = views.get(id) { desktop::show(view, false); hidden_at.entry(*id).or_insert((now, false)); }
            }
            for id in &wanted {
                if let Some(view) = views.get(id) { desktop::show(view, true); hidden_at.remove(id); }
            }
            shown.clone_from(&wanted);
        }
        placed.retain(|id, _| views.contains_key(id));
        for id in &wanted {
            if let Some(view) = views.get(id) {
                let target = pane(*id);
                if placed.get(id) != Some(&target) { let _ = view.set_bounds(target); placed.insert(*id, target); }
            }
        }
        let radius = if window.is_maximized() || window.fullscreen().is_some() { 0 } else { (ui::CONTENT_RADIUS * window.scale_factor()) as i32 };
        rounded.retain(|id, _| views.contains_key(id));
        for id in &wanted {
            if let Some(view) = views.get(id) {
                let physical = pane(*id).size.to_physical::<i32>(window.scale_factor());
                let shape = (physical.width, physical.height, radius);
                if rounded.get(id) != Some(&shape) { desktop::round(view, shape.0, shape.1, radius); rounded.insert(*id, shape); }
            }
        }
        let _ = topbar.set_bounds(topbar_bounds);
        if !palette_open {
            if state.sidebar_collapsed {
                desktop::raise(&sidebar);
                desktop::below(&topbar, &sidebar);
            } else {
                desktop::raise(&topbar);
            }
        }
        let _ = palette.set_bounds(overlay_bounds);
        if palette_shown != palette_open {
            desktop::show(&palette, palette_open);
            palette_shown = palette_open;
        }
        if palette_open {
            desktop::raise(&palette);
            desktop::raise(&sidebar);
        }
        if visible_id != Some(active_id) && !palette_open {
            if !headless && let Some(view) = views.get(&active_id) { let _ = view.focus(); }
            visible_id = Some(active_id);
        }
        state.is_loading = state.active_tab().is_some_and(|tab| tab.is_loading);
        if state.is_loading != last_loading { last_loading = state.is_loading; let _ = topbar.evaluate_script(&format!("window.setLoading?.({last_loading})")); }
        if let Some(tab) = state.active_tab() {
            window.set_title(&format!("{} — Veluno", tab.title));
        }
        if let Ok(json) = serde_json::to_string(&state) {
            if json != last_sidebar_state && sidebar.evaluate_script(&format!("window.updateState?.({json})")).is_ok() { last_sidebar_state.clone_from(&json); }
            if focus_address { if !headless { let _ = sidebar.focus(); } let _ = sidebar.evaluate_script("window.focusAddress()"); focus_address = false; }
            if palette_open && palette_needs_open {
                let history_json = serde_json::to_string(&history::all()).unwrap_or_else(|_| "[]".into());
                let _ = palette.evaluate_script(&format!("window.openNewTab?.({json},{history_json})"));
                if !headless { let _ = palette.focus(); }
                palette_needs_open = false;
            }
        }
        permission_prompt.sync(&state, overlay_bounds, palette_open, headless);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(key: u32, ctrl: bool, shift: bool, alt: bool) -> String {
        format!("{:?}", shortcut(key, ctrl, shift, alt))
    }

    #[test]
    fn shortcuts_map_virtual_keys_and_leave_page_keys_alone() {
        assert_eq!(
            action(0x54, true, false, false),
            "Some(NewTab { url: None })"
        );
        assert_eq!(action(0x54, true, true, false), "Some(RestoreClosedTab)");
        assert_eq!(action(0x42, true, true, false), "Some(ToggleSidebar)");
        assert_eq!(action(0x52, true, true, false), "Some(ReloadFresh)");
        assert_eq!(action(0x74, true, false, false), "Some(ReloadFresh)");
        assert_eq!(
            action(0x09, true, true, false),
            "Some(CycleTab { backwards: true })"
        );
        assert_eq!(action(0xBB, true, true, false), "Some(Zoom { delta: 10 })");
        assert_eq!(
            action(0x32, false, false, true),
            "Some(SwitchProfile { index: 2 })"
        );
        assert_eq!(action(0x25, false, false, true), "Some(Back)");
        assert_eq!(action(0x7A, false, false, false), "Some(ToggleFullscreen)");
        assert_eq!(action(0x78, false, false, false), "Some(ToggleReader)");
        assert_eq!(
            action(0x2E, true, true, false),
            "Some(OpenSettings { section: Some(\"privacy\") })"
        );
        // Save, find, copy, plain typing, and AltGr (Ctrl+Alt) belong to the page.
        for (key, ctrl, shift, alt) in [
            (0x53, true, false, false),
            (0x46, true, false, false),
            (0x43, true, false, false),
            (0x54, false, false, false),
            (0x54, true, false, true),
            (0x2E, true, false, false),
            (0x78, true, false, false),
        ] {
            assert_eq!(action(key, ctrl, shift, alt), "None");
        }
    }

    #[test]
    fn clear_browsing_data_requests_parse_and_ranges_are_fixed() {
        let command: Command = serde_json::from_str(
            r#"{"action":"clear_browsing_data","range":"day","kinds":{"history":true,"cache":true}}"#,
        )
        .unwrap();
        let Command::ClearBrowsingData { range, kinds } = command else {
            panic!("wrong command");
        };
        assert!(kinds.history && kinds.cache && !kinds.site_data && !kinds.passwords);
        assert_eq!(
            clear_range(&range),
            Some((Some(86_400_000), "the last 24 hours"))
        );
        assert_eq!(clear_range("all"), Some((None, "all time")));
        assert_eq!(clear_range("forever"), None);
        let settings: Command = serde_json::from_str(r#"{"action":"open_settings"}"#).unwrap();
        assert!(matches!(settings, Command::OpenSettings { section: None }));
    }

    #[test]
    fn background_tabs_trim_once_per_hide_cycle() {
        let now = std::time::Instant::now();
        let old = now.checked_sub(MEMORY_TRIM_DELAY).unwrap();
        let mut hidden_at =
            HashMap::from([(1, (old, false)), (2, (old, false)), (3, (now, false))]);
        assert_eq!(take_tabs_to_trim(&mut hidden_at, 1), vec![2]);
        assert!(take_tabs_to_trim(&mut hidden_at, 1).is_empty());
        hidden_at.remove(&2);
        hidden_at.insert(2, (old, false));
        assert_eq!(take_tabs_to_trim(&mut hidden_at, 1), vec![2]);
    }
}
