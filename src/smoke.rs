use crate::{
    app::{Command, UserEvent},
    desktop, profile,
    state::BrowserState,
};
use std::{collections::HashMap, path::PathBuf};
use tao::event_loop::EventLoopProxy;
use wry::WebView;

pub struct Smoke {
    report: PathBuf,
    origin: String,
    phase: usize,
    ticks: usize,
    first: usize,
    restore: bool,
    devtools: bool,
    done: bool,
}

impl Smoke {
    pub fn new() -> Option<Self> {
        let report = std::env::var_os("VELUNO_SMOKE_TEST")?;
        // Diagnostic runs must never use the user's real browser profile.
        std::env::var_os("VELUNO_PROFILE_DIR")?;
        let origin = std::env::var("VELUNO_SMOKE_ORIGIN").ok()?;
        let url = url::Url::parse(&origin).ok()?;
        if url.host_str() != Some("127.0.0.1") {
            return None;
        }
        Some(Self {
            report: report.into(),
            origin,
            phase: 0,
            ticks: 0,
            first: 0,
            restore: std::env::var_os("VELUNO_SMOKE_RESTORE").is_some(),
            devtools: false,
            done: false,
        })
    }

    fn finish(&mut self, success: bool, message: &str) -> bool {
        let report = serde_json::json!({ "success":success, "message":message, "phase":self.phase, "restore":self.restore, "devtools":self.devtools });
        let _ = profile::write_atomic(&self.report, report.to_string().as_bytes());
        self.done = true;
        !success || std::env::var_os("VELUNO_SMOKE_HOLD").is_none()
    }

    fn command(proxy: &EventLoopProxy<UserEvent>, command: Command) {
        let _ = proxy.send_event(UserEvent::Command(command));
    }

    fn probe(&self, view: &WebView, script: &str, proxy: &EventLoopProxy<UserEvent>) {
        let phase = self.phase;
        let proxy = proxy.clone();
        let _ = view.evaluate_script_with_callback(script, move |result| {
            let _ = proxy.send_event(UserEvent::SmokeResult(phase, result == "true"));
        });
    }

    pub fn result(
        &mut self,
        phase: usize,
        success: bool,
        proxy: &EventLoopProxy<UserEvent>,
    ) -> bool {
        if !success {
            return self.finish(false, "A native WebView assertion failed");
        }
        match phase {
            1 => {
                self.phase = 2;
                Self::command(
                    proxy,
                    Command::NewTab {
                        url: Some(format!("{}/second", self.origin)),
                    },
                );
            }
            4 => {
                self.phase = 5;
                Self::command(
                    proxy,
                    Command::Navigate {
                        url: format!("{}/next", self.origin),
                    },
                );
            }
            15 => {
                return self.finish(
                    true,
                    "Session, workspace, pins, and site storage restored after forced exit",
                );
            }
            16 => {
                self.phase = 21;
                Self::command(proxy, Command::ToggleSidebar);
                Self::command(proxy, Command::FocusAddress);
            }
            22 => {
                self.phase = 17;
                Self::command(proxy, Command::NewTab { url: None });
            }
            18 => {
                self.phase = 19;
                let _ = proxy.send_event(UserEvent::Ready);
            }
            20 => {
                Self::command(proxy, Command::DismissNewTab);
                return self.finish(true, "Live tabs, history, popups, downloads, errors/retry, address editing, palette drafts, and session saves passed");
            }
            _ => {}
        }
        false
    }

