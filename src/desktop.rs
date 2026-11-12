use crate::app::UserEvent;
use tao::event_loop::EventLoopProxy;
use wry::WebView;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadOutcome {
    Completed,
    Cancelled,
    Interrupted,
}

#[cfg(windows)]
type DownloadEntry = (
    usize,
    webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2DownloadOperation,
    Option<std::rc::Rc<std::cell::RefCell<DownloadInfo>>>,
);

#[cfg(windows)]
thread_local! {
    // Keep operations alive and reconcile state even if a completion event is missed.
    static DOWNLOADS: std::cell::RefCell<Vec<DownloadEntry>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DownloadInfo {
    pub id: usize,
    pub path: String,
    pub received: i64,
    pub total: i64,
    pub status: String,
}

#[cfg(windows)]
thread_local! {
    static CANCELLABLE_DOWNLOADS: std::cell::RefCell<Vec<(usize, webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2DownloadOperation)>> = const { std::cell::RefCell::new(Vec::new()) };
    static DOWNLOAD_ITEMS: std::cell::RefCell<Vec<std::rc::Rc<std::cell::RefCell<DownloadInfo>>>> = const { std::cell::RefCell::new(Vec::new()) };
    static SYNCED_EXTENSIONS: std::cell::RefCell<std::collections::HashSet<std::path::PathBuf>> = std::cell::RefCell::new(std::collections::HashSet::new());
}

pub fn download_items() -> Vec<DownloadInfo> {
    DOWNLOAD_ITEMS.with_borrow(|items| items.iter().map(|item| item.borrow().clone()).collect())
}

pub fn clear_downloads() {
    DOWNLOAD_ITEMS.with_borrow_mut(|items| {
        items.retain(|item| item.borrow().status == "downloading");
    });
}

#[cfg(windows)]
unsafe fn refresh_download(
    operation: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2DownloadOperation,
    info: &std::cell::RefCell<DownloadInfo>,
) -> windows::core::Result<()> {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON,
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_CANCELED, COREWEBVIEW2_DOWNLOAD_STATE,
        COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED, COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS,
        COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED,
    };
    let mut info = info.borrow_mut();
    let mut path = windows::core::PWSTR::null();
    // SAFETY: `DOWNLOADS` is thread-local, so this COM object is used only on the UI thread that created it.
    // Each out-parameter points to a live local, and `info` is borrowed for the call.
    unsafe {
        operation.ResultFilePath(&mut path)?;
        info.path = webview2_com::take_pwstr(path);
        operation.BytesReceived(&mut info.received)?;
        operation.TotalBytesToReceive(&mut info.total)?;
        let mut state = COREWEBVIEW2_DOWNLOAD_STATE::default();
        operation.State(&mut state)?;
        info.status = if state == COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED
            || (state == COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS
                && info.total > 0
                && info.received >= info.total)
        {
            "complete"
        } else if state == COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED {
            let mut reason = COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON::default();
            operation.InterruptReason(&mut reason)?;
            if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_CANCELED {
                "cancelled"
            } else {
                "interrupted"
            }
        } else {
            "downloading"
        }
        .into();
    }
    Ok(())
}

pub fn cancel_download(id: usize) -> Result<(), String> {
    CANCELLABLE_DOWNLOADS.with_borrow(|items| {
        let (_, operation) = items
            .iter()
            .find(|(item, _)| *item == id)
            .ok_or("This download is no longer active.")?;
        // SAFETY: `CANCELLABLE_DOWNLOADS` is thread-local, so `operation` is alive and used only on the UI thread.
        // `Cancel` takes no pointers, and the borrow of `operation` lasts for the call.
        unsafe {
            operation
                .Cancel()
                .map_err(|error| format!("Could not cancel the download: {error}"))
        }
    })
}

pub fn open_download(id: usize) -> Result<(), String> {
    let item = download_items()
        .into_iter()
        .find(|item| {
            item.id == id
                && (item.status == "complete"
                    || (item.status == "downloading"
                        && item.total > 0
                        && item.received >= item.total))
        })
        .ok_or("This download is not complete yet.")?;
    if !std::path::Path::new(&item.path).exists() {
        return Err("The downloaded file was moved or deleted.".into());
    }
    std::process::Command::new(crate::updater::system_binary("explorer.exe"))
        .arg(&item.path)
        .spawn()
        .map_err(|error| format!("Could not open the downloaded file: {error}"))?;
    Ok(())
}

pub fn reveal_download(id: usize) -> Result<(), String> {
    let item = download_items()
        .into_iter()
        .find(|item| {
            item.id == id
                && (item.status == "complete"
                    || (item.status == "downloading"
                        && item.total > 0
                        && item.received >= item.total))
        })
        .ok_or("This download is not complete yet.")?;
    std::process::Command::new(crate::updater::system_binary("explorer.exe"))
        .args(["/select,", &item.path])
        .spawn()
        .map_err(|error| format!("Could not show the downloaded file: {error}"))?;
    Ok(())
}

pub fn download_counts(
    views: &std::collections::HashMap<usize, WebView>,
) -> std::collections::HashMap<usize, usize> {
    let mut counts = std::collections::HashMap::new();
    #[cfg(windows)]
    DOWNLOADS.with_borrow_mut(|downloads| {
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED, COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS,
            COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED,
        };
        // SAFETY: Every entry in `DOWNLOADS` is a COM object owned by this UI thread (thread-local storage).
        // The calls write only to locals and to `info`, which is borrowed for the call.
        downloads.retain(|(id, operation, info)| unsafe {
            if !views.contains_key(id) {
                return false;
            }
            if let Some(info) = info {
                let _ = refresh_download(operation, info);
            }
            let mut state = COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS;
            let _ = operation.State(&mut state);
            let mut received = 0;
            let mut total = 0;
            let _ = operation.BytesReceived(&mut received);
            let _ = operation.TotalBytesToReceive(&mut total);
            if state == COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED
                || (state == COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS
                    && total > 0
                    && received >= total)
            {
                if let Some(info) = info {
                    info.borrow_mut().status = "complete".into();
                }
                return false;
            }
            if state == COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS {
                *counts.entry(*id).or_default() += 1;
            } else if state == COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED {
                let mut resumable = windows::core::BOOL::default();
                if operation.CanResume(&mut resumable).is_ok() && !resumable.as_bool() {
                    return false;
                }
            }
            true
        });
    });
    // SAFETY: `CANCELLABLE_DOWNLOADS` is thread-local, so these COM objects stay on the UI thread that created them.
    // Every out-parameter is a local that outlives the call.
    CANCELLABLE_DOWNLOADS.with_borrow_mut(|items| items.retain(|(_, operation)| unsafe {
        let mut state = webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_DOWNLOAD_STATE::default();
        let mut received = 0;
        let mut total = 0;
        let _ = operation.BytesReceived(&mut received);
        let _ = operation.TotalBytesToReceive(&mut total);
        operation.State(&mut state).is_ok()
            && state == webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS
            && !(total > 0 && received >= total)
    }));
    counts
}

pub fn download_hosts() -> std::collections::HashSet<usize> {
    DOWNLOADS.with_borrow(|downloads| downloads.iter().map(|(id, _, _)| *id).collect())
}

