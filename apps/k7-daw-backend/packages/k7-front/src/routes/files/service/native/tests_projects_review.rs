//! The G4 review's missing test (U4): a marker that fails after the move.

#[cfg(unix)]
use serde_json::json;

#[cfg(unix)]
use super::projects::Kind;
#[cfg(unix)]
use super::tests::scratch;
#[cfg(unix)]
use super::tests_projects::{go, make, proj};

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_marker_that_fails_after_a_move_answers_200_with_the_moves_body_logged_and_counted_io() {
    // U4 (pending): the folder moved, so a refusal would tell the person it did not.
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("pr-late");
    let (p, lines) = proj(&dir, "pr-late");
    make(&dir, "a.k7").await;
    let open = json!({ "path": "a.k7" });
    assert_eq!(go(&p, Kind::Open, &dir, open).await.0, 200);
    let markers = dir.join("home/workspace/.active-projects");
    std::fs::set_permissions(&markers, std::fs::Permissions::from_mode(0o555)).unwrap();
    let denied = std::fs::File::create(markers.join("probe")).is_err();
    let rename = json!({ "path": "a.k7", "name": "b.k7" });
    let (status, said) = go(&p, Kind::Rename, &dir, rename).await;
    std::fs::set_permissions(&markers, std::fs::Permissions::from_mode(0o755)).unwrap();
    if denied {
        assert_eq!((status, said.as_str()), (200, r#"{"path":"b.k7"}"#));
        assert!(dir.join("home/workspace/songs/b.k7").exists());
        assert_eq!(super::tests::counted(&super::stats::global().io), 1);
        let log = lines.lock().unwrap();
        assert!(
            log.iter()
                .any(|l| l.contains("marker write failed after the move")),
            "{log:?}"
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}
