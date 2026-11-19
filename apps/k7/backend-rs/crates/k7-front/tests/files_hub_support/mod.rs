//! What the file hub's tests share: a front with `K7_FILES=front` over a folder of
//! its own whose host still names a node program, which a test then proves was never run (`node_never_ran`).
//! What a reader gets wrong: the program is a marker, not a service; no account's files are ever node's.

#![allow(dead_code, reason = "each test binary uses its own share of these")]

use std::net::SocketAddr;
use std::path::PathBuf;

use crate::http_support::{self, Reply};

/// What a host that still names node's file service would start: a script that leaves a marker where it runs.
const MARKER: &str = "no-node-ran";

/// A server's front over a folder of its own; the host's `K7_NODE` and service entries name the marker script.
pub async fn front() -> (SocketAddr, PathBuf) {
    front_with(&[], |_| {}).await
}

/// A device's front (`STUDIO_LOCAL_USER`): the only one whose accounts have a runtime.
pub async fn device_front() -> (SocketAddr, PathBuf) {
    device_front_with(|_| {}).await
}

/// A device's front whose folder `prepare` fills before it starts, since the local user's runtime is made at launch.
pub async fn device_front_with(prepare: impl FnOnce(&std::path::Path)) -> (SocketAddr, PathBuf) {
    front_with(&[("STUDIO_LOCAL_USER", "1")], prepare).await
}

async fn front_with(
    extra: &[(&str, &str)],
    prepare: impl FnOnce(&std::path::Path),
) -> (SocketAddr, PathBuf) {
    // One folder per front: the tests run at once, and a front wipes its folder before it starts.
    static RUN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let run = RUN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("k7-front-files-{}-{run}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    prepare(&root);
    let node = root.join("node.sh");
    std::fs::write(
        &node,
        format!("#!/bin/sh\ntouch '{}'\n", root.join(MARKER).display()),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&node, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let entry = root.join("service.mjs").display().to_string();
    let env = [
        ("SESSION_SECRET", "test-secret".to_string()),
        ("STUDIO_AUTH_DIR", root.join("auth").display().to_string()),
        (
            "STUDIO_MEDIATEK",
            root.join("library").display().to_string(),
        ),
        (
            "STUDIO_FILES_ROOT",
            root.join("files").display().to_string(),
        ),
        ("K7_ENGINE_CORE", "front".into()),
        ("K7_FILES", "front".into()),
        ("K7_NODE", node.display().to_string()),
        ("STUDIO_FILE_SERVICE_ENTRY", entry.clone()),
        ("STUDIO_LIBRARY_SERVICE_ENTRY", entry),
    ]
    .into_iter()
    .chain(extra.iter().map(|(k, v)| (*k, v.to_string())))
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    let config = k7_front::Config {
        listen: "127.0.0.1:0".parse().unwrap(),
        secret: "s".into(),
        trust_hops: 0,
        env,
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(k7_front::serve(config, std::future::pending(), move |a| {
        tx.send(a).unwrap()
    }));
    let addr = rx.await.unwrap();
    for _ in 0..200 {
        if matches!(
            send(addr, "GET", "/api/files-hub", &[]).await.status,
            200 | 401
        ) {
            return (addr, root);
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("the front never took the file hub");
}

/// One exchange, its chunked body joined.
pub async fn send(addr: SocketAddr, method: &str, path: &str, headers: &[(&str, &str)]) -> Reply {
    send_with(addr, method, path, headers, "").await
}

/// The same, with a JSON body — a route that takes one is asked what it refuses.
pub async fn send_with(
    addr: SocketAddr,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> Reply {
    let mut r = http_support::send_with(addr, method, path, headers, body).await;
    if r.header("transfer-encoding") == Some("chunked") {
        let (mut body, mut rest) = (Vec::new(), &r.body[..]);
        while let Some(at) = rest.windows(2).position(|w| w == b"\r\n") {
            let n = usize::from_str_radix(std::str::from_utf8(&rest[..at]).unwrap().trim(), 16)
                .unwrap();
            if n == 0 {
                break;
            }
            body.extend_from_slice(&rest[at + 2..at + 2 + n]);
            rest = &rest[at + 4 + n..];
        }
        r.body = body;
    }
    r
}

/// Whether the host's node program was ever run: it must not have been, whatever the front answered.
pub fn node_never_ran(root: &std::path::Path) -> bool {
    !root.join(MARKER).exists()
}

/// A device's writes wait for its libraries' first `ensure`: asked again until the 503 is gone.
pub async fn settled(addr: SocketAddr) {
    for _ in 0..200 {
        if send(addr, "POST", "/api/files/none", &[]).await.status != 503 {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("the libraries never settled");
}
