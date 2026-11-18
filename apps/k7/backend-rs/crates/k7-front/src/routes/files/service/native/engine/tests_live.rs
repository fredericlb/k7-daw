//! The live harness (2261): the link against the REAL engine, `K7_LIVE_ENGINE_URL` naming one started alone, silent, in a
//! scratch home. Run with `-- --ignored`. A tap between the link and the engine records the wire, since the engine does not
//! report its master back. What a reader gets wrong: the kill test starts an engine of its own and wants none other of
//! yours up, since a second engine on one machine does not serve while the first holds the audio device.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use super::fake::{Rig, minimal_doc, until};

fn engine_url() -> String {
    std::env::var("K7_LIVE_ENGINE_URL").expect("K7_LIVE_ENGINE_URL names a running engine")
}

/// A relay that forwards every text frame both ways and keeps those the link sent.
struct Tap {
    url: String,
    sent: Arc<Mutex<Vec<Value>>>,
}

async fn tap(upstream: String) -> Tap {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", l.local_addr().unwrap());
    let sent: Arc<Mutex<Vec<Value>>> = Arc::default();
    let log = sent.clone();
    tokio::spawn(async move {
        while let Ok((tcp, _)) = l.accept().await {
            let up = upstream.clone();
            let log = log.clone();
            tokio::spawn(async move {
                let Ok(down) = tokio_tungstenite::accept_async(tcp).await else {
                    return;
                };
                let Ok((up, _)) = tokio_tungstenite::connect_async(&up).await else {
                    return;
                };
                let (mut dw, mut dr) = down.split();
                let (mut uw, mut ur) = up.split();
                let a = async {
                    while let Some(Ok(m)) = dr.next().await {
                        if let Message::Text(t) = &m
                            && let Ok(v) = serde_json::from_str::<Value>(t.as_str())
                        {
                            log.lock().unwrap().push(v);
                        }
                        if uw.send(m).await.is_err() {
                            break;
                        }
                    }
                };
                let b = async {
                    while let Some(Ok(m)) = ur.next().await {
                        if dw.send(m).await.is_err() {
                            break;
                        }
                    }
                };
                // Either side ending ends the relay, as a killed engine ends its connection.
                tokio::select! { _ = a => {}, _ = b => {} }
            });
        }
    });
    Tap { url, sent }
}

fn kinds(t: &Tap) -> Vec<String> {
    t.sent
        .lock()
        .unwrap()
        .iter()
        .map(|f| f["type"].as_str().unwrap_or("").to_string())
        .collect()
}

