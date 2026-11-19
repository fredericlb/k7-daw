//! The one table's fall-through: an unknown route is a 404, and a route a piece declined is refused with a sentence.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::Request;
use k7_hub::account::Names;

use super::{Held, Services, Who, fallen_through};

/// A front's services over a scratch folder and the host's settings `env`.
struct Front {
    dir: std::path::PathBuf,
    svc: Services,
    instances: Arc<Names>,
}

fn front(name: &str, env: &[(&str, &str)]) -> Front {
    // A switch value such as front:a@b holds a colon, which Windows refuses in a folder name.
    let name: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let dir = std::env::temp_dir().join(format!("k7-switches-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("demos")).unwrap();
    let data = dir.join("data");
    let mut o = k7_hub::account::Names::new(Arc::new(move |u: &str| {
        let slug = u.replace('@', "-");
        (slug.clone(), data.join(slug))
    }));
    let demos = dir.join("demos").display().to_string();
    o.env = Arc::new(move |_, _| {
        let demos = demos.clone();
        Box::pin(async move {
            BTreeMap::from([
                ("STUDIO_DEMOS".to_string(), demos),
                ("K7_NO_FS_WATCH".to_string(), "1".to_string()),
            ])
        })
    });
    let env: BTreeMap<String, String> = env
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect();
    let svc = Services::new(&env, Arc::new(|_: &str| {}));
    Front {
        dir,
        svc,
        instances: Arc::new(o),
    }
}

fn who<'a>(f: &'a Front, held: &'a Held) -> Who<'a> {
    Who {
        held,
        user: "c@d.org",
        device: false,
        log: &f.svc.log,
        code_root: None,
    }
}

fn request(method: &str, path: &str) -> Request {
    Request::builder()
        .method(method)
        .uri(path)
        .header("x-studio-workspace", "songs")
        .body(Body::empty())
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_route_no_piece_owns_is_a_404_and_one_a_piece_declined_is_refused() {
    let f = front("fall", &[]);
    let held = f.svc.held("c@d.org", &f.instances).await.unwrap();
    std::fs::create_dir_all(held.account.home.join("workspace/songs")).unwrap();
    let w = who(&f, &held);
    // Paths `live()` handed to node's routers that no piece answers now, and one nothing ever named.
    for (method, path) in [
        ("POST", "/import-project"),
        ("POST", "/import-upload"),
        ("GET", "/nothing-here"),
        ("GET", "/sync/log"),
        ("GET", "/sync/conflicts"),
        ("GET", "/workspaces/sync"),
    ] {
        let r = fallen_through(&w, &request(method, path), path);
        assert_eq!(r.status().as_u16(), 404, "{method} {path}");
    }
    // A route a piece owns and declines is refused with a sentence, never a 404.
    let r = super::native::refuse::scoped(async {
        fallen_through(&w, &request("GET", "/workspaces"), "/workspaces")
    })
    .await;
    assert_eq!(r.status().as_u16(), 409);
    f.svc.stop().await;
    let _ = std::fs::remove_dir_all(&f.dir);
}

// Two first requests of one account mint its environment once: a second mint would revoke the publish token the first holds.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_first_requests_of_an_account_make_its_environment_once() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let mut f = front("once", &[]);
    let data = f.dir.join("data");
    let mut o = Names::new(Arc::new(move |u: &str| {
        let slug = u.replace('@', "-");
        (slug.clone(), data.join(slug))
    }));
    let minted = Arc::new(AtomicUsize::new(0));
    let count = minted.clone();
    o.env = Arc::new(move |_, _| {
        count.fetch_add(1, Ordering::SeqCst);
        // A mint that takes a moment, as the hub's does, so the second request arrives while it runs.
        Box::pin(async {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            BTreeMap::from([("K7_NO_FS_WATCH".to_string(), "1".to_string())])
        })
    });
    f.instances = Arc::new(o);
    let (a, b) = tokio::join!(
        f.svc.held("t@x.org", &f.instances),
        f.svc.held("t@x.org", &f.instances)
    );
    assert!(Arc::ptr_eq(&a.unwrap(), &b.unwrap()));
    assert_eq!(
        minted.load(Ordering::SeqCst),
        1,
        "the environment was minted twice"
    );
    f.svc.stop().await;
    let _ = std::fs::remove_dir_all(&f.dir);
}
