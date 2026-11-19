//! stop_tests.rs — a stopped front lets its state go, once a route answered: a route kept in the
//! state must never hold it, or the families' services, sockets and child processes outlive the front.
//! What a reader gets wrong: the server future ending proves nothing alone; the state's last strong hold must go too.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Weak;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::{AppState, Config};

const SECRET: &str = "test-secret";

/// A front being served, the state it built seen weakly, and the switch that stops it.
struct Running {
    addr: SocketAddr,
    root: PathBuf,
    state: Weak<AppState>,
    halt: oneshot::Sender<()>,
    served: JoinHandle<std::io::Result<()>>,
}

/// A front on a folder of its own, given `env` beside the session secret and the auth folder.
async fn start(name: &str, env: impl Fn(&Path) -> Vec<(&'static str, String)>) -> Running {
    let root = std::env::temp_dir().join(format!("k7-front-stop-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let mut vars: BTreeMap<String, String> = BTreeMap::new();
    vars.insert("SESSION_SECRET".into(), SECRET.into());
    vars.insert(
        "STUDIO_AUTH_DIR".into(),
        root.join("auth").to_string_lossy().into_owned(),
    );
    for (k, v) in env(&root) {
        vars.insert(k.into(), v);
    }
    let config = Config {
        listen: "127.0.0.1:0".parse().unwrap(),
        secret: "s".into(),
        trust_hops: 0,
        env: vars,
    };
    let (halt, stopped) = oneshot::channel::<()>();
    let (tx_addr, rx_addr) = oneshot::channel();
    let (tx_state, rx_state) = oneshot::channel();
    let shutdown = async move {
        let _ = stopped.await;
    };
    let ready = move |a| tx_addr.send(a).unwrap();
    let seen = move |w| {
        let _ = tx_state.send(w);
    };
    let served = tokio::spawn(super::serve_seen(
        config,
        Default::default(),
        shutdown,
        ready,
        seen,
    ));
    Running {
        addr: rx_addr.await.unwrap(),
        state: rx_state.await.unwrap(),
        root,
        halt,
        served,
    }
}

impl Running {
    /// Waits until `owned` says the family is answered here.
    async fn owned(&self, owned: impl Fn(&AppState) -> bool) {
        for _ in 0..200 {
            if self.state.upgrade().is_some_and(|s| owned(&s)) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("the front never took the family");
    }

    /// Stops the front, then checks the server ended and nothing holds its state any more.
    async fn stop(self) {
        self.halt.send(()).unwrap();
        // Generous: the hub stops its services on the way out, and the whole suite runs at once beside it.
        let served = tokio::time::timeout(Duration::from_secs(20), self.served).await;
        served
            .expect("the front did not stop within 20 s")
            .unwrap()
            .unwrap();
        for _ in 0..100 {
            if self.state.strong_count() == 0 {
                let _ = std::fs::remove_dir_all(&self.root);
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!(
            "the front's state is still held {} times after it stopped",
            self.state.strong_count()
        );
    }
}

/// One exchange with the front: the status and the body.
async fn send(
    addr: SocketAddr,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> (u16, String) {
    let mut raw = format!(
        "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    for (k, v) in headers {
        raw.push_str(&format!("{k}: {v}\r\n"));
    }
    if !body.is_empty() {
        raw.push_str("content-type: application/json\r\n");
    }
    raw.push_str("\r\n");
    raw.push_str(body);
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(raw.as_bytes()).await.unwrap();
    let mut out = Vec::new();
    let read = tokio::time::timeout(Duration::from_secs(30), s.read_to_end(&mut out)).await;
    read.unwrap_or_else(|_| panic!("{method} {path}: no answer within 30 s"))
        .unwrap();
    let text = String::from_utf8_lossy(&out).into_owned();
    let (head, body) = text.split_once("\r\n\r\n").unwrap();
    (head[9..12].parse().unwrap(), body.to_string())
}

/// The account folder's name for `email`, as the hub spells it.
#[tokio::test]
async fn the_files_family_lets_the_state_go() {
    let f = start("files", |root| {
        let path = |p: &str| root.join(p).to_string_lossy().into_owned();
        vec![
            ("K7_FILES", "front".into()),
            ("K7_ENGINE_CORE", "front".into()),
            ("STUDIO_FILES_ROOT", path("files")),
            ("STUDIO_PREFS_DIR", path("prefs")),
            ("STUDIO_MEDIATEK", path("mediatek")),
        ]
    })
    .await;
    f.owned(|s| s.files.as_ref().is_some_and(|x| x.owned()))
        .await;
    let (status, body) = send(
        f.addr,
        "GET",
        "/api/ready",
        &[("host", &f.addr.to_string())],
        "",
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert!(!body.starts_with("ts "), "the backend answered: {body}");
    f.stop().await;
}
