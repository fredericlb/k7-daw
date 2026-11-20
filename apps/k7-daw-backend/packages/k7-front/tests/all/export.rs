//! The export routes through the front: no host is a 501, an unpicked folder is refused, a picked one is written to.
//! One test, since the host is the process's: the order below is the point.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::support::{front, send};

struct Fixed(PathBuf);

impl k7_front::export::Host for Fixed {
    fn pick_folder(&self, _: Option<&Path>) -> Option<PathBuf> {
        Some(self.0.clone())
    }
    fn reveal(&self, _: &Path) {}
}

#[tokio::test]
async fn a_folder_is_written_only_once_the_dialog_gave_it() {
    let f = front("export", &[("STUDIO_LOCAL_USER", "1")]).await;
    let cookie = crate::http_support::cookie();
    let signed = [("cookie", cookie.as_str())];
    let none = send(f.addr, "POST", "/api/local/export/folder", &signed, "").await;
    assert_eq!(none.status, 501, "{}", none.body);

    let dir = std::env::temp_dir().join(format!("k7-export-route-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    k7_front::export::set_host(Arc::new(Fixed(dir.clone())));
    let path = format!("/api/local/export/file?folder={}&name=a.wav", dir.display());
    let before = send(f.addr, "PUT", &path, &signed, "bytes").await;
    assert_eq!(
        before.status, 403,
        "no folder was picked yet: {}",
        before.body
    );
    assert_eq!(send(f.addr, "PUT", &path, &[], "bytes").await.status, 401);

    let picked = send(f.addr, "POST", "/api/local/export/folder", &signed, "").await;
    assert_eq!(picked.status, 200, "{}", picked.body);
    assert!(
        picked.body.contains("\"name\":\"k7-export-route-"),
        "{}",
        picked.body
    );
    let put = send(f.addr, "PUT", &path, &signed, "bytes").await;
    assert_eq!(put.status, 200, "{}", put.body);
    assert_eq!(std::fs::read_to_string(dir.join("a.wav")).unwrap(), "bytes");
    let bad = format!(
        "/api/local/export/file?folder={}&name=..%2Fa.wav",
        dir.display()
    );
    assert_eq!(send(f.addr, "PUT", &bad, &signed, "x").await.status, 400);
    let _ = std::fs::remove_dir_all(&dir);
}
