use crate::{history, state::BrowserState};

pub fn get_start_page_html(state: &BrowserState) -> String {
    let data = serde_json::json!({
        "shortcuts": state.top_shortcuts(),
        "bookmarks": state.bookmarks,
        "history": history::all(),
        "closed": state
            .closed_tabs
            .iter()
            .rev()
            .take(5)
            .map(|tab| serde_json::json!({ "title": tab.title, "url": tab.url }))
            .collect::<Vec<_>>(),
    });
    crate::ui::themed(include_str!("start_page.html"))
        .replace("__COMMON_SCRIPT__", &crate::ui::script())
        .replace("__SHORTCUTS__", &data.to_string().replace('<', "\\u003c"))
}