#[tokio::test]
#[ignore = "needs K7_LIVE_ENGINE_URL"]
async fn an_open_goes_through_the_gate_against_the_real_engine() {
    let rig = Rig::new("live").await;
    let t = tap(engine_url()).await;
    rig.link.bind(&json!({ "url": t.url, "engineId": "live" }));
    let s = rig.session();
    s.start(&rig.ws, &rig.project).await;
    until(60, "the load to end", || {
        s.load_phase().is_none()
            && t.sent
                .lock()
                .unwrap()
                .iter()
                .filter(|f| f["type"] == "project")
                .count()
                >= 3
    })
    .await;
    let mut masters: Vec<f64> = t
        .sent
        .lock()
        .unwrap()
        .iter()
        .filter(|f| f["type"] == "project")
        .map(|f| f["doc"]["masterVolume"].as_f64().unwrap())
        .collect();
    // The first open re-sends the served project, as the TypeScript's does.
    masters.dedup();
    assert_eq!(masters, vec![-3.0, -100.0, -3.0]);
    assert!(kinds(&t).contains(&"transport".to_string()));
    // The engine holds the session, as the poll's check reads it.
    let again = s.clone();
    again.prewarm(&rig.ws, &rig.project).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(
        rig.link
            .stats()
            .resends
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
}

#[tokio::test]
#[ignore = "needs K7_LIVE_ENGINE_URL"]
async fn a_switch_flushes_and_a_killed_engine_is_dialled_again() {
    let rig = Rig::new("live-switch").await;
    let t = tap(engine_url()).await;
    rig.link.bind(&json!({ "url": t.url, "engineId": "live" }));
    let s = rig.session();
    s.start(&rig.ws, &rig.project).await;
    until(60, "the first load", || {
        s.load_phase().is_none() && kinds(&t).iter().filter(|k| *k == "project").count() >= 3
    })
    .await;
    let other = format!("{}/q.k7", rig.ws);
    std::fs::create_dir_all(&other).unwrap();
    *rig.host.doc.lock().unwrap() = minimal_doc(-6.0);
    s.start(&rig.ws, &other).await;
    until(60, "the second load", || {
        kinds(&t).iter().filter(|k| *k == "flush").count() >= 1
    })
    .await;
    // A dead socket: the tap drops, the link dials again and re-sends what it serves.
    s.drop_socket();
    s.resend_served().await;
    until(60, "the re-sent project", || s.is_ready()).await;
}

#[tokio::test]
#[ignore = "needs K7_LIVE_ENGINE_URL, ideally an engine still scanning a fresh home"]
async fn the_scan_gate_waits_on_a_real_scan_and_writes_its_catalogue() {
    let rig = Rig::new("live-scan").await;
    rig.host
        .external
        .store(true, std::sync::atomic::Ordering::Relaxed);
    rig.link
        .bind(&json!({ "url": engine_url(), "engineId": "live" }));
    let s = rig.session();
    s.start(&rig.ws, &rig.project).await;
    let mut held = false;
    // A fresh home scans for about a minute and a half: the gate may let go much later than a load does.
    until(240, "the load to end", || {
        held |= s.load_phase() == Some(k7_file_service::engine::Phase::Scanning);
        s.load_phase().is_none()
    })
    .await;
    if held {
        assert!(
            !rig.host.catalogues.lock().unwrap().is_empty(),
            "a scan that ended wrote the catalogue"
        );
        assert!(
            rig.host
                .invalidated
                .load(std::sync::atomic::Ordering::Relaxed)
                >= 1
        );
    }
    eprintln!("scan gate held the load: {held}");
}

/// The engine binary started alone on `port`, silent, in a scratch home, its environment cleared.
fn start_engine(bin: &str, port: u16, home: &std::path::Path) -> std::process::Child {
    std::process::Command::new(bin)
        .arg(port.to_string())
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", home)
        .env("STUDIO_HOME", home.join(".k7"))
        .env("K7_SETTINGS_DIR", home.join("settings"))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        // Its startup lines (`[startup] audioDevice=…`) are what explains an engine that never serves.
        .stderr(match std::fs::File::create(home.join("engine.stderr")) {
            Ok(f) => std::process::Stdio::from(f),
            Err(_) => std::process::Stdio::null(),
        })
        .spawn()
        .expect("the engine binary starts")
}

/// An engine probed in its first seconds was seen not to serve afterwards (cause not found): it is left 20 s, and must live.
async fn settle(engine: &mut std::process::Child) {
    for _ in 0..40 {
        if let Ok(Some(status)) = engine.try_wait() {
            panic!("the engine exited while starting: {status:?}");
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

async fn listening(port: u16) {
    // A full handshake: the engine does not take a bare connect that closes at once.
    let url = format!("ws://127.0.0.1:{port}");
    let end = std::time::Instant::now() + Duration::from_secs(60);
    while tokio_tungstenite::connect_async(&url).await.is_err() {
        assert!(
            std::time::Instant::now() < end,
            "timed out waiting for the engine's port"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

#[tokio::test]
#[ignore = "needs K7_LIVE_ENGINE_BIN, the engine binary, which it starts and kills itself"]
async fn a_killed_engine_started_again_on_its_port_is_dialled_and_sent_its_project() {
    let bin =
        std::env::var("K7_LIVE_ENGINE_BIN").expect("K7_LIVE_ENGINE_BIN names the engine binary");
    // A port of the 38800s that nothing listens on: the engine is not started on one just probed and released.
    let port = (38800..38900)
        .find(|p| std::net::TcpStream::connect(("127.0.0.1", *p)).is_err())
        .expect("a free port");
    let home = crate::routes::files::service::native::tests::tmp_root()
        .join(format!("k7-live-home-{}", std::process::id()));
    std::fs::create_dir_all(&home).unwrap();
    let mut engine = start_engine(&bin, port, &home);
    settle(&mut engine).await;
    tokio::time::sleep(Duration::from_secs(3)).await;
    listening(port).await;
    let rig = Rig::new("live-kill").await;
    let t = tap(format!("ws://127.0.0.1:{port}")).await;
    rig.link.bind(&json!({ "url": t.url, "engineId": "live" }));
    let s = rig.session();
    s.start(&rig.ws, &rig.project).await;
    until(60, "the first load", || {
        s.load_phase().is_none() && kinds(&t).iter().filter(|k| *k == "project").count() >= 3
    })
    .await;
    let before = kinds(&t).iter().filter(|k| *k == "project").count();
    engine.kill().unwrap();
    engine.wait().unwrap();
    let mut again = start_engine(&bin, port, &home);
    settle(&mut again).await;
    listening(port).await;
    // The link's own backoff dials again, and the socket's open re-sends what the session serves.
    until(60, "the project sent again", || {
        kinds(&t).iter().filter(|k| *k == "project").count() > before
    })
    .await;
    let stats = rig.link.stats();
    assert_eq!(
        stats.redials.load(std::sync::atomic::Ordering::Relaxed),
        0,
        "the socket re-dialled itself"
    );
    again.kill().unwrap();
    again.wait().unwrap();
    let _ = std::fs::remove_dir_all(&home);
}
