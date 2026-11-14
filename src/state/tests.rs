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
fn zoom_stays_with_its_tab_through_sleep_and_restore() {
    let mut state = BrowserState::new();
    let page = state.add_tab(Some("https://example.com/zoom".into()), false);
    state
        .tabs
        .iter_mut()
        .find(|tab| tab.id == page)
        .unwrap()
        .zoom = 150;
    state.toggle_sleep(page);
    assert_eq!(
        state.tabs.iter().find(|tab| tab.id == page).unwrap().zoom,
        150,
        "sleeping keeps the zoom"
    );
    let saved = serde_json::to_string(&state).unwrap();
    let restored = BrowserState::from_session(&saved).unwrap();
    assert_eq!(
        restored
            .tabs
            .iter()
            .find(|tab| tab.id == page)
            .unwrap()
            .zoom,
        150
    );
    let copy = state.duplicate_tab(page).unwrap();
    assert_eq!(
        state.tabs.iter().find(|tab| tab.id == copy).unwrap().zoom,
        150,
        "a duplicate opens at the same zoom"
    );
    let older: Tab = serde_json::from_str(
        r#"{"id":9,"workspace_id":1,"title":"t","url":"https://example.com/","is_pinned":false,"is_sleeping":false}"#,
    )
    .unwrap();
    assert_eq!(
        older.zoom, 100,
        "sessions saved before zoom existed still load"
    );
}

#[test]
fn closing_other_and_below_tabs_stays_in_the_workspace_and_keeps_pins() {
    let mut state = BrowserState::new();
    let first = state.active_tab_id;
    let pinned = state.add_tab(Some("https://example.com/pinned".into()), true);
    let second = state.add_tab(Some("https://example.com/2".into()), false);
    let third = state.add_tab(Some("https://example.com/3".into()), false);
    state.create_profile("Other").unwrap();
    let other = state.add_tab(Some("https://example.org/".into()), false);
    state.switch_workspace(1);
    assert_eq!(
        state.close_tabs_below(second),
        1,
        "only the tab after it closes"
    );
    assert!(state.tabs.iter().any(|tab| tab.id == second));
    assert!(!state.tabs.iter().any(|tab| tab.id == third));
    assert!(
        state.tabs.iter().any(|tab| tab.id == other),
        "other workspaces are untouched"
    );
    assert_eq!(state.active_tab_id, second);
    assert_eq!(
        state.close_other_tabs(second),
        1,
        "the unpinned opener closes"
    );
    assert!(!state.tabs.iter().any(|tab| tab.id == first));
    assert!(
        state.tabs.iter().any(|tab| tab.id == pinned),
        "pins survive"
    );
    assert_eq!(state.active_tab_id, second);
    assert_eq!(state.closed_tabs.len(), 2, "both closes are restorable");
}

#[test]
fn a_specific_closed_tab_can_be_reopened() {
    let mut state = BrowserState::new();
    let first = state.add_tab(Some("https://example.com/first".into()), false);
    let second = state.add_tab(Some("https://example.com/second".into()), false);
    state.close_tab(first);
    state.close_tab(second);
    assert_eq!(state.closed_tabs.len(), 2);
    let reopened = state.restore_closed_at(1).unwrap();
    assert_eq!(
        state
            .tabs
            .iter()
            .find(|tab| tab.id == reopened)
            .unwrap()
            .url,
        "https://example.com/first",
        "index 1 is the older of the two closed tabs"
    );
    assert_eq!(state.closed_tabs.len(), 1);
    assert!(
        state.restore_closed_at(5).is_none(),
        "an out-of-range index is ignored"
    );
    assert_eq!(
        state.closed_tabs.len(),
        1,
        "a failed restore keeps the list"
    );
}