pub fn is_default_browser() -> bool {
    #[cfg(windows)]
    {
        use windows::{
            Win32::UI::Shell::{ASSOCF_IS_PROTOCOL, ASSOCSTR_PROGID, AssocQueryStringW},
            core::{PCWSTR, PWSTR, w},
        };
        [w!("http"), w!("https")]
            .into_iter()
            // SAFETY: `value` is a 256-element buffer, and `length` holds its size in characters, as `AssocQueryStringW` requires.
            // `protocol` is a static string.
            .all(|protocol| unsafe {
                let mut value = [0u16; 256];
                let mut length = u32::try_from(value.len()).unwrap_or(u32::MAX);
                AssocQueryStringW(
                    ASSOCF_IS_PROTOCOL,
                    ASSOCSTR_PROGID,
                    protocol,
                    PCWSTR::null(),
                    Some(PWSTR(value.as_mut_ptr())),
                    &mut length,
                )
                .is_ok()
                    && ["VelunoHTML", "FelarisHTML"].iter().any(|progid| {
                        String::from_utf16_lossy(
                            &value[..value.iter().position(|c| *c == 0).unwrap_or(value.len())],
                        )
                        .eq_ignore_ascii_case(progid)
                    })
            })
    }
    #[cfg(not(windows))]
    false
}

pub fn raise(view: &WebView) {
    stack(view, None);
}

pub fn below(view: &WebView, above: &WebView) {
    stack(view, Some(above));
}

fn stack(view: &WebView, above: Option<&WebView>) {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
        };
        use wry::WebViewExtWindows;
        // SAFETY: `view` and `above` are borrowed for the call, so their windows are alive.
        // Window calls are made on the UI thread that created them.
        unsafe {
            let _ = SetWindowPos(
                view.hwnd(),
                Some(above.map_or(HWND_TOP, |view| view.hwnd())),
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
            );
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (view, above);
    }
}

pub fn vsync() {
    #[cfg(windows)]
    // SAFETY: `DwmFlush` takes no arguments and only waits for the next compositor frame.
    unsafe {
        if windows::Win32::Graphics::Dwm::DwmFlush().is_ok() {
            return;
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(16));
}

pub fn round(view: &WebView, width: i32, height: i32, radius: i32) {
    #[cfg(windows)]
    {
        use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, DeleteObject, SetWindowRgn};
        use wry::WebViewExtWindows;
        // SAFETY: `view.hwnd()` is alive for the call. `SetWindowRgn` takes ownership of `region` on success,
        // so it is deleted here only when that call fails.
        unsafe {
            let region = (radius > 0)
                .then(|| CreateRoundRectRgn(0, 0, width + 1, height + 1, radius * 2, radius * 2));
            if SetWindowRgn(view.hwnd(), region, true) == 0
                && let Some(region) = region
            {
                let _ = DeleteObject(region.into());
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (view, width, height, radius);
    }
}

/// Shows or hides the tab's WebView. Visible tabs restore normal memory usage.
pub fn show(view: &WebView, visible: bool) {
    let _ = view.set_visible(visible);
    #[cfg(windows)]
    if visible {
        // SAFETY: `view.webview()` is borrowed for the call. `cast` returns an error rather than an interface
        // the runtime does not implement.
        unsafe {
            use webview2_com::Microsoft::Web::WebView2::Win32::{
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL, ICoreWebView2_19,
            };
            use windows::core::Interface;
            use wry::WebViewExtWindows;
            if let Ok(view) = view.webview().cast::<ICoreWebView2_19>() {
                let _ =
                    view.SetMemoryUsageTargetLevel(COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL);
            }
        }
    } else if crate::settings::performance_mode() {
        trim_memory(view);
    }
}

/// Trims a background tab's renderer memory after an idle duration.
pub fn trim_memory(view: &WebView) {
    #[cfg(windows)]
    // SAFETY: `view.webview()` is borrowed for the call. `cast` returns an error rather than an interface
    // the runtime does not implement.
    unsafe {
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW, ICoreWebView2_19,
        };
        use windows::core::Interface;
        use wry::WebViewExtWindows;
        if let Ok(view) = view.webview().cast::<ICoreWebView2_19>() {
            let _ = view.SetMemoryUsageTargetLevel(COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW);
        }
    }
    #[cfg(not(windows))]
    let _ = view;
}

pub fn quiet(view: &WebView) -> bool {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_8;
    use windows::core::{BOOL, Interface};
    use wry::WebViewExtWindows;
    // SAFETY: `view.webview()` is borrowed for the call. `cast` fails cleanly on older runtimes.
    // `playing` is a local out-parameter.
    unsafe {
        let mut playing = BOOL::default();
        view.webview()
            .cast::<ICoreWebView2_8>()
            .and_then(|view| view.IsDocumentPlayingAudio(&mut playing))
            .is_ok()
            && !playing.as_bool()
    }
}

pub fn mute(view: &WebView, muted: bool) -> Result<(), String> {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_8;
    use windows::core::Interface;
    use wry::WebViewExtWindows;
    // SAFETY: `view.webview()` is borrowed for the call. `cast` fails cleanly on older runtimes, and the
    // error is returned to the caller.
    unsafe {
        view.webview()
            .cast::<ICoreWebView2_8>()
            .and_then(|view| view.SetIsMuted(muted))
            .map_err(|error| error.to_string())
    }
}

/// Applies the Settings privacy choices to a content WebView and its profile.
pub fn apply_privacy(view: &WebView, settings: &crate::settings::Settings) -> Result<(), String> {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_BALANCED,
        COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_BASIC,
        COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_STRICT, ICoreWebView2_13, ICoreWebView2Profile3,
        ICoreWebView2Settings4,
    };
    use windows::core::Interface;
    use wry::WebViewExtWindows;
    let level = match settings.tracking_prevention.as_str() {
        "basic" => COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_BASIC,
        "strict" => COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_STRICT,
        _ => COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_BALANCED,
    };
    let webview = view.webview();
    // SAFETY: `webview` is borrowed for the call. Each `cast` fails cleanly on older runtimes, and the
    // closure's `?` returns that error.
    (|| unsafe {
        let options = webview.Settings()?.cast::<ICoreWebView2Settings4>()?;
        options.SetIsPasswordAutosaveEnabled(settings.save_passwords)?;
        options.SetIsGeneralAutofillEnabled(settings.autofill)?;
        webview
            .cast::<ICoreWebView2_13>()?
            .Profile()?
            .cast::<ICoreWebView2Profile3>()?
            .SetPreferredTrackingPreventionLevel(level)
    })()
    .map_err(|error| error.to_string())
}

/// What Settings > Clear browsing data removes.
#[derive(Debug, Clone, Copy, Default, serde::Deserialize)]
pub struct DataKinds {
    #[serde(default)]
    pub history: bool,
    #[serde(default)]
    pub site_data: bool,
    #[serde(default)]
    pub cache: bool,
    #[serde(default)]
    pub passwords: bool,
}

