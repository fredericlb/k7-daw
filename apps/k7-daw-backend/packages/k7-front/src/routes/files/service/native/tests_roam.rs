//! The roaming list's route: the device's workspaces reach the installed store with their remotes, and a store of the
//! host that has none answers `available: false`. The one test holds the process-wide store, so it is the only one to install.

use std::sync::Arc;

use k7_roam::Fake;
use serde_json::json;

use super::git::Kind;
use super::tests_git::{ask, device, post, url, ws};

#[tokio::test(flavor = "multi_thread")]
async fn the_list_follows_the_workspaces_of_this_device() {
    let d = device("roam");
    let (_, v, _) = ask(&d, Kind::Roam, "GET", "/git/roam", None).await;
    assert_eq!(v["available"], false, "a host without a store has no list");
    k7_roam::store::install(Arc::new(Fake::default()));
    let (a, remote) = (
        ws(&d, "roam-plain"),
        device("roam-remote").dir.join("bare.git"),
    );
    std::fs::write(a.join("a.k7x"), "a: 1\n").unwrap();
    let wired = ws(&d, "roam-wired");
    std::fs::write(wired.join("b.k7x"), "b: 1\n").unwrap();
    super::tests_git::bare(&remote);
    let (_, v, _) = ask(&d, Kind::Roam, "GET", "/git/roam", None).await;
    let names: Vec<_> = v["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["name"].as_str())
        .collect();
    assert!(
        names.contains(&"roam-plain") && names.contains(&"roam-wired"),
        "{v}"
    );
    let (s, ..) = post(
        &d,
        Kind::Connect,
        "/git/connect",
        json!({ "workspace": "roam-wired", "url": url(&remote) }),
    )
    .await;
    assert_eq!(s, 200);
    let (_, v, _) = ask(&d, Kind::Roam, "GET", "/git/roam", None).await;
    let wired_entry = v["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "roam-wired")
        .unwrap();
    assert_eq!(wired_entry["remote"], url(&remote));
    let plain = v["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "roam-plain")
        .unwrap();
    assert!(plain["remote"].is_null());
    assert_eq!(v["here"].as_array().unwrap().len(), 2, "{v}");
    std::fs::remove_dir_all(&wired).unwrap();
    let (_, v, _) = ask(&d, Kind::Roam, "GET", "/git/roam", None).await;
    assert!(
        v["entries"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["name"] != "roam-wired"),
        "{v}"
    );
}
