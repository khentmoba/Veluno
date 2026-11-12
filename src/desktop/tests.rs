use super::*;

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
        file_name(" \t... ", stamp),
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