/// Clears the WebView2 profile data behind `view` from the last `range_ms` milliseconds.
pub fn clear_browsing_data(
    view: &WebView,
    kinds: DataKinds,
    range_ms: Option<u64>,
    completed: impl FnOnce(Result<(), String>) + 'static,
) -> Result<(), String> {
    use webview2_com::{
        ClearBrowsingDataCompletedHandler,
        Microsoft::Web::WebView2::Win32::{
            COREWEBVIEW2_BROWSING_DATA_KINDS, COREWEBVIEW2_BROWSING_DATA_KINDS_ALL_DOM_STORAGE,
            COREWEBVIEW2_BROWSING_DATA_KINDS_BROWSING_HISTORY,
            COREWEBVIEW2_BROWSING_DATA_KINDS_COOKIES, COREWEBVIEW2_BROWSING_DATA_KINDS_DISK_CACHE,
            COREWEBVIEW2_BROWSING_DATA_KINDS_DOWNLOAD_HISTORY,
            COREWEBVIEW2_BROWSING_DATA_KINDS_GENERAL_AUTOFILL,
            COREWEBVIEW2_BROWSING_DATA_KINDS_PASSWORD_AUTOSAVE, ICoreWebView2_13,
            ICoreWebView2Profile2,
        },
    };
    use windows::core::Interface;
    use wry::WebViewExtWindows;
    let mut flags = 0;
    if kinds.history {
        flags |= COREWEBVIEW2_BROWSING_DATA_KINDS_BROWSING_HISTORY.0
            | COREWEBVIEW2_BROWSING_DATA_KINDS_DOWNLOAD_HISTORY.0;
    }
    if kinds.site_data {
        flags |= COREWEBVIEW2_BROWSING_DATA_KINDS_COOKIES.0
            | COREWEBVIEW2_BROWSING_DATA_KINDS_ALL_DOM_STORAGE.0;
    }
    if kinds.cache {
        flags |= COREWEBVIEW2_BROWSING_DATA_KINDS_DISK_CACHE.0;
    }
    if kinds.passwords {
        flags |= COREWEBVIEW2_BROWSING_DATA_KINDS_PASSWORD_AUTOSAVE.0
            | COREWEBVIEW2_BROWSING_DATA_KINDS_GENERAL_AUTOFILL.0;
    }
    if flags == 0 {
        completed(Ok(()));
        return Ok(());
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0., |elapsed| elapsed.as_secs_f64());
    let start = range_ms.map_or(0., |range| (now - range as f64 / 1000.).max(0.));
    let webview = view.webview();
    // SAFETY: `webview` is borrowed for the call. Each `cast` fails cleanly on older runtimes.
    // WebView2 invokes the completion handler, which owns `completed`, on this UI thread.
    (|| unsafe {
        webview
            .cast::<ICoreWebView2_13>()?
            .Profile()?
            .cast::<ICoreWebView2Profile2>()?
            .ClearBrowsingDataInTimeRange(
                COREWEBVIEW2_BROWSING_DATA_KINDS(flags),
                start,
                // A little slack so data written while the request is in flight is included.
                now + 60.,
                &ClearBrowsingDataCompletedHandler::create(Box::new(move |result| {
                    completed(result.map_err(|error| error.to_string()));
                    Ok(())
                })),
            )
    })()
    .map_err(|error| error.to_string())
}

/// Saves the visible part of the page as a PNG in `Pictures\Veluno`.
pub fn screenshot(
    view: &WebView,
    title: &str,
    completed: impl FnOnce(Result<std::path::PathBuf, String>) + 'static,
) -> Result<(), String> {
    use webview2_com::{
        CapturePreviewCompletedHandler,
        Microsoft::Web::WebView2::Win32::COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
    };
    use windows::{
        Win32::System::Com::{STGM_CREATE, STGM_SHARE_DENY_WRITE, STGM_WRITE},
        Win32::UI::Shell::SHCreateStreamOnFileEx,
        core::HSTRING,
    };
    use wry::WebViewExtWindows;
    let folder = pictures_folder().join("Veluno");
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    let path = unique_path(&folder, screenshot_name(title));
    // SAFETY: The `HSTRING` lives until the call returns, and the returned stream is owned here.
    let stream = unsafe {
        SHCreateStreamOnFileEx(
            &HSTRING::from(path.as_os_str()),
            (STGM_CREATE | STGM_WRITE | STGM_SHARE_DENY_WRITE).0,
            0,
            true,
            None,
        )
    }
    .map_err(|error| error.to_string())?;
    let keep = stream.clone();
    let cleanup = path.clone();
    // SAFETY: `view` is borrowed for the call and `stream` is a live IStream. The completion handler
    // owns its own clone of the stream and the path, and WebView2 invokes it on this UI thread.
    let captured = unsafe {
        view.webview().CapturePreview(
            COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
            &stream,
            &CapturePreviewCompletedHandler::create(Box::new(move |result| {
                // Closing the stream flushes the file before it is reported.
                drop(keep);
                if result.is_err() {
                    let _ = std::fs::remove_file(&path);
                }
                completed(result.map(|_| path).map_err(|error| error.to_string()));
                Ok(())
            })),
        )
    };
    if let Err(error) = captured {
        // The file already exists, so a capture that fails at once would leave an empty PNG behind.
        drop(stream);
        let _ = std::fs::remove_file(&cleanup);
        return Err(error.to_string());
    }
    Ok(())
}

/// A free file name in `folder`, so two captures in the same second both survive.
fn unique_path(folder: &std::path::Path, name: String) -> std::path::PathBuf {
    let candidate = folder.join(&name);
    if !candidate.exists() {
        return candidate;
    }
    let stem = name.strip_suffix(".png").unwrap_or(&name);
    for number in 2..1000 {
        let next = folder.join(format!("{stem} ({number}).png"));
        if !next.exists() {
            return next;
        }
    }
    candidate
}

fn pictures_folder() -> std::path::PathBuf {
    use windows::Win32::{
        System::Com::CoTaskMemFree,
        UI::Shell::{FOLDERID_Pictures, KF_FLAG_DEFAULT, SHGetKnownFolderPath},
    };
    // SAFETY: `SHGetKnownFolderPath` returns a buffer we own. It is read before the single
    // `CoTaskMemFree`, and it is not used after that.
    unsafe {
        if let Ok(path) = SHGetKnownFolderPath(&FOLDERID_Pictures, KF_FLAG_DEFAULT, None) {
            let folder = path.to_string().ok().map(std::path::PathBuf::from);
            CoTaskMemFree(Some(path.0 as *const _));
            if let Some(folder) = folder {
                return folder;
            }
        }
    }
    std::env::var_os("USERPROFILE")
        .map(|home| std::path::PathBuf::from(home).join("Pictures"))
        .unwrap_or_else(crate::profile::directory)
}

/// A Windows-safe file name stamped with local time, such as `Example 2026-10-22 14.05.09.png`.
fn screenshot_name(title: &str) -> String {
    // SAFETY: `GetLocalTime` takes no arguments and returns a plain struct by value.
    let time = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    let stamp = [
        time.wYear,
        time.wMonth,
        time.wDay,
        time.wHour,
        time.wMinute,
        time.wSecond,
    ];
    file_name(title, stamp)
}

fn file_name(title: &str, [year, month, day, hour, minute, second]: [u16; 6]) -> String {
    let clean: String = title
        .chars()
        .map(|c| {
            if c.is_control() || r#"<>:"/\|?*"#.contains(c) {
                ' '
            } else {
                c
            }
        })
        .collect();
    let clean: String = clean
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(60)
        .collect();
    let stem = clean.trim().trim_end_matches('.');
    let stem = if stem.is_empty() { "Screenshot" } else { stem };
    format!("{stem} {year:04}-{month:02}-{day:02} {hour:02}.{minute:02}.{second:02}.png")
}

pub fn clear_site_data(
    view: &WebView,
    id: usize,
    generation: u64,
    origin: &str,
    proxy: EventLoopProxy<UserEvent>,
) -> Result<(), String> {
    let parameters = serde_json::json!({"origin": origin, "storageTypes": "cookies,local_storage,indexeddb,websql,file_systems,cache_storage,service_workers"}).to_string();
    let origin = origin.to_owned();
    devtools(
        view,
        "Storage.clearDataForOrigin",
        &parameters,
        move |result| {
            let _ = proxy.send_event(UserEvent::SiteDataCleared(
                id,
                generation,
                origin.clone(),
                result,
            ));
        },
    )
}

pub fn clear_site_files(
    view: &WebView,
    page: crate::recovery::Page,
    proxy: EventLoopProxy<UserEvent>,
) -> Result<(), String> {
    let origin = crate::profile::web_origin(&page.url).ok_or("This page has no web origin.")?;
    let parameters =
        serde_json::json!({"origin": origin, "storageTypes": "cache_storage,service_workers"})
            .to_string();
    devtools(
        view,
        "Storage.clearDataForOrigin",
        &parameters,
        move |result| {
            let _ = proxy.send_event(UserEvent::SiteFilesCleared(page.clone(), result));
        },
    )
}

pub fn reload_fresh(
    view: &WebView,
    page: crate::recovery::Page,
    proxy: EventLoopProxy<UserEvent>,
) -> Result<(), String> {
    devtools(
        view,
        "Page.reload",
        r#"{"ignoreCache":true}"#,
        move |result| {
            if let Err(error) = result {
                let _ = proxy.send_event(UserEvent::FreshReloadFailed(page.clone(), error));
            }
        },
    )
}

