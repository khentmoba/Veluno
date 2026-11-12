use crate::{app::UserEvent, desktop, state::BrowserState};
use serde::Deserialize;
use std::{cell::RefCell, collections::VecDeque};
use tao::{event_loop::EventLoopProxy, window::Window};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_PERMISSION_STATE_ALLOW, COREWEBVIEW2_PERMISSION_STATE_DENY, ICoreWebView2Deferral,
    ICoreWebView2Environment, ICoreWebView2PermissionRequestedEventArgs,
    ICoreWebView2PermissionRequestedEventArgs3,
};
use windows::core::Interface;
use wry::{PageLoadEvent, Rect, WebContext, WebView, WebViewBuilder};

thread_local! {
    // WebView2 event arguments and deferrals must stay on the UI thread.
    static REQUESTS: RefCell<VecDeque<Request>> = const { RefCell::new(VecDeque::new()) };
}

struct Request {
    token: u64,
    tab: usize,
    generation: u64,
    origin: String,
    kind: &'static str,
    args: ICoreWebView2PermissionRequestedEventArgs,
    deferral: Option<ICoreWebView2Deferral>,
    proxy: EventLoopProxy<UserEvent>,
}

fn request_is_current(state: &BrowserState, tab: usize, generation: u64, origin: &str) -> bool {
    state.active_tab().is_some_and(|active| {
        active.id == tab
            && active.generation == generation
            && active.error.is_none()
            && url::Url::parse(active.pending_url.as_ref().unwrap_or(&active.url))
                .is_ok_and(|url| url.origin().ascii_serialization() == origin)
    })
}

impl Request {
    fn current(&self, state: &BrowserState) -> bool {
        request_is_current(state, self.tab, self.generation, &self.origin)
    }

    fn finish(&mut self, allow: bool, save: bool) {
        if let Some(deferral) = self.deferral.take() {
            // SAFETY: `REQUESTS` is thread-local, so `args` and `deferral` are used only on the UI thread that received them.
            // Finishing or dropping a request touches only those objects.
            unsafe {
                if let Ok(args) = self
                    .args
                    .cast::<ICoreWebView2PermissionRequestedEventArgs3>()
                {
                    let _ = args.SetSavesInProfile(save);
                }
                let granted = self
                    .args
                    .SetState(if allow {
                        COREWEBVIEW2_PERMISSION_STATE_ALLOW
                    } else {
                        COREWEBVIEW2_PERMISSION_STATE_DENY
                    })
                    .is_ok();
                let _ = deferral.Complete();
                if allow && granted && matches!(self.kind, "camera" | "microphone") {
                    let _ = self
                        .proxy
                        .send_event(UserEvent::CaptureAllowed(self.tab, self.generation));
                }
            }
        }
    }
}

impl Drop for Request {
    fn drop(&mut self) {
        self.finish(false, false);
    }
}