    pub fn tick(
        &mut self,
        state: &BrowserState,
        views: &HashMap<usize, WebView>,
        sidebar: &WebView,
        palette: &WebView,
        proxy: &EventLoopProxy<UserEvent>,
        session: &crate::session::SessionWriter,
    ) -> bool {
        if self.done {
            return false;
        }
        self.ticks += 1;
        if self.ticks > 250 {
            return self.finish(false, "Native smoke test timed out");
        }
        let Some(tab) = state.active_tab() else {
            return false;
        };
        let Some(view) = views.get(&tab.id) else {
            return false;
        };
        self.devtools = desktop::devtools_enabled(view);
        if self.restore && self.phase == 0 {
            if state.active_workspace_id != 2 || !state.tabs.iter().any(|tab| tab.is_pinned) {
                return self.finish(false, "Session or pinned tabs were lost");
            }
            if !tab.is_loading && tab.url.ends_with("/second") && tab.title == "Veluno second" {
                self.phase = 15;
                self.probe(
                    view,
                    "localStorage.getItem('veluno-smoke') === 'saved'",
                    proxy,
                );
            }
            return false;
        }
        match self.phase {
            0 if !tab.is_loading && tab.title == "Veluno fixture" => {
                self.first = tab.id;
                self.phase = 1;
                self.probe(view,"(()=>{document.getElementById('note').value='retained';localStorage.setItem('veluno-smoke','saved');return true})()",proxy);
            }
            2 if !tab.is_loading && tab.title == "Veluno second" => {
                self.phase = 3;
                let _ = proxy.send_event(UserEvent::AdBlockerInstalled(Ok(())));
                Self::command(proxy, Command::SwitchTab { id: self.first });
            }
            3 if tab.id == self.first && !tab.is_loading => {
                self.phase = 4;
                self.probe(
                    view,
                    "document.getElementById('note').value === 'retained'",
                    proxy,
                );
            }
            5 if !tab.is_loading && tab.url.ends_with("/next") && tab.can_go_back => {
                self.phase = 6;
                Self::command(proxy, Command::Back);
            }
            6 if !tab.is_loading
                && tab.url == format!("{}/", self.origin)
                && tab.can_go_forward =>
            {
                self.phase = 7;
                Self::command(proxy, Command::Forward);
            }
            7 if !tab.is_loading && tab.url.ends_with("/next") => {
                self.phase = 8;
                let url = serde_json::to_string(&format!("{}/popup", self.origin)).unwrap();
                let _ = view.evaluate_script(&format!("window.open({url},'_blank')"));
            }
            8 if !tab.is_loading && tab.title == "Veluno popup" => {
                self.phase = 9;
                let _ = view.evaluate_script("document.getElementById('download').click()");
            }
            9 if profile::directory().join("smoke-download.txt").exists()
                && state.active_downloads == 0 =>
            {
                if std::fs::read_to_string(profile::directory().join("smoke-download.txt"))
                    .ok()
                    .as_deref()
                    != Some("Veluno download fixture\n")
                {
                    return self.finish(false, "Download contents did not match");
                }
                self.phase = 10;
                Self::command(
                    proxy,
                    Command::Navigate {
                        url: "http://127.0.0.1:1/unreachable".into(),
                    },
                );
            }
            10 if tab.error.is_some() && !tab.is_loading => {
                self.phase = 12;
                Self::command(proxy, Command::Retry);
            }
            12 if tab.error.is_some() && !tab.is_loading => {
                self.phase = 13;
                Self::command(
                    proxy,
                    Command::Navigate {
                        url: format!("{}/", self.origin),
                    },
                );
            }
            13 if !tab.is_loading && tab.error.is_none() && tab.title == "Veluno fixture" => {
                self.phase = 14;
                Self::command(proxy, Command::TogglePin { id: self.first });
                Self::command(
                    proxy,
                    Command::CreateProfile {
                        name: "School".into(),
                    },
                );
                Self::command(
                    proxy,
                    Command::NewTab {
                        url: Some(format!("{}/second", self.origin)),
                    },
                );
            }
            14 if !tab.is_loading
                && tab.title == "Veluno second"
                && state.active_workspace_id == 2 =>
            {
                if let Err(error) = session.flush(state) {
                    return self.finish(false, &error);
                }
                self.phase = 16;
                if let Ok(json) = serde_json::to_string(state) {
                    self.probe(sidebar, &format!("(()=>{{const input=document.getElementById('url-input');input.focus();input.value='draft address';window.updateState({json});return input.value==='draft address'&&document.querySelector('footer').getBoundingClientRect().bottom<=innerHeight}})()"), proxy);
                }
            }
            21 => {
                self.phase = 22;
                self.probe(sidebar, "document.activeElement.id === 'url-input' && !document.body.classList.contains('collapsed')", proxy);
            }
            17 => {
                self.phase = 18;
                self.probe(palette, "(()=>{const input=document.getElementById('new-tab-input');input.value='retained search';return !input.readOnly})()", proxy);
            }
            19 => {
                self.phase = 20;
                self.probe(
                    palette,
                    "document.getElementById('new-tab-input').value === 'retained search'",
                    proxy,
                );
            }
            _ => {}
        }
        false
    }
}