fn devtools(
    view: &WebView,
    method: &str,
    parameters: &str,
    completed: impl Fn(Result<(), String>) + 'static,
) -> Result<(), String> {
    use webview2_com::CallDevToolsProtocolMethodCompletedHandler;
    use windows::core::HSTRING;
    use wry::WebViewExtWindows;
    // SAFETY: `view.webview()` is borrowed for the call. The `HSTRING` arguments are temporaries that live
    // until the call returns.
    unsafe {
        view.webview()
            .CallDevToolsProtocolMethod(
                &HSTRING::from(method),
                &HSTRING::from(parameters),
                &CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |result, _| {
                    completed(result.map_err(|error| error.to_string()));
                    Ok(())
                })),
            )
            .map_err(|error| error.to_string())
    }
}

pub fn message(title: &str, text: &str, confirm: bool) -> bool {
    if std::env::var_os("VELUNO_SMOKE_TEST").is_some() {
        eprintln!("{title}: {text}");
        return false;
    }
    #[cfg(windows)]
    {
        use windows::{
            Win32::UI::WindowsAndMessaging::{
                IDYES, MB_DEFBUTTON2, MB_ICONINFORMATION, MB_ICONWARNING, MB_YESNO, MessageBoxW,
            },
            core::PCWSTR,
        };
        let text: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        let title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
        let flags = if confirm {
            MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2
        } else {
            MB_ICONINFORMATION
        };
        // SAFETY: `text` and `title` are NUL-terminated UTF-16 buffers that live until the call returns.
        // A `None` owner is allowed for a modal box on the current thread.
        unsafe { MessageBoxW(None, PCWSTR(text.as_ptr()), PCWSTR(title.as_ptr()), flags) == IDYES }
    }
    #[cfg(not(windows))]
    {
        eprintln!("{title}: {text}");
        !confirm
    }
}