#[test]
fn tab_ids_never_overflow_or_collide() {
    let mut state = BrowserState::new();
    state.next_tab_id = usize::MAX - 1;
    let added = state.add_tab(Some("https://example.com/a".into()), false);
    assert_ne!(added, usize::MAX - 1);
    assert_eq!(
        state.tabs.iter().filter(|tab| tab.id == added).count(),
        1,
        "a saturated counter still hands out a free id"
    );
    state.next_tab_id = usize::MAX - 1;
    let copy = state.duplicate_tab(state.active_tab_id).unwrap();
    assert_eq!(state.tabs.iter().filter(|tab| tab.id == copy).count(), 1);
    state.next_tab_id = usize::MAX - 1;
    state.close_tab(added);
    let restored = state.restore_closed_tab().unwrap();
    assert_eq!(
        state.tabs.iter().filter(|tab| tab.id == restored).count(),
        1
    );
}

#[test]
fn workspace_ids_reuse_gaps_and_never_overflow() {
    let mut state = BrowserState::new();
    let second = state.create_profile("Second").unwrap();
    let third = state.create_profile("Third").unwrap();
    assert_eq!((second, third), (2, 3));
    state.delete_workspace(second, 1).unwrap();
    assert_eq!(
        state.create_profile("Reused").unwrap(),
        2,
        "a deleted id is handed out again"
    );
    state.workspaces[0].id = usize::MAX - 2;
    let private = state.open_private_tab();
    let private = state
        .tabs
        .iter()
        .find(|tab| tab.id == private)
        .unwrap()
        .workspace_id;
    assert!(
        private < usize::MAX - 2,
        "the private profile takes a free id instead of overflowing"
    );
    assert!(
        state
            .workspaces
            .iter()
            .any(|w| w.private && w.id == private)
    );
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
        let restored = BrowserState::from_session(&serde_json::to_string(&state).unwrap()).unwrap();
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
    let mut restored = BrowserState::from_session(&serde_json::to_string(&state).unwrap()).unwrap();
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

#[test]
fn google_drive_shortcut_origin_points_to_drive_app() {
    let (host, root_url, label) =
        BrowserState::shortcut_origin("https://drive.google.com/drive/u/0/my-drive").unwrap();
    assert_eq!(host, "drive.google.com");
    assert_eq!(root_url, "https://drive.google.com/drive");
    assert_eq!(label, "Google Drive");
}

#[test]
fn essentials_follow_regular_workspaces_and_unload_back_to_pinned_url() {
    let mut state = BrowserState::new();
    let work = state.create_profile("Work").unwrap();
    state.switch_workspace(1);
    let essential = state.add_tab(Some("https://mail.example.com".into()), false);
    assert!(state.toggle_essential(essential).unwrap());
    assert_eq!(
        state
            .tabs
            .iter()
            .find(|t| t.id == essential)
            .unwrap()
            .pinned_url
            .as_deref(),
        Some("https://mail.example.com/")
    );
    state.update_active_tab(
        "https://mail.example.com/inbox/42".into(),
        Some("Inbox".into()),
    );
    state.switch_workspace(work);
    // Switching to an Essential keeps the user in their current regular workspace.
    state.switch_tab(essential);
    assert_eq!(state.active_workspace_id, work);
    assert_eq!(
        state
            .tabs
            .iter()
            .find(|t| t.id == essential)
            .unwrap()
            .workspace_id,
        work
    );
    assert!(state.unload_tab(essential));
    let unloaded = state.tabs.iter().find(|t| t.id == essential).unwrap();
    assert!(unloaded.is_sleeping);
    assert_eq!(unloaded.url, "https://mail.example.com/");
    assert_ne!(state.active_tab_id, essential);

    let private_tab = state.open_private_tab();
    assert!(state.toggle_essential(private_tab).is_err());
}

#[test]
fn folders_group_tabs_and_delete_without_losing_tabs() {
    let mut state = BrowserState::new();
    let t1 = state.add_tab(Some("https://example.com/1".into()), true);
    let t2 = state.add_tab(Some("https://example.com/2".into()), false);
    let folder = state.create_folder("Docs", Some(t1), false).unwrap();
    assert!(state.folders.iter().any(|f| f.id == folder && f.is_pinned));
    state.move_tab_to_folder(t2, Some(folder));
    assert!(state.tabs.iter().find(|t| t.id == t2).unwrap().is_pinned);
    state.toggle_folder(folder);
    assert!(
        state
            .folders
            .iter()
            .find(|f| f.id == folder)
            .unwrap()
            .collapsed
    );
    state.rename_folder(folder, "Reading").unwrap();
    assert_eq!(
        state.folders.iter().find(|f| f.id == folder).unwrap().name,
        "Reading"
    );
    state.delete_folder(folder);
    assert!(state.folders.is_empty());
    assert!(
        state
            .tabs
            .iter()
            .any(|t| t.id == t1 && t.folder_id.is_none())
    );
    assert!(
        state
            .tabs
            .iter()
            .any(|t| t.id == t2 && t.folder_id.is_none())
    );
}

#[test]
fn workspace_reordering_themes_and_split_swap_persist_cleanly() {
    let mut state = BrowserState::new();
    let second = state.create_profile("Second").unwrap();
    let third = state.create_profile("Third").unwrap();
    state.reorder_workspace(third, 0);
    assert_eq!(
        state.workspaces.iter().map(|w| w.id).collect::<Vec<_>>(),
        vec![third, 1, second]
    );
    assert!(state.set_workspace_accent(third, "#8ecbff").is_ok());
    assert!(state.set_workspace_accent(third, "not-a-color").is_err());

    state.switch_workspace(1);
    let partner = state.add_tab(Some("https://example.org".into()), false);
    let active = state.add_tab(Some("https://example.com".into()), false);
    assert!(state.split_with(partner));
    assert_eq!(state.split, Some([active, partner]));
    assert!(state.swap_split());
    assert_eq!(state.split, Some([partner, active]));
    assert!(state.toggle_rail());
    assert!(!state.sidebar_collapsed);
}

#[test]
fn number_shortcuts_follow_the_sidebar_order() {
    let mut state = BrowserState::new();
    let loose = state.add_tab(Some("https://a.example".into()), false);
    let foldered = state.add_tab(Some("https://b.example".into()), false);
    let pinned = state.add_tab(Some("https://c.example".into()), false);
    let essential = state.add_tab(Some("https://d.example".into()), false);
    state.toggle_pin(pinned);
    state.toggle_essential(essential).unwrap();
    state.create_folder("Read", Some(foldered), false).unwrap();
    assert_eq!(
        state.sidebar_order(),
        vec![essential, pinned, foldered, 1, loose]
    );
    assert_eq!(state.select_tab_position(2).unwrap().id, pinned);
    assert_eq!(state.select_tab_position(9).unwrap().id, loose);
    assert!(state.select_tab_position(8).is_none());
    assert_eq!(state.cycle_tab(false).unwrap().id, essential);
}

#[test]
fn close_shortcut_unloads_pinned_tabs_instead_of_closing_them() {
    let mut state = BrowserState::new();
    let pinned = state.add_tab(Some("https://mail.example".into()), false);
    state.toggle_pin(pinned);
    state
        .tabs
        .iter_mut()
        .find(|tab| tab.id == pinned)
        .unwrap()
        .url = "https://mail.example/inbox/42".into();
    assert!(state.close_shortcut_unloads(pinned));
    assert!(!state.close_shortcut_unloads(1));
    state.switch_tab(pinned);
    state.unload_tab(pinned);
    let tab = state.tabs.iter().find(|tab| tab.id == pinned).unwrap();
    assert!(tab.is_sleeping);
    assert_eq!(tab.url, "https://mail.example/");
    assert_eq!(state.active_tab_id, 1);
}

#[test]
fn split_shortcut_pairs_the_active_tab_with_the_last_one_used() {
    let mut state = BrowserState::new();
    let older = state.add_tab(Some("https://a.example".into()), false);
    let recent = state.add_tab(Some("https://b.example".into()), false);
    let active = state.add_tab(Some("https://c.example".into()), false);
    state.switch_tab(older);
    state.switch_tab(recent);
    state.switch_tab(active);
    assert!(state.split_with_recent());
    assert_eq!(state.split, Some([active, recent]));

    let mut lonely = BrowserState::new();
    assert!(!lonely.split_with_recent());
    assert_eq!(lonely.split, None);
}
