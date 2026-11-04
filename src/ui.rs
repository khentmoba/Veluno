use crate::{
    extensions, history, settings,
    state::{Bookmark, Shortcut},
    updater,
};

pub const SIDEBAR_WIDTH_EXPANDED: f64 = 260.0;
pub const SIDEBAR_HOT_ZONE: f64 = 6.0;
pub const WINDOW_MARGIN: f64 = 6.0;
pub const CONTENT_RADIUS: f64 = 10.0;
pub const TOPBAR_HOT_ZONE: f64 = 4.0;
pub const TOPBAR_EXPANDED: f64 = 34.0;
pub const POPUP_BAR_HEIGHT: f64 = 42.0;
pub const SIDE_PANEL_WIDTH: f64 = 380.0;

/// The shared page script, using the search engine chosen in Settings.
pub fn script() -> String {
    let (name, url) = settings::engine();
    include_str!("browser.js")
        .replace("'https://search.brave.com/search?q='", &format!("'{url}'"))
        .replace("'Brave Search'", &format!("'{name}'"))
}

pub fn search_script() -> String {
    let (name, url) = settings::engine();
    format!("Veluno.searchUrl='{url}';Veluno.searchName='{name}'")
}

/// Applies the Settings accent to Veluno's own pages. Never used for websites.
pub fn themed(html: &str) -> String {
    html.replace("#dedee3", settings::accent())
}

/// Updates the accent of an already loaded Veluno page.
pub fn accent_script() -> String {
    format!(
        "document.documentElement.style.setProperty('--accent','{0}');document.documentElement.style.setProperty('--focus','{0}')",
        settings::accent()
    )
}

pub fn get_ui_html() -> String {
    let body = if settings::sidebar_right() {
        "<body class=\"right\">"
    } else {
        "<body>"
    };
    themed(include_str!("ui.html"))
        .replacen("<body>", body, 1)
        .replace("__COMMON_SCRIPT__", &script())
}

pub fn get_new_tab_html() -> String {
    themed(include_str!("new_tab.html")).replace("__COMMON_SCRIPT__", &script())
}

pub fn get_history_html(bookmarks: &[Bookmark], shortcuts: &[Shortcut]) -> String {
    // One placeholder, filled last, so page titles can't collide with another one.
    let data = serde_json::json!({
        "history": history::all(),
        "bookmarks": bookmarks,
        "saving": settings::get().save_history,
        "shortcuts": shortcuts,
    });
    themed(include_str!("history.html"))
        .replace("__COMMON_SCRIPT__", &script())
        .replace("__HISTORY__", &data.to_string().replace('<', "\\u003c"))
}

pub const SETTINGS_SECTIONS: [&str; 6] = [
    "general",
    "search",
    "privacy",
    "extensions",
    "shortcuts",
    "about",
];

thread_local! {
    static SETTINGS_SECTION: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Makes the next Settings page open at this section instead of General.
pub fn open_settings_at(section: String) {
    SETTINGS_SECTION.with(|pending| *pending.borrow_mut() = Some(section));
}

pub fn get_settings_html() -> String {
    let section = SETTINGS_SECTION
        .with(|pending| pending.borrow_mut().take())
        .unwrap_or_else(|| "general".into());
    let data = serde_json::json!({
        "section": section,
        "importers": crate::import::available(),
        "settings": settings::get(),
        "accents": settings::ACCENTS,
        "engines": settings::ENGINES.iter().map(|(id, name, _)| [id, name]).collect::<Vec<_>>(),
        "ublock": extensions::status(),
        "ublock_active": extensions::active_at_startup(),
        "adblock_updated": crate::adblock::updated_at(),
        "extensions_folder": extensions::directory(),
        "version": updater::CURRENT_VERSION,
        "is_default_browser": crate::desktop::is_default_browser(),
    });
    themed(include_str!("settings.html"))
        .replace("__COMMON_SCRIPT__", &script())
        .replace("__SETTINGS__", &data.to_string().replace('<', "\\u003c"))
}

pub fn get_topbar_html() -> String {
    themed(include_str!("topbar.html"))
}

pub fn get_popup_html() -> String {
    themed(include_str!("popup.html"))
}
