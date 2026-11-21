//! The service's own helpers: the front's path and a workspace's name (the owner marker is tests_owner.rs').

use super::*;

#[test]
fn no_spelling_of_front_reaches_node() {
    for p in [
        "/front",
        "/front/machine",
        "/FRONT/machine",
        "/Front/ask",
        "/%66ront/machine",
        "/%46RONT/ask",
        "//front/machine",
    ] {
        assert!(names_front(p), "{p}");
    }
    for p in ["/frontier", "/file", "/workspace", "/x/front"] {
        assert!(!names_front(p), "{p}");
    }
}

#[test]
fn names_a_workspace_as_the_service_does() {
    for ok in ["songs", "My Songs", "a.b_c-d", "0"] {
        assert!(workspace_name_ok(ok), "{ok}");
    }
    for bad in ["", ".", "..", ".hidden", "a/b", "a\\b", "é", "-x", " x"] {
        assert!(!workspace_name_ok(bad), "{bad}");
    }
}

#[tokio::test]
async fn a_name_with_no_folder_is_no_workspace_and_is_not_made() {
    let dir = std::env::temp_dir().join(format!("k7-is-ws-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let studio = dir.display().to_string().replace('\\', "/");
    let env = std::collections::BTreeMap::from([("STUDIO_HOME".to_string(), studio)]);
    let home = k7_file_service::libraries::Home::of(&env);
    let ws = k7_file_service::workspaces::names::workspace_dir(&home);
    std::fs::create_dir_all(format!("{ws}/band")).unwrap();
    assert!(is_workspace(&home, "band").await);
    assert!(!is_workspace(&home, "none").await);
    assert!(!std::path::Path::new(&format!("{ws}/none")).exists());
    std::fs::remove_dir_all(dir).unwrap();
}