#[cfg(windows)]
pub fn attach(
    view: &WebView,
    id: usize,
    generation: u64,
    proxy: EventLoopProxy<UserEvent>,
) -> wry::Result<()> {
    use webview2_com::{
        AcceleratorKeyPressedEventHandler, BytesReceivedChangedEventHandler,
        ContainsFullScreenElementChangedEventHandler, DownloadStartingEventHandler,
        FaviconChangedEventHandler, FocusChangedEventHandler, HistoryChangedEventHandler,
        IsDocumentPlayingAudioChangedEventHandler, IsMutedChangedEventHandler,
        Microsoft::Web::WebView2::Win32::{
            COREWEBVIEW2_DOWNLOAD_STATE, COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED,
            COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS, COREWEBVIEW2_KEY_EVENT_KIND,
            COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN, COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN,
            COREWEBVIEW2_PERMISSION_KIND, COREWEBVIEW2_PERMISSION_KIND_CAMERA,
            COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ, COREWEBVIEW2_PERMISSION_KIND_GEOLOCATION,
            COREWEBVIEW2_PERMISSION_KIND_MICROPHONE, COREWEBVIEW2_PERMISSION_KIND_NOTIFICATIONS,
            COREWEBVIEW2_PERMISSION_STATE_DENY, COREWEBVIEW2_PHYSICAL_KEY_STATUS,
            COREWEBVIEW2_WEB_ERROR_STATUS, COREWEBVIEW2_WEB_ERROR_STATUS_OPERATION_CANCELED,
            ICoreWebView2_4, ICoreWebView2_8, ICoreWebView2_15,
            ICoreWebView2NavigationCompletedEventArgs2,
        },
        NavigationCompletedEventHandler, NavigationStartingEventHandler,
        PermissionRequestedEventHandler, ProcessFailedEventHandler, SourceChangedEventHandler,
        StateChangedEventHandler, take_pwstr,
    };
    use windows::core::{BOOL, Interface, PWSTR};
    use wry::WebViewExtWindows;
    let webview = view.webview();
    let mut token = 0;
    let history = proxy.clone();
    // Pages still load if the runtime is too old to filter requests.
    let _ = block_ads(&webview);
    // SAFETY: `webview` is borrowed for the call. The handlers capture cloned proxies, not borrows, so they
    // stay valid after this returns. Tokens are written to locals.
    unsafe {
        if let Ok(audio) = webview.cast::<ICoreWebView2_8>() {
            let playing_proxy = proxy.clone();
            audio.add_IsDocumentPlayingAudioChanged(
                &IsDocumentPlayingAudioChangedEventHandler::create(Box::new(move |view, _| {
                    if let Some(view) = view
                        && let Ok(audio) = view.cast::<ICoreWebView2_8>()
                    {
                        let (mut playing, mut muted) = (BOOL::default(), BOOL::default());
                        audio.IsDocumentPlayingAudio(&mut playing)?;
                        audio.IsMuted(&mut muted)?;
                        let _ = playing_proxy.send_event(UserEvent::Audio(
                            id,
                            generation,
                            playing.as_bool(),
                            muted.as_bool(),
                        ));
                    }
                    Ok(())
                })),
                &mut token,
            )?;
            let muted_proxy = proxy.clone();
            audio.add_IsMutedChanged(
                &IsMutedChangedEventHandler::create(Box::new(move |view, _| {
                    if let Some(view) = view
                        && let Ok(audio) = view.cast::<ICoreWebView2_8>()
                    {
                        let (mut playing, mut muted) = (BOOL::default(), BOOL::default());
                        audio.IsDocumentPlayingAudio(&mut playing)?;
                        audio.IsMuted(&mut muted)?;
                        let _ = muted_proxy.send_event(UserEvent::Audio(
                            id,
                            generation,
                            playing.as_bool(),
                            muted.as_bool(),
                        ));
                    }
                    Ok(())
                })),
                &mut token,
            )?;
        }
        let focused = proxy.clone();
        view.controller().add_GotFocus(
            &FocusChangedEventHandler::create(Box::new(move |_, _| {
                let _ = focused.send_event(UserEvent::Focused(id, generation));
                Ok(())
            })),
            &mut token,
        )?;
        let keys = proxy.clone();
        view.controller().add_AcceleratorKeyPressed(
            &AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
                use windows::Win32::UI::Input::KeyboardAndMouse::{
                    GetKeyState, VIRTUAL_KEY, VK_CONTROL, VK_ESCAPE, VK_MENU, VK_SHIFT,
                };
                let Some(args) = args else { return Ok(()) };
                let mut kind = COREWEBVIEW2_KEY_EVENT_KIND::default();
                args.KeyEventKind(&mut kind)?;
                if kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN
                    && kind != COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN
                {
                    return Ok(());
                }
                let mut key = 0;
                args.VirtualKey(&mut key)?;
                let down = |key: VIRTUAL_KEY| GetKeyState(i32::from(key.0)) < 0;
                if let Some(command) =
                    crate::app::shortcut(key, down(VK_CONTROL), down(VK_SHIFT), down(VK_MENU))
                {
                    args.SetHandled(true)?;
                    let mut status = COREWEBVIEW2_PHYSICAL_KEY_STATUS::default();
                    args.PhysicalKeyStatus(&mut status)?;
                    if !status.WasKeyDown.as_bool() {
                        let _ = keys.send_event(UserEvent::Command(command));
                    }
                } else if key == u32::from(VK_ESCAPE.0) {
                    // Not handled, so the page still receives Escape.
                    let mut status = COREWEBVIEW2_PHYSICAL_KEY_STATUS::default();
                    args.PhysicalKeyStatus(&mut status)?;
                    if !status.WasKeyDown.as_bool() {
                        let _ = keys
                            .send_event(UserEvent::Command(crate::app::Command::CloseDownloads));
                    }
                }
                Ok(())
            })),
            &mut token,
        )?;
        webview.add_HistoryChanged(
            &HistoryChangedEventHandler::create(Box::new(move |view, _| {
                if let Some(view) = view {
                    let (mut back, mut forward) = (BOOL::default(), BOOL::default());
                    view.CanGoBack(&mut back)?;
                    view.CanGoForward(&mut forward)?;
                    let _ = history.send_event(UserEvent::History(
                        id,
                        generation,
                        back.as_bool(),
                        forward.as_bool(),
                    ));
                }
                Ok(())
            })),
            &mut token,
        )?;
        let source = proxy.clone();
        webview.add_SourceChanged(
            &SourceChangedEventHandler::create(Box::new(move |view, args| {
                if let (Some(view), Some(args)) = (view, args) {
                    let mut new_document = BOOL::default();
                    args.IsNewDocument(&mut new_document)?;
                    if !new_document.as_bool() {
                        let mut uri = PWSTR::null();
                        view.Source(&mut uri)?;
                        let _ =
                            source.send_event(UserEvent::Source(id, generation, take_pwstr(uri)));
                    }
                }
                Ok(())
            })),
            &mut token,
        )?;
        let navigations: std::rc::Rc<std::cell::RefCell<Option<(u64, String)>>> =
            std::rc::Rc::new(std::cell::RefCell::new(None));
        let starting = navigations.clone();
        let started = proxy.clone();
        webview.add_NavigationStarting(
            &NavigationStartingEventHandler::create(Box::new(move |_, args| {
                if let Some(args) = args {
                    let mut navigation_id = 0;
                    let mut uri = PWSTR::null();
                    args.NavigationId(&mut navigation_id)?;
                    args.Uri(&mut uri)?;
                    let uri = take_pwstr(uri);
                    *starting.borrow_mut() = Some((navigation_id, uri.clone()));
                    let _ = started.send_event(UserEvent::PageLoad(id, generation, true, uri));
                }
                Ok(())
            })),
            &mut token,
        )?;
        let failed = proxy.clone();
        webview.add_NavigationCompleted(&NavigationCompletedEventHandler::create(Box::new(move |view, args| {
            if let Some(args) = args {
                let mut navigation_id = 0;
                args.NavigationId(&mut navigation_id)?;
                let Some((_, mut uri)) = navigations
                    .borrow_mut()
                    .take_if(|(current_id, _)| *current_id == navigation_id)
                else {
                    return Ok(());
                };
                let mut success = BOOL::default();
                args.IsSuccess(&mut success)?;
                let source_uri = view.and_then(|view| {
                    let mut current = PWSTR::null();
                    view.Source(&mut current).ok()?;
                    crate::profile::normalize_url(&take_pwstr(current))
                });
                if success.as_bool() {
                    if let Some(current) = source_uri {
                        uri = current;
                    }
                    let _ = failed.send_event(UserEvent::PageLoad(id, generation, false, uri));
                } else {
                    let mut status = COREWEBVIEW2_WEB_ERROR_STATUS::default();
                    args.WebErrorStatus(&mut status)?;
                    if status != COREWEBVIEW2_WEB_ERROR_STATUS_OPERATION_CANCELED {
                        let mut http_status = 0;
                        if let Ok(args2) = args.cast::<ICoreWebView2NavigationCompletedEventArgs2>() {
                            let _ = args2.HttpStatusCode(&mut http_status);
                        }
                        if (200..300).contains(&http_status) {
                            if let Some(current) = source_uri {
                                uri = current;
                            }
                            let _ = failed.send_event(UserEvent::IncompleteLoad(id, generation, uri));
                        } else {
                            let _ = failed.send_event(UserEvent::Failed(id, generation, uri, "This page could not be reached. Check your connection or the address, then try again.".into()));
                        }
                    }
                }
            }
            Ok(())
        })), &mut token)?;
        let favicon = proxy.clone();
        webview.cast::<ICoreWebView2_15>()?.add_FaviconChanged(
            &FaviconChangedEventHandler::create(Box::new(move |view, _| {
                if let Some(view) = view
                    && let Ok(view) = view.cast::<ICoreWebView2_15>()
                {
                    let mut uri = PWSTR::null();
                    view.FaviconUri(&mut uri)?;
                    let _ = favicon.send_event(UserEvent::Favicon(id, generation, take_pwstr(uri)));
                }
                Ok(())
            })),
            &mut token,
        )?;
        let crashed = proxy.clone();
        webview.add_ProcessFailed(
            &ProcessFailedEventHandler::create(Box::new(move |_, _| {
                let _ = crashed.send_event(UserEvent::Crashed(id, generation));
                Ok(())
            })),
            &mut token,
        )?;
        let fullscreen = proxy.clone();
        webview.add_ContainsFullScreenElementChanged(
            &ContainsFullScreenElementChangedEventHandler::create(Box::new(move |view, _| {
                if let Some(view) = view {
                    let mut contains = BOOL::default();
                    view.ContainsFullScreenElement(&mut contains)?;
                    let _ = fullscreen.send_event(UserEvent::ContentFullscreen(
                        id,
                        generation,
                        contains.as_bool(),
                    ));
                }
                Ok(())
            })),
            &mut token,
        )?;
        let capture = proxy.clone();
        webview.add_PermissionRequested(
            &PermissionRequestedEventHandler::create(Box::new(move |_, args| {
                if let Some(args) = args {
                    let mut uri = PWSTR::null();
                    args.Uri(&mut uri)?;
                    let uri = take_pwstr(uri);
                    let origin = url::Url::parse(&uri)
                        .map(|url| url.origin().ascii_serialization())
                        .unwrap_or_else(|_| "This page".into());
                    let mut kind = COREWEBVIEW2_PERMISSION_KIND::default();
                    args.PermissionKind(&mut kind)?;
                    let permission = match kind {
                        COREWEBVIEW2_PERMISSION_KIND_CAMERA => "camera",
                        COREWEBVIEW2_PERMISSION_KIND_MICROPHONE => "microphone",
                        COREWEBVIEW2_PERMISSION_KIND_GEOLOCATION => "location",
                        COREWEBVIEW2_PERMISSION_KIND_NOTIFICATIONS => "notifications",
                        COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ => "clipboard",
                        _ => {
                            args.SetState(COREWEBVIEW2_PERMISSION_STATE_DENY)?;
                            return Ok(());
                        }
                    };
                    crate::permission::request(
                        &args,
                        id,
                        generation,
                        origin,
                        permission,
                        capture.clone(),
                    )?;
                }
                Ok(())
            })),
            &mut token,
        )?;
        let download_proxy = proxy.clone();
        webview.cast::<ICoreWebView2_4>()?.add_DownloadStarting(
            &DownloadStartingEventHandler::create(Box::new(move |view, args| {
                if let Some(args) = args {
                    let operation = args.DownloadOperation()?;
                    let info = std::rc::Rc::new(std::cell::RefCell::new(DownloadInfo {
                        id: 0,
                        path: String::new(),
                        received: 0,
                        total: 0,
                        status: "downloading".into(),
                    }));
                    refresh_download(&operation, &info)?;
                    DOWNLOAD_ITEMS.with_borrow_mut(|items| {
                        info.borrow_mut().id = items.last().map_or(1, |item| item.borrow().id + 1);
                        if items.len() >= 100
                            && let Some(index) = items
                                .iter()
                                .position(|item| item.borrow().status != "downloading")
                        {
                            items.remove(index);
                        }
                        items.push(info.clone());
                    });
                    CANCELLABLE_DOWNLOADS
                        .with_borrow_mut(|items| items.push((info.borrow().id, operation.clone())));
                    let finished_sent = std::rc::Rc::new(std::cell::Cell::new(false));
                    let progress = download_proxy.clone();
                    let progress_info = info.clone();
                    let progress_finished = finished_sent.clone();
                    operation.add_BytesReceivedChanged(
                        &BytesReceivedChangedEventHandler::create(Box::new(move |operation, _| {
                            let mut finished = false;
                            if let Some(operation) = operation {
                                refresh_download(&operation, &progress_info)?;
                                let item = progress_info.borrow();
                                if item.status == "complete" && !progress_finished.replace(true) {
                                    finished = true;
                                }
                            }
                            let _ = progress.send_event(UserEvent::DownloadsChanged);
                            if finished {
                                let _ = progress.send_event(UserEvent::DownloadFinished(
                                    id,
                                    generation,
                                    DownloadOutcome::Completed,
                                ));
                            }
                            Ok(())
                        })),
                        &mut 0,
                    )?;
                    let complete = download_proxy.clone();
                    let complete_info = info.clone();
                    let complete_finished = finished_sent.clone();
                    operation.add_StateChanged(
                        &StateChangedEventHandler::create(Box::new(move |operation, _| {
                            if let Some(operation) = operation {
                                refresh_download(&operation, &complete_info)?;
                                let mut state = COREWEBVIEW2_DOWNLOAD_STATE::default();
                                operation.State(&mut state)?;
                                let _ = complete.send_event(UserEvent::DownloadsChanged);
                                if (state != COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS
                                    || complete_info.borrow().status == "complete")
                                    && !complete_finished.replace(true)
                                {
                                    let outcome = if complete_info.borrow().status == "cancelled" {
                                        DownloadOutcome::Cancelled
                                    } else if state == COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED
                                        || complete_info.borrow().status == "complete"
                                    {
                                        DownloadOutcome::Completed
                                    } else {
                                        DownloadOutcome::Interrupted
                                    };
                                    let _ = complete.send_event(UserEvent::DownloadFinished(
                                        id, generation, outcome,
                                    ));
                                }
                            }
                            Ok(())
                        })),
                        &mut 0,
                    )?;
                    DOWNLOADS.with_borrow_mut(|downloads| {
                        downloads.push((id, operation, Some(info.clone())))
                    });
                    let mut source = PWSTR::null();
                    let document = view
                        .and_then(|view| view.Source(&mut source).ok().map(|_| take_pwstr(source)));
                    let blank = document
                        .as_deref()
                        .is_some_and(|url| url.is_empty() || url == "about:blank");
                    let document = document.and_then(|url| crate::profile::normalize_url(&url));
                    let _ = download_proxy
                        .send_event(UserEvent::DownloadStarted(id, generation, blank, document));
                    let _ = download_proxy.send_event(UserEvent::DownloadsChanged);
                }
                Ok(())
            })),
            &mut token,
        )?;
    }
    Ok(())
}

