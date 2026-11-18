//! hostile.rs — a forged or missing secret, a body over the limit, a seed with a link.

use std::fs;
use std::sync::Arc;

use k7_libraries::{Libraries, Request, Store};
use serde_json::json;

use crate::support::Root;

const SECRET: &str = "s3cret";

fn seeded(root: &Root) -> Arc<Libraries> {
    fs::create_dir_all(root.0.join("seed/k7-core/modules")).unwrap();
    fs::write(root.0.join("seed/k7-core/modules/mix.k7x"), "x(1)\n").unwrap();
    let seed = root.0.join("seed").to_string_lossy().into_owned();
    let store = Arc::new(Store::new(&root.0.join("data"), Some(&seed)));
    Libraries::new(store, SECRET.into())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_request_that_is_not_the_hubs_is_answered_nothing() {
    let root = Root::new();
    let svc = seeded(&root);
    svc.first_ensure().await;
    // The secret missing, empty or forged: nothing is read either.
    let body = b"{}";
    for secret in [
        None,
        Some(""),
        Some("forged"),
        Some("s3cret "),
        Some("S3CRET"),
    ] {
        let r = svc
            .answer(&Request {
                method: "GET",
                path: "/account/libraries",
                query: "",
                secret,
                admin: true,
                body,
            })
            .await;
        assert_eq!(r.status, 401, "{secret:?}");
        assert_eq!(
            r.value().unwrap()["error"],
            json!("not from this service's file hub")
        );
    }
    // A service made with no secret answers nobody.
    let open = Libraries::new(svc.store.clone(), String::new());
    let r = open
        .answer(&Request {
            method: "GET",
            path: "/status",
            query: "",
            secret: Some(""),
            admin: true,
            body,
        })
        .await;
    assert_eq!(r.status, 401);
    // And a body over the limit is refused with its sentence.
    let big = vec![b' '; k7_libraries::JSON_BODY_MAX + 1];
    let r = svc
        .answer(&Request {
            method: "POST",
            path: "/libraries/sync",
            query: "",
            secret: Some(SECRET),
            admin: true,
            body: &big,
        })
        .await;
    assert_eq!(
        (r.status, r.value().unwrap()["error"].as_str().unwrap()),
        (413, "body over 8388608 bytes")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_link_in_a_seed_or_a_version_serves_nothing_outside() {
    let root = Root::new();
    let outside = root.0.join("outside.txt");
    fs::write(&outside, "secret outside").unwrap();
    let seed = root.0.join("seed/k7-core");
    fs::create_dir_all(&seed).unwrap();
    fs::write(seed.join("mix.k7x"), "x(1)\n").unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&outside, seed.join("leak.txt")).unwrap();
        std::os::unix::fs::symlink(&root.0, seed.join("up")).unwrap();
    }
    let svc = seeded(&root);
    // `seeded` laid its own seed: point the store at the one holding the links.
    let store = Arc::new(Store::new(
        &root.0.join("data2"),
        Some(&root.0.join("seed").to_string_lossy()),
    ));
    let notes = store.ensure(None, false);
    assert_eq!(notes[0].state, "seeded", "{}", notes[0].note);
    let v1 = root.0.join("data2/libraries/k7-core/1");
    assert!(v1.join("mix.k7x").exists());
    assert!(!v1.join("leak.txt").exists() && !v1.join("up").exists());
    drop(svc);
}
