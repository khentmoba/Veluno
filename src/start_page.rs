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
    let html = crate::ui::themed(include_str!("start_page.html"))
        .replace("__COMMON_SCRIPT__", &crate::ui::script())
        .replace("__SHORTCUTS__", &data.to_string().replace('<', "\\u003c"));
    match workspace_tint(state) {
        Some(color) => html.replacen(
            "</head>",
            &format!("<style>:root{{--ws:{color};--ws-a:30%;--ws-b:18%}}</style></head>"),
            1,
        ),
        None => html,
    }
}

/// The active workspace's color for the start page gradient, when workspace themes are on.
fn workspace_tint(state: &BrowserState) -> Option<&str> {
    if !state.workspace_themes {
        return None;
    }
    let workspace = state
        .workspaces
        .iter()
        .find(|workspace| workspace.id == state.active_workspace_id)?;
    let color = workspace.accent.as_str();
    let hex = color.strip_prefix('#')?;
    let valid = matches!(hex.len(), 3 | 6) && hex.chars().all(|c| c.is_ascii_hexdigit());
    (valid && !workspace.private && !color.eq_ignore_ascii_case("#dedee3")).then_some(color)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_page_tint_follows_a_valid_workspace_color_only() {
        let mut state = BrowserState::new();
        assert_eq!(workspace_tint(&state), None, "neutral color adds no tint");
        state.workspaces[0].accent = "#8ab4f8".into();
        assert_eq!(workspace_tint(&state), Some("#8ab4f8"));
        state.workspace_themes = false;
        assert_eq!(workspace_tint(&state), None);
        state.workspace_themes = true;
        state.workspaces[0].accent = "red;}*{x".into();
        assert_eq!(workspace_tint(&state), None);
        state.workspaces[0].accent = "#8ab4f8".into();
        state.workspaces[0].private = true;
        assert_eq!(workspace_tint(&state), None);
    }
}