/// Receives page messages posted through `window.chrome.webview.postMessage`.
///
/// wry's own IPC handler would define a frozen `window.ipc` global, which page scripts such as
/// Google Keep's instrumentation read as their own `ipc` function and crash on.
#[cfg(windows)]
pub fn attach_ipc(view: &WebView, handler: impl Fn(&str, &str) + 'static) -> Result<(), String> {
    use webview2_com::{WebMessageReceivedEventHandler, take_pwstr};
    use windows::core::PWSTR;
    use wry::WebViewExtWindows;
    let webview = view.webview();
    let mut token = 0;
    // SAFETY: `webview` is borrowed for the call, and the handler owns the closure rather than
    // borrowing it. The token is a local out-parameter.
    unsafe {
        webview
            .add_WebMessageReceived(
                &WebMessageReceivedEventHandler::create(Box::new(move |_, args| {
                    let Some(args) = args else { return Ok(()) };
                    let mut source = PWSTR::null();
                    args.Source(&mut source)?;
                    let source = take_pwstr(source);
                    let mut body = PWSTR::null();
                    args.TryGetWebMessageAsString(&mut body)?;
                    handler(&source, &take_pwstr(body));
                    Ok(())
                })),
                &mut token,
            )
            .map_err(|error| error.to_string())
    }
}

#[cfg(not(windows))]
pub fn attach_ipc(_: &WebView, _: impl Fn(&str, &str) + 'static) -> Result<(), String> {
    Ok(())
}

/// Blocks ads in a WebView that is not a tab, such as the side web panel.
pub fn block_view_ads(view: &WebView) {
    #[cfg(windows)]
    {
        use wry::WebViewExtWindows;
        let _ = block_ads(&view.webview());
    }
    #[cfg(not(windows))]
    let _ = view;
}

/// Runs every request through Brave's engine and hides known ad slots on each page.
#[cfg(windows)]
fn block_ads(
    webview: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2,
) -> windows::core::Result<()> {
    use crate::adblock::{self, Block};
    use std::{cell::RefCell, rc::Rc};
    use webview2_com::{
        AddScriptToExecuteOnDocumentCreatedCompletedHandler,
        Microsoft::Web::WebView2::Win32::{
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_EVENT_SOURCE,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FETCH, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FONT,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_IMAGE, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_MEDIA,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_PING, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_SCRIPT,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_STYLESHEET,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_WEBSOCKET,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_XML_HTTP_REQUEST, ICoreWebView2_2,
        },
        NavigationStartingEventHandler, WebResourceRequestedEventHandler, take_pwstr,
    };
    use windows::Win32::UI::Shell::SHCreateMemStream;
    use windows::core::{HSTRING, Interface, PWSTR, w};
    if !adblock::enabled() {
        return Ok(());
    }
    // SAFETY: `webview` is borrowed for the call, and `cast` fails cleanly on runtimes without `ICoreWebView2_2`.
    let environment = unsafe { webview.cast::<ICoreWebView2_2>()?.Environment()? };
    let page = Rc::new(RefCell::new(String::new()));
    let scripts = Rc::new(RefCell::new(Vec::<String>::new()));
    let mut token = 0;
    // SAFETY: `webview` is borrowed for the call. Handlers capture `Rc` state that is used only on this UI thread.
    // Tokens are written to a local out-parameter.
    unsafe {
        webview.AddWebResourceRequestedFilter(w!("*"), COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL)?;
        let navigating = page.clone();
        webview.add_NavigationStarting(
            &NavigationStartingEventHandler::create(Box::new(move |view, args| {
                let (Some(view), Some(args)) = (view, args) else {
                    return Ok(());
                };
                let mut uri = PWSTR::null();
                args.Uri(&mut uri)?;
                let uri = take_pwstr(uri);
                for id in scripts.borrow_mut().drain(..) {
                    view.RemoveScriptToExecuteOnDocumentCreated(&HSTRING::from(id))?;
                }
                if let Some(script) = adblock::cosmetic_script(&uri) {
                    let added = scripts.clone();
                    view.AddScriptToExecuteOnDocumentCreated(
                        &HSTRING::from(script),
                        &AddScriptToExecuteOnDocumentCreatedCompletedHandler::create(Box::new(
                            move |result, id| {
                                if result.is_ok() {
                                    added.borrow_mut().push(id);
                                }
                                Ok(())
                            },
                        )),
                    )?;
                }
                if crate::settings::data_saver() {
                    let data_saver_script = "try{if(navigator.connection){Object.defineProperty(navigator.connection,'saveData',{get:()=>true,configurable:true});}}catch(_){}";
                    let added = scripts.clone();
                    view.AddScriptToExecuteOnDocumentCreated(
                        &HSTRING::from(data_saver_script),
                        &AddScriptToExecuteOnDocumentCreatedCompletedHandler::create(Box::new(
                            move |result, id| {
                                if result.is_ok() {
                                    added.borrow_mut().push(id);
                                }
                                Ok(())
                            },
                        )),
                    )?;
                }
                *navigating.borrow_mut() = uri;
                Ok(())
            })),
            &mut token,
        )?;
        webview.add_WebResourceRequested(
            &WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else { return Ok(()) };
                let mut context = COREWEBVIEW2_WEB_RESOURCE_CONTEXT::default();
                args.ResourceContext(&mut context)?;
                let data_saver = crate::settings::data_saver();
                if let Ok(request) = args.Request()
                    && data_saver
                    && let Ok(headers) = request.Headers()
                {
                    let _ =
                        headers.SetHeader(windows::core::w!("Save-Data"), windows::core::w!("on"));
                    let _ = headers.SetHeader(
                        windows::core::w!("Sec-CH-Save-Data"),
                        windows::core::w!("?1"),
                    );
                    let _ = headers.SetHeader(
                        windows::core::w!("Sec-CH-Prefers-Reduced-Data"),
                        windows::core::w!("1"),
                    );
                }
                // Under Data Saver mode, shed background beacon and ping requests immediately to conserve bandwidth.
                if data_saver && context == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_PING {
                    let response = environment.CreateWebResourceResponse(
                        None,
                        204,
                        w!("No Content"),
                        w!(""),
                    )?;
                    args.SetResponse(&response)?;
                    return Ok(());
                }
                // Documents load untouched; uBlock Origin already blocks ad frames.
                if context == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT {
                    return Ok(());
                }
                let request = args.Request()?;
                let mut uri = PWSTR::null();
                request.Uri(&mut uri)?;
                let uri = take_pwstr(uri);
                if !uri.starts_with("http://") && !uri.starts_with("https://") {
                    return Ok(());
                }
                let mut method = PWSTR::null();
                request.Method(&mut method)?;
                let method = take_pwstr(method);
                let page = page.borrow();
                let kind = match context {
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT => unreachable!(),
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_STYLESHEET => "stylesheet",
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_IMAGE => "image",
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_MEDIA => "media",
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FONT => "font",
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_SCRIPT => "script",
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_XML_HTTP_REQUEST
                    | COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FETCH
                    | COREWEBVIEW2_WEB_RESOURCE_CONTEXT_EVENT_SOURCE => "xmlhttprequest",
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_WEBSOCKET => "websocket",
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_PING => "ping",
                    _ => "other",
                };
                let response = match adblock::check(&uri, &page, kind, &method) {
                    None => return Ok(()),
                    Some(Block::Cancel) => {
                        environment.CreateWebResourceResponse(None, 403, w!("Blocked"), w!(""))?
                    }
                    Some(Block::Replace { mime, body }) => environment.CreateWebResourceResponse(
                        SHCreateMemStream(Some(&body)).as_ref(),
                        200,
                        w!("OK"),
                        &HSTRING::from(format!(
                            "Content-Type: {mime}\r\nAccess-Control-Allow-Origin: *"
                        )),
                    )?,
                };
                args.SetResponse(&response)
            })),
            &mut token,
        )?;
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn attach(_: &WebView, _: usize, _: u64, _: EventLoopProxy<UserEvent>) -> wry::Result<()> {
    Ok(())
}

