use serde::{Deserialize, Serialize};

use crate::updater::UpdateManifest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: usize,
    pub name: String,
    pub icon: String,
    pub accent: String,
    /// An emoji shown beside the workspace name, chosen by the user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<String>,
    /// Private profiles open InPrivate pages and are never saved or recorded in history.
    #[serde(default)]
    pub private: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bookmark {
    pub url: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tab {
    pub id: usize,
    pub workspace_id: usize,
    pub title: String,
    pub url: String,
    pub is_pinned: bool,
    pub is_sleeping: bool,
    #[serde(default)]
    pub is_loading: bool,
    #[serde(default)]
    pub favicon: Option<String>,
    #[serde(default)]
    pub can_go_back: bool,
    #[serde(default)]
    pub can_go_forward: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(skip)]
    pub generation: u64,
    #[serde(skip)]
    pub pending_url: Option<String>,
    #[serde(skip)]
    pub last_active: u64,
    #[serde(skip)]
    pub has_capture: bool,
    #[serde(default, skip_deserializing)]
    pub is_playing_audio: bool,
    #[serde(default)]
    pub is_muted: bool,
    #[serde(skip)]
    pub opener: Option<usize>,
    #[serde(skip)]
    pub document_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shortcut {
    pub host: String,
    pub title: String,
    pub url: String,
    pub visits: usize,
    #[serde(default)]
    pub favicon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserState {
    pub tabs: Vec<Tab>,
    pub active_tab_id: usize,
    pub active_workspace_id: usize,
    pub workspaces: Vec<Workspace>,
    #[serde(default)]
    profiles_version: u8,
    pub sidebar_collapsed: bool,
    #[serde(default)]
    pub sidebar_peek: bool,
    #[serde(skip)]
    pub topbar_peek: bool,
    pub is_loading: bool,
    pub can_restore_closed_tab: bool,
    pub shortcuts: Vec<Shortcut>,
    #[serde(skip)]
    next_tab_id: usize,
    #[serde(skip)]
    activity_clock: u64,
    pub homepage: String,
    #[serde(default)]
    pub welcome_dismissed: bool,
    #[serde(default)]
    pub closed_tabs: Vec<Tab>,
    #[serde(default)]
    pub bookmarks: Vec<Bookmark>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_available: Option<UpdateManifest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_status: Option<String>,
    #[serde(default)]
    pub dismissed_update_version: Option<String>,
    #[serde(default, skip_deserializing)]
    pub downloads_open: bool,
    #[serde(default, skip_deserializing)]
    pub download_items: Vec<crate::desktop::DownloadInfo>,
    #[serde(default)]
    pub active_downloads: usize,
    #[serde(default, skip_deserializing)]
    pub is_default_browser: bool,
    #[serde(default)]
    pub update_busy: bool,
    #[serde(default)]
    pub zoom: u32,
    /// Two tabs shown side by side whenever either is active. Not restored after a restart.
    #[serde(default, skip_deserializing)]
    pub split: Option<[usize; 2]>,
    /// The site docked in the side web panel, kept across restarts.
    #[serde(default)]
    pub side_panel_url: Option<String>,
    #[serde(default)]
    pub side_panel_open: bool,
}

impl BrowserState {
    pub fn new() -> Self {
        let workspaces = vec![Workspace {
            id: 1,
            name: "General".into(),
            icon: "home".into(),
            accent: "#dedee3".into(),
            emoji: None,
            private: false,
        }];

        let initial_tab = Tab {
            id: 1,
            workspace_id: 1,
            title: "New Tab".to_string(),
            url: "veluno://newtab".to_string(),
            is_pinned: false,
            is_sleeping: false,
            is_loading: false,
            favicon: None,
            can_go_back: false,
            can_go_forward: false,
            error: None,
            generation: 0,
            pending_url: None,
            last_active: 0,
            has_capture: false,
            is_playing_audio: false,
            is_muted: false,
            opener: None,
            document_url: None,
        };

        Self {
            tabs: vec![initial_tab],
            active_tab_id: 1,
            active_workspace_id: 1,
            workspaces,
            profiles_version: 1,
            sidebar_collapsed: false,
            sidebar_peek: false,
            topbar_peek: false,
            is_loading: false,
            can_restore_closed_tab: false,
            shortcuts: Self::load_shortcuts(),
            next_tab_id: 2,
            activity_clock: 0,
            homepage: "veluno://newtab".to_string(),
            welcome_dismissed: false,
            closed_tabs: Vec::new(),
            bookmarks: Vec::new(),
            update_available: None,
            update_status: None,
            dismissed_update_version: None,
            downloads_open: false,
            download_items: Vec::new(),
            active_downloads: 0,
            is_default_browser: false,
            update_busy: false,
            zoom: 100,
            split: None,
            side_panel_url: None,
            side_panel_open: false,
        }
    }

    pub fn restore() -> Self {
        let path = crate::profile::directory().join("session.json");
        for candidate in [path.clone(), path.with_extension("json.bak")] {
            if let Ok(json) = std::fs::read_to_string(&candidate)
                && let Some(mut state) = Self::from_session(&json)
            {
                if candidate != path {
                    state.update_status =
                        Some("Recovered your previous session from a backup.".into());
                }
                return state;
            }
        }
        let mut state = Self::new();
        if path.exists() {
            state.update_status =
                Some("Your saved session could not be read. A new session was opened.".into());
        }
        state
    }

    pub(crate) fn from_session(json: &str) -> Option<Self> {
        let mut state: Self = serde_json::from_str(json).ok()?;
        for tab in state.tabs.iter_mut().chain(state.closed_tabs.iter_mut()) {
            if let Some(url) = crate::profile::normalize_url(&tab.url) {
                tab.url = url;
            }
        }
        for bookmark in &mut state.bookmarks {
            if let Some(url) = crate::profile::normalize_url(&bookmark.url) {
                bookmark.url = url;
            }
        }
        // Fold the old preset spaces into General without losing their tabs or pins.
        if state.profiles_version == 0
            && state.workspaces.len() == 3
            && state
                .workspaces
                .iter()
                .map(|w| (w.id, w.name.as_str()))
                .eq([(1, "General"), (2, "Dev & Code"), (3, "Deep Dive")])
        {
            state.workspaces = Self::new().workspaces;
            for tab in state.tabs.iter_mut().chain(state.closed_tabs.iter_mut()) {
                tab.workspace_id = 1;
            }
        }
        state.profiles_version = 1;
        let mut workspace_ids = std::collections::HashSet::new();
        state.workspaces.retain(|w| {
            !w.private
                && w.id > 0
                && w.id < usize::MAX - 1
                && !w.name.trim().is_empty()
                && workspace_ids.insert(w.id)
        });
        if state.workspaces.is_empty() {
            state.workspaces = Self::new().workspaces;
            workspace_ids.insert(1);
        }
        let mut ids = std::collections::HashSet::new();
        state.tabs.retain(|tab| {
            tab.id > 0
                && tab.id < usize::MAX - 1
                && workspace_ids.contains(&tab.workspace_id)
                && crate::profile::allowed_url(&tab.url)
                && ids.insert(tab.id)
        });
        if state.tabs.is_empty() {
            state.active_workspace_id = state.workspaces.first().map_or(1, |w| w.id);
            state.add_tab(None, false);
        }
        for tab in &mut state.tabs {
            tab.is_loading = false;
            tab.can_go_back = false;
            tab.can_go_forward = false;
            tab.error = None;
            tab.is_sleeping = tab.id != state.active_tab_id;
        }
        if !state.tabs.iter().any(|tab| tab.id == state.active_tab_id) {
            state.active_tab_id = state.tabs[0].id;
        }
        state.active_workspace_id = state.active_tab()?.workspace_id;
        state
            .tabs
            .iter_mut()
            .find(|tab| tab.id == state.active_tab_id)?
            .is_sleeping = false;
        state.next_tab_id = state.tabs.iter().map(|tab| tab.id).max()?.checked_add(1)?;
        state.closed_tabs.retain(|tab| {
            crate::profile::allowed_url(&tab.url) && workspace_ids.contains(&tab.workspace_id)
        });
        state.closed_tabs.truncate(30);
        state
            .bookmarks
            .retain(|bookmark| crate::profile::allowed_url(&bookmark.url));
        state.can_restore_closed_tab = !state.closed_tabs.is_empty();
        state.is_loading = false;
        state.active_downloads = 0;
        state.update_busy = false;
        state.zoom = 100;
        state.update_status = None;
        state.update_available = None;
        state.homepage = if state.welcome_dismissed {
            "veluno://blank"
        } else {
            "veluno://newtab"
        }
        .into();
        Some(state)
    }

    pub fn add_tab(&mut self, url: Option<String>, is_pinned: bool) -> usize {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        let target_url = url
            .map(|value| crate::profile::normalize_url(&value).unwrap_or(value))
            .unwrap_or_else(|| self.homepage.clone());
        let new_tab = Tab {
            id,
            workspace_id: self.active_workspace_id,
            title: match target_url.as_str() {
                "veluno://newtab" | "veluno://blank" => "New Tab".to_string(),
                "veluno://settings" => "Settings".to_string(),
                "veluno://history" => "History".to_string(),
                _ => target_url.clone(),
            },
            url: target_url.clone(),
            is_pinned,
            is_sleeping: false,
            is_loading: false,
            favicon: None,
            can_go_back: false,
            can_go_forward: false,
            error: None,
            generation: 0,
            pending_url: None,
            last_active: self.activity_clock.saturating_add(1),
            has_capture: false,
            is_playing_audio: false,
            is_muted: false,
            opener: Some(self.active_tab_id),
            document_url: None,
        };
        self.activity_clock = new_tab.last_active;
        self.tabs.push(new_tab);
        self.active_tab_id = id;
        self.record_visit(&target_url);
        id
    }

    pub fn open_internal(&mut self, url: &str) {
        if let Some(id) = self
            .tabs
            .iter()
            .find(|tab| tab.url == url)
            .map(|tab| tab.id)
        {
            self.switch_tab(id);
        } else {
            self.add_tab(Some(url.into()), false);
        }
    }

    pub fn close_tab(&mut self, tab_id: usize) -> Option<usize> {
        let idx = self.tabs.iter().position(|t| t.id == tab_id)?;
        let removed = self.tabs.remove(idx);
        if removed.url == "veluno://newtab" {
            self.welcome_dismissed = true;
            self.homepage = "veluno://blank".into();
        }
        self.closed_tabs.push(removed);
        if self.closed_tabs.len() > 30 {
            self.closed_tabs.remove(0);
        }
        self.can_restore_closed_tab = true;

        if self.tabs.is_empty() {
            return None;
        }

        if self.active_tab_id == tab_id {
            match self
                .tabs
                .iter()
                .rfind(|t| t.workspace_id == self.active_workspace_id)
            {
                Some(tab) => self.active_tab_id = tab.id,
                None => return Some(self.add_tab(None, false)),
            }
        }

        self.switch_tab(self.active_tab_id);
        Some(self.active_tab_id)
    }

    pub fn discard_download_tab(&mut self, id: usize) -> bool {
        let Some(index) = self
            .tabs
            .iter()
            .position(|tab| tab.id == id && tab.document_url.is_none() && !tab.is_pinned)
        else {
            return false;
        };
        let removed = self.tabs.remove(index);
        if self.active_tab_id == id {
            let replacement = removed
                .opener
                .filter(|opener| {
                    self.tabs
                        .iter()
                        .any(|tab| tab.id == *opener && tab.workspace_id == removed.workspace_id)
                })
                .or_else(|| {
                    self.tabs
                        .iter()
                        .rfind(|tab| tab.workspace_id == removed.workspace_id)
                        .map(|tab| tab.id)
                });
            if let Some(replacement) = replacement {
                self.switch_tab(replacement);
            } else {
                self.active_workspace_id = removed.workspace_id;
                self.add_tab(None, false);
            }
        }
        true
    }

    pub fn restore_closed_tab(&mut self) -> Option<usize> {
        if let Some(mut tab) = self.closed_tabs.pop() {
            tab.id = self.next_tab_id;
            self.next_tab_id += 1;
            tab.is_sleeping = false;
            tab.is_loading = false;
            tab.can_go_back = false;
            tab.can_go_forward = false;
            tab.error = None;
            tab.pending_url = None;
            tab.has_capture = false;
            tab.is_playing_audio = false;
            tab.opener = None;
            tab.document_url = None;
            let id = tab.id;
            self.active_workspace_id = tab.workspace_id;
            self.tabs.push(tab);
            self.active_tab_id = id;
            self.switch_tab(id);
            self.can_restore_closed_tab = !self.closed_tabs.is_empty();
            Some(id)
        } else {
            None
        }
    }

    pub fn switch_tab(&mut self, tab_id: usize) -> Option<&Tab> {
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
            self.activity_clock = self.activity_clock.saturating_add(1);
            t.last_active = self.activity_clock;
            t.is_sleeping = false;
            self.active_tab_id = tab_id;
            self.active_workspace_id = t.workspace_id;
        }
        self.active_tab()
    }

    pub fn sleep_candidates(
        &self,
        live: &std::collections::HashSet<usize>,
        idle: &std::collections::HashSet<usize>,
        downloads: &std::collections::HashMap<usize, usize>,
        budget: usize,
    ) -> Vec<&Tab> {
        let over_budget = live.len() > budget;
        let mut candidates: Vec<_> = self
            .tabs
            .iter()
            .filter(|tab| {
                live.contains(&tab.id)
                    && (over_budget || idle.contains(&tab.id))
                    && tab.id != self.active_tab_id
                    && Some(tab.id) != self.split_partner()
                    && !tab.is_pinned
                    && !tab.is_sleeping
                    && !tab.is_loading
                    && !tab.has_capture
                    && downloads.get(&tab.id).copied().unwrap_or(0) == 0
            })
            .collect();
        candidates.sort_by_key(|tab| tab.last_active);
        candidates
    }

    pub fn create_profile(&mut self, name: &str) -> Result<usize, String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 40 || name.chars().any(char::is_control) {
            return Err("Use a workspace name between 1 and 40 characters.".into());
        }
        if self
            .workspaces
            .iter()
            .any(|w| w.name.to_lowercase() == name.to_lowercase())
        {
            return Err("A workspace with that name already exists.".into());
        }
        let id = self
            .workspaces
            .iter()
            .map(|w| w.id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .filter(|id| *id < usize::MAX - 1)
            .ok_or("Could not create another workspace.")?;
        self.workspaces.push(Workspace {
            id,
            name: name.into(),
            icon: "globe".into(),
            accent: "#dedee3".into(),
            emoji: None,
            private: false,
        });
        self.switch_workspace(id);
        Ok(id)
    }

    pub fn rename_workspace(&mut self, id: usize, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 40 || name.chars().any(char::is_control) {
            return Err("Use a workspace name between 1 and 40 characters.".into());
        }
        if self
            .workspaces
            .iter()
            .any(|w| w.id != id && w.name.to_lowercase() == name.to_lowercase())
        {
            return Err("A workspace with that name already exists.".into());
        }
        let workspace = self
            .workspaces
            .iter_mut()
            .find(|w| w.id == id && !w.private)
            .ok_or("This workspace cannot be renamed.")?;
        workspace.name = name.into();
        Ok(())
    }

    /// Sets or clears (with an empty string) a regular workspace's emoji.
    pub fn set_workspace_emoji(&mut self, id: usize, emoji: &str) -> Result<(), String> {
        let emoji = emoji.trim();
        if emoji.chars().count() > 8 || emoji.chars().any(|c| c.is_ascii() || c.is_control()) {
            return Err("Choose a single emoji for the workspace icon.".into());
        }
        let workspace = self
            .workspaces
            .iter_mut()
            .find(|w| w.id == id && !w.private)
            .ok_or("This workspace's icon cannot be changed.")?;
        workspace.emoji = (!emoji.is_empty()).then(|| emoji.into());
        Ok(())
    }

    pub fn delete_workspace(&mut self, id: usize, destination: usize) -> Result<(), String> {
        if id == destination
            || !self.workspaces.iter().any(|w| w.id == id && !w.private)
            || !self
                .workspaces
                .iter()
                .any(|w| w.id == destination && !w.private)
        {
            return Err("Choose another regular workspace to receive the tabs.".into());
        }
        for tab in self.tabs.iter_mut().chain(self.closed_tabs.iter_mut()) {
            if tab.workspace_id == id {
                tab.workspace_id = destination;
            }
        }
        self.workspaces.retain(|w| w.id != id);
        if self.active_workspace_id == id {
            self.active_workspace_id = destination;
        }
        Ok(())
    }

    pub fn reorder_tab(&mut self, id: usize, target: usize, after: bool) {
        let Some(from) = self.tabs.iter().position(|t| t.id == id) else {
            return;
        };
        let Some(to) = self.tabs.iter().position(|t| t.id == target) else {
            return;
        };
        if from == to
            || self.tabs[from].workspace_id != self.tabs[to].workspace_id
            || self.tabs[from].is_pinned != self.tabs[to].is_pinned
        {
            return;
        }
        let tab = self.tabs.remove(from);
        let to = self.tabs.iter().position(|t| t.id == target).unwrap();
        self.tabs.insert(to + usize::from(after), tab);
    }

    pub fn private_workspace(&self) -> Option<usize> {
        self.workspaces.iter().find(|w| w.private).map(|w| w.id)
    }

    pub fn is_private(&self, tab_id: usize) -> bool {
        self.tabs
            .iter()
            .any(|tab| tab.id == tab_id && Some(tab.workspace_id) == self.private_workspace())
    }

    /// Opens a tab in the Private profile, creating that profile on first use.
    pub fn open_private_tab(&mut self) -> usize {
        let id = self.private_workspace().unwrap_or_else(|| {
            let id = self.workspaces.iter().map(|w| w.id).max().unwrap_or(0) + 1;
            self.workspaces.push(Workspace {
                id,
                name: "Private".into(),
                icon: "private".into(),
                accent: "#dedee3".into(),
                emoji: None,
                private: true,
            });
            id
        });
        self.active_workspace_id = id;
        self.add_tab(None, false)
    }

    /// The state to write to disk: everything except the Private profile.
    pub fn for_session(&self) -> std::borrow::Cow<'_, Self> {
        let private = self.private_workspace();
        if private.is_none()
            && self.download_items.is_empty()
            && !self.downloads_open
            && !self.tabs.is_empty()
        {
            return std::borrow::Cow::Borrowed(self);
        }
        let mut state = self.clone();
        state.download_items.clear();
        state.downloads_open = false;
        if let Some(private) = private {
            state.workspaces.retain(|w| w.id != private);
            state.tabs.retain(|tab| tab.workspace_id != private);
            state.closed_tabs.retain(|tab| tab.workspace_id != private);
        }
        if state.tabs.is_empty() {
            state.active_workspace_id = state.workspaces.first().map_or(1, |w| w.id);
            state.add_tab(None, false);
        }
        std::borrow::Cow::Owned(state)
    }

    pub fn is_bookmarked(&self, url: &str) -> bool {
        self.bookmarks.iter().any(|bookmark| bookmark.url == url)
    }

    /// Adds or removes a bookmark; without a URL it toggles the active page.
    pub fn toggle_bookmark(&mut self, url: Option<String>) {
        if let Some(url) = url {
            self.bookmarks.retain(|bookmark| bookmark.url != url);
            return;
        }
        let Some(tab) = self.active_tab() else {
            return;
        };
        if crate::profile::is_internal(&tab.url) || !crate::profile::allowed_url(&tab.url) {
            return;
        }
        let url = tab.url.clone();
        if self.is_bookmarked(&url) {
            self.bookmarks.retain(|bookmark| bookmark.url != url);
        } else {
            let title = tab.title.clone();
            self.bookmarks.push(Bookmark { url, title });
        }
    }

    /// Appends bookmarks that aren't saved yet and returns how many were added.
    pub fn import_bookmarks(&mut self, bookmarks: Vec<Bookmark>) -> usize {
        let before = self.bookmarks.len();
        for bookmark in bookmarks {
            if !self.is_bookmarked(&bookmark.url) {
                self.bookmarks.push(bookmark);
            }
        }
        self.bookmarks.len() - before
    }

    pub fn switch_workspace(&mut self, workspace_id: usize) -> Option<&Tab> {
        if !self
            .workspaces
            .iter()
            .any(|workspace| workspace.id == workspace_id)
        {
            return self.active_tab();
        }
        self.active_workspace_id = workspace_id;
        let is_current_in_workspace = self
            .active_tab()
            .map(|t| t.workspace_id == workspace_id)
            .unwrap_or(false);

        if is_current_in_workspace {
            return self.active_tab();
        }

        let existing = self
            .tabs
            .iter()
            .filter(|t| t.workspace_id == workspace_id)
            .max_by_key(|t| t.last_active)
            .map(|t| t.id);

        let target_id = if let Some(id) = existing {
            id
        } else {
            self.add_tab(None, false)
        };
        self.switch_tab(target_id)
    }

    pub fn toggle_sidebar(&mut self) -> bool {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        self.sidebar_peek = false;
        self.sidebar_collapsed
    }

    pub fn toggle_sleep(&mut self, tab_id: usize) {
        if tab_id == self.active_tab_id || Some(tab_id) == self.split_partner() {
            return;
        }
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
            tab.is_sleeping = !tab.is_sleeping;
        }
    }

    pub fn sleep_inactive_tabs(&mut self) {
        let partner = self.split_partner();
        for tab in self.tabs.iter_mut() {
            if tab.id != self.active_tab_id && Some(tab.id) != partner {
                tab.is_sleeping = true;
            }
        }
    }

    pub fn toggle_pin(&mut self, tab_id: usize) {
        if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == tab_id) {
            tab.is_pinned = !tab.is_pinned;
        }
    }

    pub fn duplicate_tab(&mut self, tab_id: usize) -> Option<usize> {
        let index = self.tabs.iter().position(|tab| tab.id == tab_id)?;
        let source = self.tabs[index].clone();
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        let copy = Tab {
            id,
            workspace_id: source.workspace_id,
            title: source.title,
            url: source.url.clone(),
            is_pinned: false,
            is_sleeping: false,
            is_loading: false,
            favicon: source.favicon,
            can_go_back: false,
            can_go_forward: false,
            error: None,
            generation: 0,
            pending_url: None,
            last_active: 0,
            has_capture: false,
            is_playing_audio: false,
            is_muted: false,
            opener: Some(tab_id),
            document_url: None,
        };
        self.tabs.insert(index + 1, copy);
        self.switch_tab(id);
        self.record_visit(&source.url);
        Some(id)
    }

    pub fn move_tab_to_workspace(&mut self, tab_id: usize, workspace_id: usize) {
        // A live page keeps its InPrivate or normal storage, so it can't change sides.
        if !self
            .workspaces
            .iter()
            .any(|w| w.id == workspace_id && !w.private)
            || self.is_private(tab_id)
        {
            return;
        }
        let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == tab_id) else {
            return;
        };
        if tab.workspace_id == workspace_id {
            return;
        }
        let old_workspace = tab.workspace_id;
        tab.workspace_id = workspace_id;
        if self.active_tab_id == tab_id {
            if let Some(remaining) = self
                .tabs
                .iter()
                .rev()
                .find(|t| t.workspace_id == old_workspace)
                .map(|t| t.id)
            {
                self.switch_tab(remaining);
            } else {
                self.active_workspace_id = workspace_id;
            }
        }
    }

    pub fn cycle_tab(&mut self, backwards: bool) -> Option<&Tab> {
        let ids: Vec<usize> = self
            .tabs
            .iter()
            .filter(|tab| tab.workspace_id == self.active_workspace_id)
            .map(|tab| tab.id)
            .collect();
        let index = ids.iter().position(|id| *id == self.active_tab_id)?;
        let next = if backwards {
            (index + ids.len() - 1) % ids.len()
        } else {
            (index + 1) % ids.len()
        };
        self.switch_tab(ids[next])
    }

    /// Shows `tab_id` beside the active tab. Both must be awake tabs in the same workspace.
    pub fn split_with(&mut self, tab_id: usize) -> bool {
        let active = self.active_tab_id;
        let same_workspace = self.active_tab().and_then(|a| {
            self.tabs
                .iter()
                .find(|t| t.id == tab_id)
                .map(|t| t.workspace_id == a.workspace_id)
        });
        if tab_id == active || same_workspace != Some(true) {
            return false;
        }
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
            tab.is_sleeping = false;
        }
        self.split = Some([active, tab_id]);
        true
    }

    /// The split pair while one of its tabs is active, dropping a pair that no longer fits.
    pub fn split_pair(&mut self) -> Option<[usize; 2]> {
        let pair = self.split?;
        let tabs: Vec<_> = pair
            .iter()
            .filter_map(|id| self.tabs.iter().find(|t| t.id == *id))
            .collect();
        if tabs.len() != 2
            || tabs[0].workspace_id != tabs[1].workspace_id
            || tabs
                .iter()
                .any(|t| t.is_sleeping && t.id != self.active_tab_id)
        {
            self.split = None;
            return None;
        }
        pair.contains(&self.active_tab_id).then_some(pair)
    }

    /// Docks a regular web tab's address in the side panel and shows the panel.
    pub fn open_in_side_panel(&mut self, tab_id: usize) -> bool {
        let Some(tab) = self.tabs.iter().find(|t| t.id == tab_id) else {
            return false;
        };
        if self.is_private(tab_id)
            || !(tab.url.starts_with("https://") || tab.url.starts_with("http://"))
        {
            return false;
        }
        self.side_panel_url = Some(tab.url.clone());
        self.side_panel_open = true;
        true
    }

    /// The other half of the split while the active tab is in it.
    pub fn split_partner(&self) -> Option<usize> {
        let [a, b] = self.split?;
        match self.active_tab_id {
            id if id == a => Some(b),
            id if id == b => Some(a),
            _ => None,
        }
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == self.active_tab_id)
    }

    pub fn update_active_tab(&mut self, url: String, title: Option<String>) {
        let should_count = self
            .active_tab()
            .map(|tab| title.is_none() && tab.url != url)
            .unwrap_or(false);
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == self.active_tab_id) {
            tab.url = url.clone();
            if let Some(t) = title
                && !t.is_empty()
            {
                tab.title = t;
            }
        }
        if should_count {
            self.record_visit(&url);
        }
    }

    pub fn top_shortcuts(&self) -> Vec<Shortcut> {
        let mut items = self.shortcuts.clone();
        items.sort_by_key(|a| std::cmp::Reverse(a.visits));
        for fallback in Self::default_shortcuts() {
            if items.len() >= 8 {
                break;
            }
            if !items.iter().any(|item| item.host == fallback.host) {
                items.push(fallback);
            }
        }
        items.truncate(8);
        items
    }

    pub fn set_favicon(&mut self, id: usize, favicon: Option<String>) {
        let private = self.is_private(id);
        let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) else {
            return;
        };
        tab.favicon = favicon.clone();
        if private {
            return;
        }
        let (Some(favicon), Some((host, ..))) = (favicon, Self::shortcut_origin(&tab.url)) else {
            return;
        };
        for tab in &mut self.tabs {
            if tab.favicon.is_none()
                && Self::shortcut_origin(&tab.url).is_some_and(|origin| origin.0 == host)
            {
                tab.favicon = Some(favicon.clone());
            }
        }
        if let Some(item) = self.shortcuts.iter_mut().find(|item| item.host == host)
            && item.favicon.as_ref() != Some(&favicon)
        {
            item.favicon = Some(favicon);
            self.save_shortcuts();
        }
    }

    fn record_visit(&mut self, url: &str) {
        if self.private_workspace() == Some(self.active_workspace_id) {
            return;
        }
        let Some((host, root_url, label)) = Self::shortcut_origin(url) else {
            return;
        };
        if let Some(index) = self.shortcuts.iter().position(|item| item.host == host) {
            let mut item = self.shortcuts.remove(index);
            item.visits = item.visits.saturating_add(1);
            item.url = root_url;
            item.title = label;
            self.shortcuts.insert(0, item);
        } else {
            self.shortcuts.insert(
                0,
                Shortcut {
                    host,
                    title: label,
                    url: root_url,
                    visits: 1,
                    favicon: None,
                },
            );
        }
        self.shortcuts.truncate(40);
        self.save_shortcuts();
    }

    fn shortcut_origin(url: &str) -> Option<(String, String, String)> {
        let (scheme, rest) = url.split_once("://")?;
        if !matches!(scheme, "http" | "https") {
            return None;
        }
        let authority = rest.split(['/', '?', '#']).next()?.trim();
        let host_port = authority.rsplit('@').next()?.to_ascii_lowercase();
        if host_port.is_empty() {
            return None;
        }
        let raw_host = host_port.strip_prefix("www.").unwrap_or(&host_port);
        let (host, root_url) = match raw_host {
            "m.youtube.com" => ("youtube.com", "https://youtube.com".to_string()),
            "twitter.com" | "mobile.twitter.com" => ("x.com", "https://x.com".to_string()),
            "old.reddit.com" => ("reddit.com", "https://reddit.com".to_string()),
            "en.wikipedia.org" => ("wikipedia.org", "https://wikipedia.org".to_string()),
            "doc.rust-lang.org" => (
                "doc.rust-lang.org",
                "https://doc.rust-lang.org/book/".to_string(),
            ),
            "search.brave.com" | "duckduckgo.com" => return None,
            _ => (raw_host, format!("{scheme}://{raw_host}")),
        };
        let label = Self::label_for_host(host);
        Some((host.to_string(), root_url, label))
    }

    fn label_for_host(host: &str) -> String {
        match host {
            "youtube.com" | "m.youtube.com" => "YouTube".to_string(),
            "x.com" | "twitter.com" => "X".to_string(),
            "github.com" => "GitHub".to_string(),
            "doc.rust-lang.org" => "Rust Docs".to_string(),
            "crates.io" => "Crates.io".to_string(),
            "news.ycombinator.com" => "Hacker News".to_string(),
            "wikipedia.org" | "en.wikipedia.org" => "Wikipedia".to_string(),
            "reddit.com" | "old.reddit.com" => "Reddit".to_string(),
            _ => {
                let stem = host.split(':').next().unwrap_or(host);
                let first = stem.split('.').next().unwrap_or(stem);
                let mut chars = first.chars();
                match chars.next() {
                    Some(_) if first.len() <= 2 => first.to_ascii_uppercase(),
                    Some(ch) => format!("{}{}", ch.to_ascii_uppercase(), chars.as_str()),
                    None => host.to_string(),
                }
            }
        }
    }

    fn default_shortcuts() -> Vec<Shortcut> {
        [
            ("github.com", "GitHub", "https://github.com"),
            (
                "doc.rust-lang.org",
                "Rust Docs",
                "https://doc.rust-lang.org/book/",
            ),
            ("crates.io", "Crates.io", "https://crates.io"),
            (
                "search.brave.com",
                "Brave Search",
                "https://search.brave.com",
            ),
            ("youtube.com", "YouTube", "https://youtube.com"),
            (
                "news.ycombinator.com",
                "Hacker News",
                "https://news.ycombinator.com",
            ),
            ("wikipedia.org", "Wikipedia", "https://wikipedia.org"),
            ("reddit.com", "Reddit", "https://reddit.com"),
        ]
        .into_iter()
        .map(|(host, title, url)| Shortcut {
            host: host.to_string(),
            title: title.to_string(),
            url: url.to_string(),
            visits: 0,
            favicon: None,
        })
        .collect()
    }

    fn shortcuts_path() -> Option<std::path::PathBuf> {
        if cfg!(test) {
            return None;
        }
        Some(crate::profile::directory().join("shortcuts.json"))
    }

    fn load_shortcuts() -> Vec<Shortcut> {
        let Some(path) = Self::shortcuts_path() else {
            return Vec::new();
        };
        std::fs::read_to_string(path)
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    fn save_shortcuts(&self) {
        let Some(path) = Self::shortcuts_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string(&self.shortcuts) {
            let _ = crate::profile::write_atomic(&path, json.as_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_panel_docks_only_regular_web_pages() {
        let mut state = BrowserState::new();
        let home = state.active_tab_id;
        assert!(!state.open_in_side_panel(home));
        let page = state.add_tab(Some("https://example.com/notes".into()), false);
        assert!(state.open_in_side_panel(page));
        assert_eq!(
            state.side_panel_url.as_deref(),
            Some("https://example.com/notes")
        );
        assert!(state.side_panel_open);
        let private = state.open_private_tab();
        state.update_active_tab("https://example.com/secret".into(), None);
        assert!(!state.open_in_side_panel(private));
        assert_eq!(
            state.side_panel_url.as_deref(),
            Some("https://example.com/notes")
        );
        let saved = serde_json::to_string(&state).unwrap();
        let restored = BrowserState::from_session(&saved).unwrap();
        assert_eq!(
            restored.side_panel_url.as_deref(),
            Some("https://example.com/notes")
        );
    }

    #[test]
    fn workspace_emoji_is_validated_and_clearable() {
        let mut state = BrowserState::new();
        assert!(state.set_workspace_emoji(1, "\u{1F4DA}").is_ok());
        assert_eq!(state.workspaces[0].emoji.as_deref(), Some("\u{1F4DA}"));
        assert!(state.set_workspace_emoji(1, "<b>").is_err());
        assert!(
            state
                .set_workspace_emoji(1, "\u{1F468}\u{200D}\u{1F4BB}")
                .is_ok()
        );
        assert!(state.set_workspace_emoji(1, "").is_ok());
        assert_eq!(state.workspaces[0].emoji, None);
        let private = state.open_private_tab();
        let private = state
            .tabs
            .iter()
            .find(|t| t.id == private)
            .unwrap()
            .workspace_id;
        assert!(state.set_workspace_emoji(private, "\u{1F4DA}").is_err());
    }

    #[test]
    fn split_view_keeps_both_tabs_awake_and_ends_when_a_tab_leaves() {
        let mut state = BrowserState::new();
        let left = state.add_tab(Some("https://example.com/left".into()), false);
        let right = state.add_tab(Some("https://example.com/right".into()), false);
        state.toggle_sleep(left);
        assert!(state.split_with(left));
        assert_eq!(state.split_pair(), Some([right, left]));
        assert!(
            !state
                .tabs
                .iter()
                .find(|t| t.id == left)
                .unwrap()
                .is_sleeping
        );
        state.sleep_inactive_tabs();
        assert!(
            !state
                .tabs
                .iter()
                .find(|t| t.id == left)
                .unwrap()
                .is_sleeping
        );
        state.switch_tab(left);
        assert_eq!(state.split_partner(), Some(right));
        assert!(!state.split_with(left), "a tab cannot split with itself");
        let private = state.open_private_tab();
        state.switch_tab(right);
        assert!(!state.split_with(private), "split panes share a workspace");
        state.close_tab(left);
        assert_eq!(state.split_pair(), None);
        assert_eq!(state.split, None);
    }

    #[test]
    fn download_tab_cleanup_returns_to_opener_without_undo_or_focus_theft() {
        let mut state = BrowserState::new();
        let source = state.add_tab(Some("https://example.com/source".into()), false);
        let other = state.add_tab(Some("https://example.com/other".into()), false);
        state.switch_tab(source);
        let download = state.add_tab(Some("https://example.com/file".into()), false);
        assert!(state.discard_download_tab(download));
        assert_eq!(state.active_tab_id, source);
        assert!(state.closed_tabs.is_empty());
        assert!(!state.can_restore_closed_tab);
        let background = state.add_tab(Some("https://example.com/file".into()), false);
        state.switch_tab(other);
        assert!(state.discard_download_tab(background));
        assert_eq!(state.active_tab_id, other);
        let document = state.add_tab(Some("https://example.com/page".into()), false);
        state
            .tabs
            .iter_mut()
            .find(|tab| tab.id == document)
            .unwrap()
            .document_url = Some("https://example.com/page".into());
        assert!(!state.discard_download_tab(document));
        let pinned = state.add_tab(Some("https://example.com/file".into()), true);
        assert!(!state.discard_download_tab(pinned));
        let private = state.open_private_tab();
        assert!(state.discard_download_tab(private));
        assert!(state.is_private(state.active_tab_id));
        assert!(
            !state
                .for_session()
                .tabs
                .iter()
                .any(|tab| tab.id == download || tab.id == background)
        );
    }

    #[test]
    fn reordering_stays_within_workspace_and_pin_group_and_survives_restore() {
        let mut state = BrowserState::new();
        let a = state.active_tab_id;
        let b = state.add_tab(Some("https://example.com/b".into()), false);
        let c = state.add_tab(Some("https://example.com/c".into()), false);
        state.reorder_tab(a, c, true);
        assert_eq!(
            state.tabs.iter().map(|t| t.id).collect::<Vec<_>>(),
            [b, c, a]
        );
        state.reorder_tab(a, b, false);
        assert_eq!(
            state.tabs.iter().map(|t| t.id).collect::<Vec<_>>(),
            [a, b, c]
        );
        state.toggle_pin(b);
        let before = state.tabs.iter().map(|t| t.id).collect::<Vec<_>>();
        state.reorder_tab(a, b, true);
        state.create_profile("Other").unwrap();
        let other = state.add_tab(Some("https://example.org/".into()), false);
        state.reorder_tab(a, other, true);
        assert_eq!(
            state.tabs.iter().take(3).map(|t| t.id).collect::<Vec<_>>(),
            before
        );
        state.tabs[0].is_muted = true;
        state.tabs[0].is_playing_audio = true;
        let restored = BrowserState::from_session(&serde_json::to_string(&state).unwrap()).unwrap();
        assert_eq!(
            restored.tabs.iter().map(|t| t.id).collect::<Vec<_>>(),
            state.tabs.iter().map(|t| t.id).collect::<Vec<_>>()
        );
        assert!(restored.tabs[0].is_muted);
        assert!(!restored.tabs[0].is_playing_audio);
    }

    #[test]
    fn workspace_deletion_moves_live_and_closed_tabs_without_changing_identity() {
        let mut state = BrowserState::new();
        let original = state.active_workspace_id;
        let destination = state.create_profile("School").unwrap();
        let active = state.add_tab(Some("https://example.com/live".into()), true);
        let closed = state.add_tab(Some("https://example.com/closed".into()), false);
        state.close_tab(closed);
        state.rename_workspace(destination, "Study").unwrap();
        assert!(state.rename_workspace(destination, "General").is_err());
        assert!(state.rename_workspace(destination, "\n").is_err());
        state.delete_workspace(destination, original).unwrap();
        assert_eq!(state.active_workspace_id, original);
        assert_eq!(state.active_tab_id, active);
        assert!(
            state
                .tabs
                .iter()
                .any(|t| t.id == active && t.is_pinned && t.workspace_id == original)
        );
        assert!(
            state
                .closed_tabs
                .iter()
                .any(|t| t.id == closed && t.workspace_id == original)
        );
        assert!(state.delete_workspace(original, original).is_err());
        let private_tab = state.open_private_tab();
        let private = state.private_workspace().unwrap();
        assert!(state.delete_workspace(private, original).is_err());
        assert!(state.rename_workspace(private, "Secret").is_err());
        assert!(state.delete_workspace(original, private).is_err());
        assert!(state.is_private(private_tab));
    }

    #[test]
    fn automatic_sleep_orders_old_tabs_and_protects_active_work() {
        let mut state = BrowserState::new();
        let pinned = state.add_tab(None, true);
        let capture = state.add_tab(None, false);
        state.tabs.last_mut().unwrap().has_capture = true;
        let loading = state.add_tab(None, false);
        state.tabs.last_mut().unwrap().is_loading = true;
        let download = state.add_tab(None, false);
        let recent = state.add_tab(None, false);
        state.switch_tab(1);
        let live = state.tabs.iter().map(|tab| tab.id).collect();
        let none = std::collections::HashSet::new();
        let downloads = [(download, 1)].into_iter().collect();
        let candidates: Vec<_> = state
            .sleep_candidates(&live, &none, &downloads, 1)
            .iter()
            .map(|tab| tab.id)
            .collect();
        assert_eq!(candidates, [recent]);
        assert!(
            !candidates
                .iter()
                .any(|id| [1, pinned, capture, loading, download].contains(id))
        );
        assert!(
            state
                .sleep_candidates(&live, &none, &downloads, 12)
                .is_empty()
        );
        let idle = [pinned, download, recent].into_iter().collect();
        let idle_candidates: Vec<_> = state
            .sleep_candidates(&live, &idle, &downloads, 12)
            .iter()
            .map(|tab| tab.id)
            .collect();
        assert_eq!(
            idle_candidates,
            [recent],
            "idle tabs sleep under budget, guards still apply"
        );
        state.switch_tab(recent);
        assert_eq!(state.sleep_candidates(&live, &none, &downloads, 1)[0].id, 1);
    }

    #[test]
    fn large_sessions_restore_lazily_without_losing_tabs() {
        for count in [100, 300, 500] {
            let mut state = BrowserState::new();
            for index in 1..count {
                state.add_tab(
                    Some(format!("https://example.com/{index}")),
                    index % 25 == 0,
                );
            }
            let restored =
                BrowserState::from_session(&serde_json::to_string(&state).unwrap()).unwrap();
            assert_eq!(restored.tabs.len(), count);
            assert_eq!(
                restored.tabs.iter().filter(|tab| !tab.is_sleeping).count(),
                1
            );
            assert_eq!(restored.active_tab_id, state.active_tab_id);
            assert_eq!(
                restored.tabs.iter().filter(|tab| tab.is_pinned).count(),
                state.tabs.iter().filter(|tab| tab.is_pinned).count()
            );
        }
    }

    #[test]
    fn legacy_preset_spaces_fold_into_general_keeping_tabs() {
        let mut state = BrowserState::new();
        state.create_profile("Dev & Code").unwrap();
        let tab = state.add_tab(Some("https://example.com".into()), false);
        let mut json: serde_json::Value = serde_json::to_value(&state).unwrap();
        json["profiles_version"] = 0.into();
        json["workspaces"] = serde_json::json!([
            {"id":1,"name":"General","icon":"","accent":""},
            {"id":2,"name":"Dev & Code","icon":"","accent":""},
            {"id":3,"name":"Deep Dive","icon":"","accent":""}
        ]);
        let restored = BrowserState::from_session(&json.to_string()).unwrap();
        assert_eq!(restored.workspaces.len(), 1);
        assert!(
            restored
                .tabs
                .iter()
                .any(|t| t.id == tab && t.workspace_id == 1)
        );
    }

    #[test]
    fn legacy_sessions_keep_internal_tabs_closed_tabs_and_bookmarks() {
        let mut state = BrowserState::new();
        state.tabs[0].url = "felaris://newtab".into();
        state.tabs[0].is_pinned = true;
        let settings = state.add_tab(Some("veluno://settings".into()), false);
        state.close_tab(settings);
        state.closed_tabs[0].url = "felaris://settings".into();
        let mut json = serde_json::to_value(&state).unwrap();
        json["bookmarks"] = serde_json::json!([
            {"title": "Saved settings", "url": "felaris://settings"}
        ]);
        let mut restored = BrowserState::from_session(&json.to_string()).unwrap();
        assert_eq!(restored.tabs[0].url, "veluno://newtab");
        assert!(restored.tabs[0].is_pinned);
        assert_eq!(restored.bookmarks[0].url, "veluno://settings");
        assert_eq!(restored.closed_tabs[0].url, "veluno://settings");
        restored.restore_closed_tab().unwrap();
        assert_eq!(restored.active_tab().unwrap().url, "veluno://settings");
    }

    #[test]
    fn saved_session_restores_pins_workspaces_and_unique_ids() {
        let mut state = BrowserState::new();
        state.toggle_pin(1);
        state.create_profile("School").unwrap();
        let active = state.add_tab(Some("https://example.com/draft".into()), false);
        state.is_loading = true;
        state.active_downloads = 2;
        state.update_busy = true;
        let mut restored =
            BrowserState::from_session(&serde_json::to_string(&state).unwrap()).unwrap();
        assert_eq!(restored.active_tab_id, active);
        assert_eq!(restored.active_workspace_id, 2);
        assert!(restored.tabs[0].is_pinned);
        assert!(restored.tabs[0].is_sleeping);
        assert!(!restored.active_tab().unwrap().is_sleeping);
        assert!(!restored.is_loading);
        assert_eq!(restored.active_downloads, 0);
        assert!(!restored.update_busy);
        assert!(restored.add_tab(None, false) > active);
    }

    #[test]
    fn session_rejects_invalid_tabs_and_repairs_active_selection() {
        let mut state = BrowserState::new();
        state.add_tab(Some("javascript:alert(1)".into()), false);
        state.tabs.push(state.tabs[0].clone());
        state.active_tab_id = 999;
        let restored = BrowserState::from_session(&serde_json::to_string(&state).unwrap()).unwrap();
        assert_eq!(restored.tabs.len(), 1);
        assert_eq!(restored.active_tab_id, 1);
        assert_eq!(restored.active_workspace_id, 1);
        assert!(BrowserState::from_session("{truncated").is_none());
    }

    #[test]
    fn session_keeps_update_dismissal_without_download_paths() {
        for private in [false, true] {
            let mut state = BrowserState::new();
            if private {
                state.open_private_tab();
            }
            state.dismissed_update_version = Some("999.0.0".into());
            state.downloads_open = true;
            state.download_items.push(crate::desktop::DownloadInfo {
                id: 1,
                path: "C:\\secret-download.txt".into(),
                received: 10,
                total: 10,
                status: "complete".into(),
            });
            assert!(
                serde_json::to_string(&state)
                    .unwrap()
                    .contains("secret-download")
            );
            let saved = serde_json::to_string(&*state.for_session()).unwrap();
            assert!(!saved.contains("secret-download"));
            let restored = BrowserState::from_session(&saved).unwrap();
            assert_eq!(
                restored.dismissed_update_version.as_deref(),
                Some("999.0.0")
            );
            assert!(restored.download_items.is_empty());
            assert!(!restored.downloads_open);
        }
    }

    #[test]
    fn private_profile_is_never_saved_or_counted() {
        let mut state = BrowserState::new();
        let private = state.open_private_tab();
        state.update_active_tab("https://secret.test/".into(), None);
        state.toggle_bookmark(None);
        assert!(state.is_private(private));
        assert!(state.shortcuts.iter().all(|s| s.host != "secret.test"));
        state.close_tab(1);
        let saved = serde_json::to_string(&*state.for_session()).unwrap();
        assert!(!saved.contains("\"private\":true"));
        let restored = BrowserState::from_session(&saved).unwrap();
        assert_eq!(restored.workspaces.len(), 1);
        assert!(
            restored
                .tabs
                .iter()
                .all(|tab| tab.url != "https://secret.test/")
        );
        assert!(restored.closed_tabs.iter().all(|tab| tab.workspace_id == 1));
        assert_eq!(restored.bookmarks.len(), 1);
        state.move_tab_to_workspace(private, 1);
        assert!(state.is_private(private));
    }

    #[test]
    fn imported_bookmarks_skip_existing_ones() {
        let mut state = BrowserState::new();
        state.bookmarks.push(Bookmark {
            url: "https://a.test/".into(),
            title: "Mine".into(),
        });
        let added = state.import_bookmarks(vec![
            Bookmark {
                url: "https://a.test/".into(),
                title: "Theirs".into(),
            },
            Bookmark {
                url: "https://b.test/".into(),
                title: "B".into(),
            },
        ]);
        assert_eq!(added, 1);
        assert_eq!(state.bookmarks[0].title, "Mine");
        assert_eq!(state.bookmarks[1].url, "https://b.test/");
    }

    #[test]
    fn bookmarks_toggle_real_pages_only() {
        let mut state = BrowserState::new();
        state.toggle_bookmark(None);
        assert!(state.bookmarks.is_empty());
        state.update_active_tab("https://example.com/".into(), Some("Example".into()));
        state.toggle_bookmark(None);
        assert!(state.is_bookmarked("https://example.com/"));
        assert_eq!(state.bookmarks[0].title, "Example");
        state.toggle_bookmark(None);
        assert!(state.bookmarks.is_empty());
        state.toggle_bookmark(None);
        state.toggle_bookmark(Some("https://example.com/".into()));
        assert!(state.bookmarks.is_empty());
    }

    #[test]
    fn active_tab_cannot_be_put_to_sleep() {
        let mut state = BrowserState::new();
        state.toggle_sleep(1);
        assert!(!state.active_tab().unwrap().is_sleeping);
        state.switch_workspace(999);
        assert_eq!(state.active_workspace_id, 1);
    }

    #[test]
    fn closing_active_tab_wakes_its_replacement() {
        let mut state = BrowserState::new();
        let active = state.add_tab(Some("https://example.com".into()), false);
        state.sleep_inactive_tabs();
        state.close_tab(active);
        assert_eq!(state.active_tab_id, 1);
        assert!(!state.active_tab().unwrap().is_sleeping);
    }

    #[test]
    fn restoration_selects_original_workspace_and_wakes_tab() {
        let mut state = BrowserState::new();
        state.create_profile("School").unwrap();
        let id = state.add_tab(Some("https://example.com".into()), false);
        state.close_tab(id);
        state.switch_workspace(1);
        state.restore_closed_tab().unwrap();
        let tab = state.active_tab().unwrap();
        assert_eq!(tab.workspace_id, state.active_workspace_id);
        assert_eq!(state.active_workspace_id, 2);
        assert!(!tab.is_sleeping);
        assert_ne!(tab.id, id);
    }

    #[test]
    fn cycling_wraps_within_workspace_and_wakes_tab() {
        let mut state = BrowserState::new();
        let second = state.add_tab(None, false);
        state.create_profile("School").unwrap();
        state.switch_tab(second);
        state.sleep_inactive_tabs();
        assert_eq!(state.cycle_tab(false).unwrap().id, 1);
        assert!(!state.active_tab().unwrap().is_sleeping);
        assert_eq!(state.cycle_tab(true).unwrap().id, second);
    }

    #[test]
    fn closing_welcome_keeps_existing_tabs_on_restart() {
        let mut state = BrowserState::new();
        let id = state.add_tab(Some("https://example.com".into()), false);
        state.close_tab(1);
        let restored = BrowserState::from_session(&serde_json::to_string(&state).unwrap()).unwrap();
        assert!(restored.welcome_dismissed);
        assert_eq!(restored.tabs.len(), 1);
        assert_eq!(restored.active_tab_id, id);
        assert_eq!(restored.active_tab().unwrap().url, "https://example.com/");
        let mut legacy = serde_json::to_value(BrowserState::new()).unwrap();
        legacy.as_object_mut().unwrap().remove("welcome_dismissed");
        let restored = BrowserState::from_session(&legacy.to_string()).unwrap();
        assert!(!restored.welcome_dismissed);
        assert_eq!(restored.homepage, "veluno://newtab");
    }

    #[test]
    fn closing_last_tab_leaves_tabs_empty_and_allows_restoring_closed_tab() {
        let mut state = BrowserState::new();
        assert_eq!(state.close_tab(1), None);
        assert!(state.tabs.is_empty());
        assert!(state.can_restore_closed_tab);
        assert_eq!(state.close_tab(999), None);
        assert!(state.tabs.is_empty());

        let saved = serde_json::to_string(&*state.for_session()).unwrap();
        let mut restored = BrowserState::from_session(&saved).unwrap();
        assert!(restored.welcome_dismissed);
        assert_eq!(restored.homepage, "veluno://blank");
        assert_eq!(restored.tabs.len(), 1);
        assert_eq!(restored.active_tab().unwrap().url, "veluno://blank");
        assert!(restored.can_restore_closed_tab);

        restored.restore_closed_tab().unwrap();
        assert_eq!(restored.tabs.len(), 2);
    }

    #[test]
    fn closing_tab_with_surviving_tabs_switches_active_tab() {
        let mut state = BrowserState::new();
        let second = state.add_tab(Some("https://example.com".into()), false);
        assert_eq!(state.tabs.len(), 2);
        assert_eq!(state.close_tab(second), Some(1));
        assert_eq!(state.tabs.len(), 1);
        assert_eq!(state.active_tab_id, 1);
    }

    #[test]
    fn shortcuts_rank_frequent_sites_and_keep_eight_tiles() {
        let mut state = BrowserState::new();
        for _ in 0..3 {
            state.add_tab(Some("https://www.youtube.com/watch?v=123".into()), false);
        }
        state.update_active_tab(
            "https://www.youtube.com/watch?v=123".into(),
            Some("YouTube Video".into()),
        );
        state.add_tab(Some("https://x.com/rustlang/status/1".into()), false);
        state.add_tab(Some("https://twitter.com/rustlang".into()), false);

        let shortcuts = state.top_shortcuts();
        assert_eq!(shortcuts.len(), 8);
        assert_eq!(shortcuts[0].host, "youtube.com");
        assert_eq!(shortcuts[0].title, "YouTube");
        assert_eq!(shortcuts[0].url, "https://youtube.com");
        assert_eq!(shortcuts[0].visits, 3);
        assert_eq!(shortcuts[1].host, "x.com");
        assert_eq!(shortcuts[1].title, "X");
        assert_eq!(shortcuts[1].url, "https://x.com");
        assert_eq!(shortcuts[1].visits, 2);
        assert_eq!(
            shortcuts
                .iter()
                .filter(|item| item.host == "youtube.com")
                .count(),
            1
        );
    }

    #[test]
    fn duplicate_and_move_tab_preserve_workspace_invariants() {
        let mut state = BrowserState::new();
        state.update_active_tab("https://example.com".into(), Some("Example".into()));
        state.tabs[0].has_capture = true;
        let copy = state.duplicate_tab(1).unwrap();
        assert_eq!(state.active_tab_id, copy);
        assert_eq!(state.active_tab().unwrap().url, "https://example.com");
        assert_eq!(state.active_tab().unwrap().title, "Example");
        assert!(!state.active_tab().unwrap().has_capture);
        assert!(state.active_tab().unwrap().last_active > state.tabs[0].last_active);

        state.create_profile("School").unwrap();
        state.switch_workspace(1);
        state.move_tab_to_workspace(copy, 2);
        assert_eq!(state.active_workspace_id, 1);
        assert_eq!(state.active_tab_id, 1);
        assert_eq!(
            state
                .tabs
                .iter()
                .find(|t| t.id == copy)
                .unwrap()
                .workspace_id,
            2
        );

        state.create_profile("Personal").unwrap();
        state.switch_workspace(1);
        state.move_tab_to_workspace(1, 3);
        assert_eq!(state.active_workspace_id, 3);
        assert_eq!(state.active_tab_id, 1);
    }
}
