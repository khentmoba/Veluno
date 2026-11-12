use super::*;

fn action(key: u32, ctrl: bool, shift: bool, alt: bool) -> String {
    format!("{:?}", shortcut(key, ctrl, shift, alt))
}

#[test]
fn content_tabs_refuse_data_and_script_urls() {
    for url in [
        "https://example.com/",
        "veluno://newtab",
        "about:blank",
        "blob:https://drive.google.com/7b1486be-4607-d53d-9f93-841b7c776bc5",
    ] {
        assert!(is_valid_navigation(url), "{url}");
    }
    for url in [
        "data:text/html,<script>alert(1)</script>",
        "data:text/html;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==",
        "javascript:alert(1)",
    ] {
        assert!(!is_valid_navigation(url), "{url}");
    }
}

#[test]
fn error_page_data_url_is_the_only_allowed_data_url() {
    use base64::Engine as _;
    let page = include_str!("../error.html");
    let error_url = format!(
        "data:text/html;charset=utf-8;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(page)
    );
    assert!(is_valid_navigation(&error_url));
    let other_page = format!(
        "data:text/html;charset=utf-8;base64,{}",
        base64::engine::general_purpose::STANDARD.encode("<p>other</p>")
    );
    assert!(!is_valid_navigation(&other_page));
    let same_page_other_mime = format!(
        "data:text/plain;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(page)
    );
    assert!(!is_valid_navigation(&same_page_other_mime));
}

#[test]
fn palette_reveals_on_first_open_after_launch() {
    let mut shown = PaletteShown::new();
    assert_eq!(shown.transition(true), Some(true));
    assert_eq!(shown.transition(true), None);
    assert_eq!(shown.transition(false), Some(false));
    assert_eq!(shown.transition(false), None);
    assert_eq!(shown.transition(true), Some(true));
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
    let mut hidden_at = HashMap::from([(1, (old, false)), (2, (old, false)), (3, (now, false))]);
    assert_eq!(take_tabs_to_trim(&mut hidden_at, 1), vec![2]);
    assert!(take_tabs_to_trim(&mut hidden_at, 1).is_empty());
    hidden_at.remove(&2);
    hidden_at.insert(2, (old, false));
    assert_eq!(take_tabs_to_trim(&mut hidden_at, 1), vec![2]);
}

#[test]
fn animation_step_and_easing_progress_smoothly() {
    assert_eq!(ease(0.), 0.);
    assert_eq!(ease(1.), 1.);
    assert_eq!(ease(0.5), 0.5);
    assert!(ease(0.3) < 0.3);
    assert!(ease(0.7) > 0.7);

    let p1 = step(0., 1., 0.05);
    assert!(p1 > 0. && p1 < 1.);
    let p2 = step(p1, 1., 0.05);
    assert!(p2 > p1 && p2 < 1.);

    let r1 = step(1., 0., 0.05);
    assert!(r1 < 1. && r1 > 0.);
    let r2 = step(r1, 0., 0.05);
    assert!(r2 < r1 && r2 > 0.);
}

#[test]
fn peek_to_dock_handoff_retains_reveal_without_wobble() {
    let mut dock_progress = 0.0;
    let mut reveal_progress = 1.0;

    let dock_target: f64 = 1.0;
    let peek_target: f64 = 0.0;
    let reveal_target = dock_target.max(peek_target);

    assert_eq!(reveal_target, 1.0);
    for _ in 0..10 {
        reveal_progress = step(reveal_progress, reveal_target, 0.016);
        dock_progress = step(dock_progress, dock_target, 0.016);
        assert_eq!(
            reveal_progress, 1.0,
            "Sidebar reveal must remain 1.0 during peek-to-dock handoff"
        );
        assert!(dock_progress > 0.0, "Dock progress should rise");
    }
}

#[test]
fn side_panel_bounds_interpolate_with_progress() {
    let content = Rect {
        position: Position::Logical(LogicalPosition::new(0., 0.)),
        size: Size::Logical(LogicalSize::new(1000., 800.)),
    };
    let (closed_content, closed_panel) = side_panel_bounds(content, 1.0, false, 0.0);
    assert_eq!(closed_content.size.to_logical::<f64>(1.0).width, 1000.);
    assert_eq!(closed_panel.position.to_logical::<f64>(1.0).x, 1000.);

    let (half_content, _) = side_panel_bounds(content, 1.0, false, 0.5);
    let (open_content, open_panel) = side_panel_bounds(content, 1.0, false, 1.0);
    let open_width = open_panel.size.to_logical::<f64>(1.0).width;
    assert!(half_content.size.to_logical::<f64>(1.0).width < 1000.);
    assert!(
        half_content.size.to_logical::<f64>(1.0).width
            > open_content.size.to_logical::<f64>(1.0).width
    );
    assert_eq!(
        open_content.size.to_logical::<f64>(1.0).width,
        1000. - open_width - ui::WINDOW_MARGIN
    );
}