/// Ensures unpacked extensions in `directory` are installed once without reloading existing ones.
#[cfg(windows)]
pub fn load_extensions(view: &WebView, directory: &std::path::Path) -> Result<(), String> {
    use webview2_com::{
        BrowserExtensionEnableCompletedHandler, BrowserExtensionRemoveCompletedHandler,
        Microsoft::Web::WebView2::Win32::{ICoreWebView2_13, ICoreWebView2Profile7},
        ProfileAddBrowserExtensionCompletedHandler, ProfileGetBrowserExtensionsCompletedHandler,
    };
    use windows::core::{BOOL, HSTRING, Interface, PWSTR};
    use wry::WebViewExtWindows;

    let mut unpacked = Vec::new();
    if let Ok(entries) = std::fs::read_dir(directory) {
        for entry in entries.flatten() {
            let path = std::path::absolute(entry.path()).unwrap_or_else(|_| entry.path());
            if path.join("manifest.json").is_file()
                && SYNCED_EXTENSIONS.with_borrow_mut(|synced| synced.insert(path.clone()))
            {
                unpacked.push((crate::extensions::unpacked_id(&path), path));
            }
        }
    }
    if unpacked.is_empty() {
        return Ok(());
    }
    // SAFETY: `view.webview()` is borrowed for the call, and each `cast` fails cleanly on older runtimes.
    // The handler clones `profile` rather than borrowing it, so it can outlive this call.
    unsafe {
        let profile = view
            .webview()
            .cast::<ICoreWebView2_13>()
            .and_then(|view| view.Profile())
            .and_then(|profile| profile.cast::<ICoreWebView2Profile7>())
            .map_err(|error| error.to_string())?;
        let profile_for_add = profile.clone();
        profile
            .GetBrowserExtensions(&ProfileGetBrowserExtensionsCompletedHandler::create(
                Box::new(move |result, list| {
                    result?;
                    let mut present = std::collections::HashSet::new();
                    if let Some(list) = list {
                        let mut count = 0;
                        list.Count(&mut count)?;
                        for index in 0..count {
                            if let Ok(extension) = list.GetValueAtIndex(index) {
                                let mut raw_id = PWSTR::null();
                                extension.Id(&mut raw_id)?;
                                let id = webview2_com::take_pwstr(raw_id);
                                let mut raw_name = PWSTR::null();
                                extension.Name(&mut raw_name)?;
                                let name = webview2_com::take_pwstr(raw_name);
                                if unpacked.iter().any(|(expected, _)| expected == &id) {
                                    let mut enabled = BOOL::default();
                                    extension.IsEnabled(&mut enabled)?;
                                    if !enabled.as_bool() {
                                        let _ = extension.Enable(
                                            true,
                                            &BrowserExtensionEnableCompletedHandler::create(
                                                Box::new(|_| Ok(())),
                                            ),
                                        );
                                    }
                                    present.insert(id);
                                } else if name == "uBlock Origin Lite" {
                                    let _ = extension.Remove(
                                        &BrowserExtensionRemoveCompletedHandler::create(Box::new(
                                            |_| Ok(()),
                                        )),
                                    );
                                }
                            }
                        }
                    }
                    for (id, path) in &unpacked {
                        if !present.contains(id) {
                            let path_hs = HSTRING::from(path.as_path());
                            let _ = profile_for_add.AddBrowserExtension(
                                &path_hs,
                                &ProfileAddBrowserExtensionCompletedHandler::create(Box::new(
                                    |_, _| Ok(()),
                                )),
                            );
                        }
                    }
                    Ok(())
                }),
            ))
            .map_err(|error| error.to_string())
    }
}