pub fn request(
    args: &ICoreWebView2PermissionRequestedEventArgs,
    tab: usize,
    generation: u64,
    origin: String,
    kind: &'static str,
    proxy: EventLoopProxy<UserEvent>,
) -> windows::core::Result<()> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    // SAFETY: This runs in the WebView2 permission handler on the UI thread, so `args` is a live COM reference.
    // `GetDeferral` keeps the event open until `Request::finish` completes it.
    unsafe {
        if let Ok(args) = args.cast::<ICoreWebView2PermissionRequestedEventArgs3>() {
            args.SetSavesInProfile(false)?;
        }
        let request = Request {
            token: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            tab,
            generation,
            origin,
            kind,
            args: args.clone(),
            deferral: Some(args.GetDeferral()?),
            proxy: proxy.clone(),
        };
        REQUESTS.with_borrow_mut(|requests| requests.push_back(request));
        if proxy.send_event(UserEvent::PermissionRequested).is_err() {
            REQUESTS.with_borrow_mut(|requests| requests.clear());
        }
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct Response {
    pub token: u64,
    pub allow: bool,
}

pub struct Prompt {
    view: WebView,
    pending: Option<Request>,
    ready: bool,
    dirty: bool,
    visible: bool,
}

impl Prompt {
    pub fn new(
        window: &Window,
        context: &mut WebContext,
        proxy: EventLoopProxy<UserEvent>,
        _headless: bool,
        environment: ICoreWebView2Environment,
    ) -> wry::Result<Self> {
        let loaded = proxy.clone();
        let mut builder = WebViewBuilder::new_with_web_context(context)
            .with_visible(false)
            .with_focused(false)
            .with_transparent(true)
            .with_background_color((0, 0, 0, 0))
            .with_html(include_str!("permission.html"))
            .with_navigation_handler(|url| {
                url == "about:blank" || url.starts_with("data:text/html;")
            })
            .with_on_page_load_handler(move |event, _| {
                if matches!(event, PageLoadEvent::Finished) {
                    let _ = loaded.send_event(UserEvent::PermissionReady);
                }
            })
            .with_ipc_handler(move |request| {
                if let Ok(response) = serde_json::from_str::<Response>(request.body()) {
                    let _ = proxy.send_event(UserEvent::PermissionResponse(
                        response.token,
                        response.allow,
                    ));
                }
            });
        #[cfg(windows)]
        {
            use wry::WebViewBuilderExtWindows;
            builder = builder
                .with_additional_browser_args(crate::app::chrome_browser_arguments())
                .with_environment(environment);
        }
        Ok(Self {
            view: builder.build_as_child(window)?,
            pending: None,
            ready: false,
            dirty: false,
            visible: true,
        })
    }

    pub fn ready(&mut self) {
        self.ready = true;
        self.dirty = true;
    }

    pub fn respond(&mut self, token: u64, allow: bool, state: &BrowserState) -> bool {
        if self
            .pending
            .as_ref()
            .is_some_and(|request| request.token == token)
            && let Some(mut request) = self.pending.take()
        {
            let current = request.current(state);
            request.finish(allow && current, current);
            return true;
        }
        false
    }

    pub fn cancel_tab(&mut self, tab: usize) {
        if self
            .pending
            .as_ref()
            .is_some_and(|request| request.tab == tab)
        {
            self.pending.take();
        }
        REQUESTS.with_borrow_mut(|requests| requests.retain(|request| request.tab != tab));
    }

    pub fn sync(&mut self, state: &BrowserState, bounds: Rect, blocked: bool, headless: bool) {
        if self
            .pending
            .as_ref()
            .is_some_and(|request| blocked || !request.current(state))
        {
            self.pending.take();
        }
        while self.pending.is_none() {
            let Some(request) = REQUESTS.with_borrow_mut(|requests| requests.pop_front()) else {
                break;
            };
            if !blocked && request.current(state) {
                self.pending = Some(request);
                self.dirty = true;
            }
        }
        let visible = self.ready && self.pending.is_some();
        if visible {
            let _ = self.view.set_bounds(bounds);
            if self.dirty
                && let Some(request) = &self.pending
            {
                let data = serde_json::json!({ "token":request.token, "origin":request.origin, "kind":request.kind });
                if self
                    .view
                    .evaluate_script(&format!("window.showPermission({data})"))
                    .is_ok()
                {
                    self.dirty = false;
                }
            }
            desktop::raise(&self.view);
        }
        if visible != self.visible {
            if !visible {
                let _ = self.view.evaluate_script("window.hidePermission()");
            }
            desktop::show(&self.view, visible);
            if !headless && visible {
                let _ = self.view.focus();
            }
            self.visible = visible;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_requests_follow_only_the_live_tab_and_origin() {
        let mut state = BrowserState::new();
        state.tabs[0].url = "https://example.com/page".into();
        let (tab, generation) = (state.tabs[0].id, state.tabs[0].generation);
        assert!(request_is_current(
            &state,
            tab,
            generation,
            "https://example.com"
        ));
        assert!(!request_is_current(
            &state,
            tab,
            generation,
            "https://evil.example"
        ));
        assert!(!request_is_current(
            &state,
            tab,
            generation + 1,
            "https://example.com"
        ));
        assert!(!request_is_current(
            &state,
            tab + 1,
            generation,
            "https://example.com"
        ));
        state.tabs[0].error = Some("failed".into());
        assert!(!request_is_current(
            &state,
            tab,
            generation,
            "https://example.com"
        ));
        state.tabs[0].error = None;
        state.tabs[0].pending_url = Some("https://other.example/".into());
        assert!(!request_is_current(
            &state,
            tab,
            generation,
            "https://example.com"
        ));
    }
}
