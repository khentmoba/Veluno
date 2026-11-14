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
    /// Whether pinned tabs in this workspace are collapsed under the workspace header.
    #[serde(default)]
    pub pins_collapsed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
    pub id: usize,
    pub workspace_id: usize,
    pub name: String,
    #[serde(default)]
    pub collapsed: bool,
    #[serde(default)]
    pub is_pinned: bool,
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
    /// Page zoom percentage, kept with the tab across sleep and restart.
    #[serde(default = "default_zoom")]
    pub zoom: u32,
    /// Global Zen-style Essential tab shown as an icon tile across regular workspaces.
    #[serde(default)]
    pub is_essential: bool,
    /// Original URL saved when pinned or added to Essentials, for one-click reset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_url: Option<String>,
    /// Optional collapsible sidebar folder containing this tab.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<usize>,
    /// User-defined tab label (double-click to rename, just like Zen).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_title: Option<String>,
    #[serde(skip)]
    pub opener: Option<usize>,
    #[serde(skip)]
    pub document_url: Option<String>,
}

fn default_zoom() -> u32 {
    100
}

fn default_sidebar_width() -> f64 {
    260.0
}

fn default_true() -> bool {
    true
}

pub const MAX_ESSENTIALS: usize = 12;

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
    pub sidebar_rail: bool,
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: f64,
    #[serde(default = "default_true")]
    pub workspace_themes: bool,
    #[serde(default = "default_true")]
    pub essentials_glow: bool,
    #[serde(default)]
    pub sidebar_peek: bool,
    #[serde(skip)]
    pub topbar_peek: bool,
    #[serde(default)]
    pub folders: Vec<Folder>,
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
            pins_collapsed: false,
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
            zoom: 100,
            is_essential: false,
            pinned_url: None,
            folder_id: None,
            custom_title: None,
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
            sidebar_rail: false,
            sidebar_width: 260.0,
            workspace_themes: true,
            essentials_glow: true,
            sidebar_peek: false,
            topbar_peek: false,
            folders: Vec::new(),
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
        for candidate in [
            path.clone(),
            path.with_extension("json.bak"),
            path.with_extension("json.bak2"),
        ] {
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
        let mut folder_ids = std::collections::HashSet::new();
        state.folders.retain(|folder| {
            folder.id > 0
                && workspace_ids.contains(&folder.workspace_id)
                && !folder.name.trim().is_empty()
                && folder_ids.insert(folder.id)
        });
        let mut essential_count = 0usize;
        for tab in &mut state.tabs {
            tab.is_loading = false;
            tab.can_go_back = false;
            tab.can_go_forward = false;
            tab.error = None;
            tab.is_sleeping = tab.id != state.active_tab_id;
            if tab.is_essential {
                if essential_count < MAX_ESSENTIALS {
                    essential_count += 1;
                    tab.folder_id = None;
                    if tab.pinned_url.is_none() {
                        tab.pinned_url = Some(tab.url.clone());
                    }
                } else {
                    tab.is_essential = false;
                }
            }
            if tab.is_pinned && tab.pinned_url.is_none() {
                tab.pinned_url = Some(tab.url.clone());
            }
            if let Some(fid) = tab.folder_id
                && !state
                    .folders
                    .iter()
                    .any(|f| f.id == fid && f.workspace_id == tab.workspace_id)
            {
                tab.folder_id = None;
            }
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
        state.zoom = state.active_tab().map_or(100, |tab| tab.zoom);
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

    /// The next free tab id. Never overflows and never reuses a live tab's id.
    fn allocate_tab_id(&mut self) -> usize {
        if self.next_tab_id < usize::MAX - 1
            && !self.tabs.iter().any(|tab| tab.id == self.next_tab_id)
        {
            let id = self.next_tab_id;
            self.next_tab_id += 1;
            return id;
        }
        let id = (1..=usize::MAX - 2)
            .find(|id| !self.tabs.iter().any(|tab| tab.id == *id))
            .unwrap_or(self.active_tab_id);
        self.next_tab_id = id.saturating_add(1);
        id
    }

    pub fn add_tab(&mut self, url: Option<String>, is_pinned: bool) -> usize {
        let id = self.allocate_tab_id();
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
            zoom: 100,
            is_essential: false,
            pinned_url: None,
            folder_id: None,
            custom_title: None,
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

    /// Closes every unpinned, non-essential tab in this workspace except `tab_id`; returns how many closed.
    pub fn close_other_tabs(&mut self, tab_id: usize) -> usize {
        let Some(workspace) = self
            .tabs
            .iter()
            .find(|tab| tab.id == tab_id)
            .map(|tab| tab.workspace_id)
        else {
            return 0;
        };
        let ids: Vec<_> = self
            .tabs
            .iter()
            .filter(|tab| {
                tab.id != tab_id
                    && tab.workspace_id == workspace
                    && !tab.is_pinned
                    && !tab.is_essential
            })
            .map(|tab| tab.id)
            .collect();
        for id in &ids {
            self.close_tab(*id);
        }
        self.switch_tab(tab_id);
        ids.len()
    }

    /// Closes the unpinned tabs after `tab_id` in its own pin group.
    pub fn close_tabs_below(&mut self, tab_id: usize) -> usize {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == tab_id) else {
            return 0;
        };
        let workspace = self.tabs[index].workspace_id;
        let pinned = self.tabs[index].is_pinned;
        let ids: Vec<_> = self
            .tabs
            .iter()
            .skip(index + 1)
            .filter(|tab| {
                tab.workspace_id == workspace && tab.is_pinned == pinned && !tab.is_essential
            })
            .map(|tab| tab.id)
            .collect();
        for id in &ids {
            self.close_tab(*id);
        }
        self.switch_tab(tab_id);
        ids.len()
    }

    /// Restores a closed tab by recency: 0 is the tab closed most recently.
    pub fn restore_closed_at(&mut self, index: usize) -> Option<usize> {
        let position = self.closed_tabs.len().checked_sub(index + 1)?;
        let tab = self.closed_tabs.remove(position);
        self.closed_tabs.push(tab);
        self.restore_closed_tab()
    }

    pub fn restore_closed_tab(&mut self) -> Option<usize> {
        if let Some(mut tab) = self.closed_tabs.pop() {
            tab.id = self.allocate_tab_id();
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
        let current_is_private = self.private_workspace() == Some(self.active_workspace_id);
        let active_workspace_id = self.active_workspace_id;
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
            self.activity_clock = self.activity_clock.saturating_add(1);
            t.last_active = self.activity_clock;
            t.is_sleeping = false;
            self.active_tab_id = tab_id;
            if t.is_essential && !current_is_private {
                // Global Essentials follow the user across regular workspaces without ripping them out of context.
                t.workspace_id = active_workspace_id;
            } else {
                self.active_workspace_id = t.workspace_id;
            }
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
                    && !tab.is_essential
                    && !tab.is_sleeping
                    && !tab.is_loading
                    && !tab.has_capture
                    && downloads.get(&tab.id).copied().unwrap_or(0) == 0
            })
            .collect();
        candidates.sort_by_key(|tab| tab.last_active);
        candidates
    }

    /// The lowest unused workspace id, so ids stay small and can never overflow.
    fn next_workspace_id(&self) -> Option<usize> {
        (1..=usize::MAX - 2).find(|id| !self.workspaces.iter().any(|w| w.id == *id))
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
            .next_workspace_id()
            .ok_or("Could not create another workspace.")?;
        self.workspaces.push(Workspace {
            id,
            name: name.into(),
            icon: "globe".into(),
            accent: "#dedee3".into(),
            emoji: None,
            private: false,
            pins_collapsed: false,
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
        for folder in &mut self.folders {
            if folder.workspace_id == id {
                folder.workspace_id = destination;
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
        let same_essential = self.tabs[from].is_essential && self.tabs[to].is_essential;
        let same_group = !self.tabs[from].is_essential
            && !self.tabs[to].is_essential
            && self.tabs[from].workspace_id == self.tabs[to].workspace_id
            && self.tabs[from].is_pinned == self.tabs[to].is_pinned;
        if from == to || (!same_essential && !same_group) {
            return;
        }
        let target_folder = self.tabs[to].folder_id;
        let mut tab = self.tabs.remove(from);
        if same_group {
            tab.folder_id = target_folder;
        }
        // The target stays present, but never trust an index that could have moved.
        match self.tabs.iter().position(|t| t.id == target) {
            Some(to) => self.tabs.insert(to + usize::from(after), tab),
            None => self.tabs.insert(from.min(self.tabs.len()), tab),
        }
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
        let id = match self.private_workspace() {
            Some(id) => id,
            None => {
                let Some(id) = self.next_workspace_id() else {
                    return self.active_tab_id;
                };
                self.workspaces.push(Workspace {
                    id,
                    name: "Private".into(),
                    icon: "private".into(),
                    accent: "#dedee3".into(),
                    emoji: None,
                    private: true,
                    pins_collapsed: false,
                });
                id
            }
        };
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
            state
                .folders
                .retain(|folder| folder.workspace_id != private);
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

    pub fn toggle_rail(&mut self) -> bool {
        self.sidebar_rail = !self.sidebar_rail;
        if self.sidebar_rail {
            self.sidebar_collapsed = false;
            self.sidebar_peek = false;
        }
        self.sidebar_rail
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
            if tab.id != self.active_tab_id
                && Some(tab.id) != partner
                && !tab.is_pinned
                && !tab.is_essential
            {
                tab.is_sleeping = true;
            }
        }
    }

    pub fn toggle_pin(&mut self, tab_id: usize) {
        if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == tab_id) {
            tab.is_pinned = !tab.is_pinned;
            if tab.is_pinned {
                tab.is_essential = false;
                tab.pinned_url = Some(tab.url.clone());
            } else {
                tab.pinned_url = None;
            }
        }
    }

    pub fn toggle_essential(&mut self, tab_id: usize) -> Result<bool, String> {
        if self.is_private(tab_id) {
            return Err("Private tabs cannot be added to Essentials.".into());
        }
        let currently_essential = self
            .tabs
            .iter()
            .find(|t| t.id == tab_id)
            .is_some_and(|t| t.is_essential);
        if !currently_essential
            && self.tabs.iter().filter(|t| t.is_essential).count() >= MAX_ESSENTIALS
        {
            return Err(format!("You can keep up to {MAX_ESSENTIALS} Essentials."));
        }
        let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) else {
            return Err("Tab not found.".into());
        };
        tab.is_essential = !tab.is_essential;
        if tab.is_essential {
            tab.is_pinned = false;
            tab.folder_id = None;
            if tab.pinned_url.is_none() {
                tab.pinned_url = Some(tab.url.clone());
            }
        } else if !tab.is_pinned {
            tab.pinned_url = None;
        }
        Ok(tab.is_essential)
    }

    /// Unloads an Essential or Pinned tab (resets to its pinned URL and sleeps it, switching away if active).
    pub fn unload_tab(&mut self, tab_id: usize) -> bool {
        let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) else {
            return false;
        };
        if let Some(pinned) = tab.pinned_url.clone() {
            tab.url = pinned;
        }
        tab.is_loading = false;
        tab.pending_url = None;
        tab.is_playing_audio = false;
        if tab_id == self.active_tab_id {
            let replacement = self
                .tabs
                .iter()
                .rev()
                .find(|t| {
                    t.id != tab_id && t.workspace_id == self.active_workspace_id && !t.is_sleeping
                })
                .or_else(|| {
                    self.tabs
                        .iter()
                        .rev()
                        .find(|t| t.id != tab_id && t.workspace_id == self.active_workspace_id)
                })
                .map(|t| t.id);
            if let Some(next_id) = replacement {
                self.switch_tab(next_id);
            } else {
                self.add_tab(None, false);
            }
        }
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
            tab.is_sleeping = true;
        }
        true
    }

    /// Resets a pinned or essential tab back to its saved `pinned_url`, returning the URL to navigate to.
    pub fn reset_pinned_url(&mut self, tab_id: usize) -> Option<String> {
        let tab = self.tabs.iter_mut().find(|t| t.id == tab_id)?;
        let target = tab.pinned_url.clone()?;
        tab.url.clone_from(&target);
        tab.pending_url = Some(target.clone());
        tab.error = None;
        Some(target)
    }

    pub fn rename_tab(&mut self, tab_id: usize, title: &str) {
        let trimmed = title.trim();
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
            tab.custom_title = (!trimmed.is_empty()).then(|| trimmed.chars().take(80).collect());
        }
    }

    fn next_folder_id(&self) -> usize {
        (1..=usize::MAX - 2)
            .find(|id| !self.folders.iter().any(|f| f.id == *id))
            .unwrap_or(1)
    }

    pub fn create_folder(
        &mut self,
        name: &str,
        tab_id: Option<usize>,
        is_pinned: bool,
    ) -> Result<usize, String> {
        if self.private_workspace() == Some(self.active_workspace_id) {
            return Err("Private mode does not save folders.".into());
        }
        let trimmed = name.trim();
        let folder_name: String = if trimmed.is_empty() {
            "New Folder".into()
        } else {
            trimmed.chars().take(40).collect()
        };
        let workspace_id = tab_id
            .and_then(|id| self.tabs.iter().find(|t| t.id == id))
            .map_or(self.active_workspace_id, |t| t.workspace_id);
        let pinned = tab_id
            .and_then(|id| self.tabs.iter().find(|t| t.id == id))
            .map_or(is_pinned, |t| t.is_pinned);
        let id = self.next_folder_id();
        self.folders.push(Folder {
            id,
            workspace_id,
            name: folder_name,
            collapsed: false,
            is_pinned: pinned,
        });
        if let Some(tid) = tab_id {
            self.move_tab_to_folder(tid, Some(id));
        }
        Ok(id)
    }

    pub fn rename_folder(&mut self, id: usize, name: &str) -> Result<(), String> {
        let trimmed = name.trim();
        if trimmed.is_empty()
            || trimmed.chars().count() > 40
            || trimmed.chars().any(char::is_control)
        {
            return Err("Use a folder name between 1 and 40 characters.".into());
        }
        let folder = self
            .folders
            .iter_mut()
            .find(|f| f.id == id)
            .ok_or("Folder not found.")?;
        folder.name = trimmed.into();
        Ok(())
    }

    pub fn toggle_folder(&mut self, id: usize) {
        if let Some(folder) = self.folders.iter_mut().find(|f| f.id == id) {
            folder.collapsed = !folder.collapsed;
        }
    }

    pub fn delete_folder(&mut self, id: usize) {
        self.folders.retain(|f| f.id != id);
        for tab in &mut self.tabs {
            if tab.folder_id == Some(id) {
                tab.folder_id = None;
            }
        }
    }

    pub fn move_tab_to_folder(&mut self, tab_id: usize, folder_id: Option<usize>) {
        if let Some(fid) = folder_id {
            let Some(folder) = self.folders.iter().find(|f| f.id == fid).cloned() else {
                return;
            };
            if let Some(tab) = self
                .tabs
                .iter_mut()
                .find(|t| t.id == tab_id && !t.is_essential)
            {
                tab.workspace_id = folder.workspace_id;
                tab.is_pinned = folder.is_pinned;
                if tab.is_pinned && tab.pinned_url.is_none() {
                    tab.pinned_url = Some(tab.url.clone());
                }
                tab.folder_id = Some(fid);
            }
        } else if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
            tab.folder_id = None;
        }
    }

    pub fn reorder_workspace(&mut self, id: usize, index: usize) {
        let Some(from) = self
            .workspaces
            .iter()
            .position(|w| w.id == id && !w.private)
        else {
            return;
        };
        let regular_len = self.workspaces.iter().filter(|w| !w.private).count();
        let target = index.min(regular_len.saturating_sub(1));
        if from == target {
            return;
        }
        let space = self.workspaces.remove(from);
        self.workspaces.insert(target, space);
    }

    pub fn set_workspace_accent(&mut self, id: usize, accent: &str) -> Result<(), String> {
        let valid = accent.starts_with('#')
            && (accent.len() == 7 || accent.len() == 4)
            && accent[1..].chars().all(|c| c.is_ascii_hexdigit());
        if !valid {
            return Err("Invalid workspace color.".into());
        }
        let workspace = self
            .workspaces
            .iter_mut()
            .find(|w| w.id == id && !w.private)
            .ok_or("This workspace's theme cannot be changed.")?;
        workspace.accent = accent.to_string();
        Ok(())
    }

    pub fn toggle_workspace_pins(&mut self, id: usize) {
        if let Some(w) = self.workspaces.iter_mut().find(|w| w.id == id) {
            w.pins_collapsed = !w.pins_collapsed;
        }
    }

    pub fn cycle_workspace(&mut self, forward: bool) -> Option<&Tab> {
        let ids: Vec<usize> = self.workspaces.iter().map(|w| w.id).collect();
        if ids.len() <= 1 {
            return self.active_tab();
        }
        let index = ids
            .iter()
            .position(|id| *id == self.active_workspace_id)
            .unwrap_or(0);
        let next = if forward {
            (index + 1) % ids.len()
        } else {
            (index + ids.len() - 1) % ids.len()
        };
        self.switch_workspace(ids[next])
    }

    pub fn swap_split(&mut self) -> bool {
        if let Some([a, b]) = self.split {
            self.split = Some([b, a]);
            true
        } else {
            false
        }
    }

    pub fn duplicate_tab(&mut self, tab_id: usize) -> Option<usize> {
        let index = self.tabs.iter().position(|tab| tab.id == tab_id)?;
        let source = self.tabs[index].clone();
        let id = self.allocate_tab_id();
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
            zoom: source.zoom,
            is_essential: false,
            pinned_url: None,
            folder_id: source.folder_id,
            custom_title: None,
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
        tab.folder_id = None;
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

    /// Tab ids in the order the sidebar shows them: Essentials, then pinned and regular groups,
    /// each with its folders first and loose tabs after.
    pub fn sidebar_order(&self) -> Vec<usize> {
        let workspace = self.active_workspace_id;
        let private = self.private_workspace() == Some(workspace);
        let mut ids: Vec<usize> = if private {
            Vec::new()
        } else {
            self.tabs
                .iter()
                .filter(|tab| tab.is_essential)
                .map(|tab| tab.id)
                .collect()
        };
        let tabs: Vec<&Tab> = self
            .tabs
            .iter()
            .filter(|tab| !tab.is_essential && tab.workspace_id == workspace)
            .collect();
        for pinned in [true, false] {
            let group: Vec<&&Tab> = tabs.iter().filter(|tab| tab.is_pinned == pinned).collect();
            let folders: Vec<usize> = self
                .folders
                .iter()
                .filter(|folder| folder.workspace_id == workspace && folder.is_pinned == pinned)
                .map(|folder| folder.id)
                .collect();
            for folder in &folders {
                ids.extend(
                    group
                        .iter()
                        .filter(|tab| tab.folder_id == Some(*folder))
                        .map(|tab| tab.id),
                );
            }
            ids.extend(
                group
                    .iter()
                    .filter(|tab| {
                        !tab.folder_id
                            .is_some_and(|folder| folders.contains(&folder))
                    })
                    .map(|tab| tab.id),
            );
        }
        ids
    }

    /// Ctrl+1 to Ctrl+8 pick that tab in sidebar order; Ctrl+9 picks the last one.
    pub fn select_tab_position(&mut self, position: usize) -> Option<&Tab> {
        let ids = self.sidebar_order();
        let id = match position {
            9 => ids.last().copied(),
            1..=8 => ids.get(position - 1).copied(),
            _ => None,
        }?;
        self.switch_tab(id)
    }

    pub fn cycle_tab(&mut self, backwards: bool) -> Option<&Tab> {
        let ids = self.sidebar_order();
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
            .is_some_and(|tab| title.is_none() && tab.url != url);
        if should_count {
            self.record_visit(&url);
        }
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == self.active_tab_id) {
            tab.url = url;
            if let Some(t) = title
                && !t.is_empty()
            {
                tab.title = t;
            }
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
        tab.favicon.clone_from(&favicon);
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
            "drive.google.com" => (
                "drive.google.com",
                "https://drive.google.com/drive".to_string(),
            ),
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
            "drive.google.com" => "Google Drive".to_string(),
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
mod tests;