#[cfg(not(windows))]
pub fn load_extensions(_: &WebView, _: &std::path::Path) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
pub fn attach_popup(view: &WebView, proxy: EventLoopProxy<UserEvent>) -> wry::Result<()> {
    use webview2_com::{
        NavigationCompletedEventHandler, NavigationStartingEventHandler, SourceChangedEventHandler,
        WindowCloseRequestedEventHandler, take_pwstr,
    };
    use windows::core::{BOOL, PWSTR};
    use wry::WebViewExtWindows;
    let webview = view.webview();
    let mut token = 0;
    let _ = block_ads(&webview);
    // SAFETY: `webview` is borrowed for the call, and the handler owns a cloned proxy, not a borrow.
    // The token is a local out-parameter.
    unsafe {
        let close = proxy.clone();
        webview.add_WindowCloseRequested(
            &WindowCloseRequestedEventHandler::create(Box::new(move |_, _| {
                let _ = close.send_event(UserEvent::PopupClosed);
                Ok(())
            })),
            &mut token,
        )?;
        let started = proxy.clone();
        webview.add_NavigationStarting(
            &NavigationStartingEventHandler::create(Box::new(move |_, args| {
                if let Some(args) = args {
                    let mut uri = PWSTR::null();
                    args.Uri(&mut uri)?;
                    let _ = started.send_event(UserEvent::PopupState(take_pwstr(uri), true));
                }
                Ok(())
            })),
            &mut token,
        )?;
        let completed = proxy.clone();
        webview.add_NavigationCompleted(
            &NavigationCompletedEventHandler::create(Box::new(move |view, _| {
                if let Some(view) = view {
                    let mut uri = PWSTR::null();
                    view.Source(&mut uri)?;
                    let _ = completed.send_event(UserEvent::PopupState(take_pwstr(uri), false));
                }
                Ok(())
            })),
            &mut token,
        )?;
        let source = proxy;
        webview.add_SourceChanged(
            &SourceChangedEventHandler::create(Box::new(move |view, args| {
                if let (Some(view), Some(args)) = (view, args) {
                    let mut new_document = BOOL::default();
                    args.IsNewDocument(&mut new_document)?;
                    if !new_document.as_bool() {
                        let mut uri = PWSTR::null();
                        view.Source(&mut uri)?;
                        let _ = source.send_event(UserEvent::PopupState(take_pwstr(uri), false));
                    }
                }
                Ok(())
            })),
            &mut token,
        )?;
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn attach_popup(_: &WebView, _: EventLoopProxy<UserEvent>) -> wry::Result<()> {
    Ok(())
}

pub fn navigate(view: &WebView, backwards: bool) {
    #[cfg(windows)]
    {
        use wry::WebViewExtWindows;
        // SAFETY: `view.webview()` is borrowed for the call. `GoBack` and `GoForward` take no pointers.
        unsafe {
            let _ = if backwards {
                view.webview().GoBack()
            } else {
                view.webview().GoForward()
            };
        }
    }
    #[cfg(not(windows))]
    {
        let _ = view.evaluate_script(if backwards {
            "history.back()"
        } else {
            "history.forward()"
        });
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::{
        collections::HashMap,
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        sync::mpsc,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };
    use tao::{
        event_loop::{ControlFlow, EventLoopBuilder},
        platform::{run_return::EventLoopExtRunReturn, windows::EventLoopBuilderExtWindows},
        window::WindowBuilder,
    };
    use webview2_com::{
        DownloadStartingEventHandler, Microsoft::Web::WebView2::Win32::ICoreWebView2_4,
    };
    use windows::core::Interface;
    use wry::{WebContext, WebViewBuilder, WebViewExtWindows};

    #[test]
    fn completed_download_clears_without_completion_notifications() {
        let directory = std::env::temp_dir().join(format!(
            "veluno-download-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let destination = directory.join("download.txt");
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (release, wait) = mpsc::channel();
        let server = std::thread::spawn(move || {
            loop {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(10)))
                    .unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = BufReader::new(&stream);
                let mut first = String::new();
                // Chromium can open and close a speculative connection without a request.
                if !matches!(request.read_line(&mut first), Ok(1..)) {
                    continue;
                }
                loop {
                    let mut line = String::new();
                    assert!(request.read_line(&mut line).unwrap() > 0);
                    if line == "\r\n" {
                        break;
                    }
                }
                static CHUNK: [u8; 32_768] = [b'x'; 32_768];
                stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=download.txt\r\nContent-Length: 65536\r\nConnection: close\r\n\r\n").unwrap();
                stream.write_all(&CHUNK).unwrap();
                wait.recv_timeout(Duration::from_secs(15)).unwrap();
                stream.write_all(&CHUNK).unwrap();
                break;
            }
        });
        let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event()
            .with_any_thread(true)
            .build();
        let window = WindowBuilder::new()
            .with_visible(false)
            .build(&event_loop)
            .unwrap();
        let mut context = WebContext::new(Some(directory.join("webview")));
        let file = destination.clone();
        let view = WebViewBuilder::new_with_web_context(&mut context)
            .with_download_started_handler(move |_, path| {
                *path = file.clone();
                true
            })
            .build(&window)
            .unwrap();
        // SAFETY: The test created `view` on this thread and keeps it alive until the test ends.
        // `cast` fails the test if the runtime lacks the interface, and the handler pushes only into the
        // thread-local `DOWNLOADS` list.
        unsafe {
            view.webview()
                .cast::<ICoreWebView2_4>()
                .unwrap()
                .add_DownloadStarting(
                    &DownloadStartingEventHandler::create(Box::new(|_, args| {
                        if let Some(args) = args {
                            let operation = args.DownloadOperation()?;
                            // Deliberately omit StateChanged to exercise missed notifications.
                            DOWNLOADS
                                .with_borrow_mut(|downloads| downloads.push((7, operation, None)));
                        }
                        Ok(())
                    })),
                    &mut 0,
                )
                .unwrap();
        }
        view.load_url(&format!("http://{address}/download"))
            .unwrap();
        let views = HashMap::from([(7, view)]);
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut started = false;
        let mut completed = false;
        event_loop.run_return(|_, _, flow| {
            *flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(10));
            let counts = download_counts(&views);
            if !started && counts.get(&7) == Some(&1) {
                started = true;
                release.send(()).unwrap();
            }
            if started && counts.is_empty() {
                completed = true;
            }
            if completed || Instant::now() >= deadline {
                *flow = ControlFlow::Exit;
            }
        });
        let _ = release.send(());
        assert!(started, "Download never became active");
        assert!(
            completed,
            "Completed download stayed active without a notification"
        );
        assert_eq!(std::fs::read(destination).unwrap(), vec![b'x'; 65536]);
        server.join().unwrap();
        DOWNLOADS.with_borrow(|downloads| assert!(downloads.is_empty()));
        drop(views);
        drop(context);
        // WebView2 may still hold profile files while its child processes exit.
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn clear_downloads_removes_completed_and_preserves_active() {
        DOWNLOAD_ITEMS.with_borrow_mut(|items| {
            items.clear();
            items.push(std::rc::Rc::new(std::cell::RefCell::new(DownloadInfo {
                id: 1,
                path: "C:\\test\\file1.txt".into(),
                received: 100,
                total: 100,
                status: "complete".into(),
            })));
            items.push(std::rc::Rc::new(std::cell::RefCell::new(DownloadInfo {
                id: 2,
                path: "C:\\test\\file2.txt".into(),
                received: 50,
                total: 100,
                status: "downloading".into(),
            })));
            items.push(std::rc::Rc::new(std::cell::RefCell::new(DownloadInfo {
                id: 3,
                path: "C:\\test\\file3.txt".into(),
                received: 20,
                total: 100,
                status: "cancelled".into(),
            })));
        });
        clear_downloads();
        let items = download_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, 2);
        assert_eq!(items[0].status, "downloading");
        DOWNLOAD_ITEMS.with_borrow_mut(|items| items.clear());
    }

    #[test]
    fn screenshot_names_are_safe_windows_file_names() {
        let stamp = [2026, 3, 7, 9, 5, 2];
        assert_eq!(
            file_name("Rust: A <Language> / Guide?", stamp),
            "Rust A Language Guide 2026-03-07 09.05.02.png"
        );
        assert_eq!(
            file_name(" 	... ", stamp),
            "Screenshot 2026-03-07 09.05.02.png"
        );
        let long = file_name(&"x".repeat(200), stamp);
        assert!(long.starts_with(&"x".repeat(60)) && !long.starts_with(&"x".repeat(61)));
    }

    #[test]
    fn a_second_screenshot_in_the_same_second_gets_its_own_name() {
        let folder = std::env::temp_dir().join(format!("veluno-shots-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let name = "Example 2026-03-07 09.05.02.png".to_string();
        assert_eq!(unique_path(&folder, name.clone()), folder.join(&name));
        std::fs::write(folder.join(&name), b"png").unwrap();
        assert_eq!(
            unique_path(&folder, name.clone()),
            folder.join("Example 2026-03-07 09.05.02 (2).png")
        );
        std::fs::write(folder.join("Example 2026-03-07 09.05.02 (2).png"), b"png").unwrap();
        assert_eq!(
            unique_path(&folder, name),
            folder.join("Example 2026-03-07 09.05.02 (3).png")
        );
        std::fs::remove_dir_all(folder).unwrap();
    }
}
