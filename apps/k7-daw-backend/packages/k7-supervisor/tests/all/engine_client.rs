//! engine_client.rs — the one-connection client against a scripted engine on 127.0.0.1: a reply is found among
//! the broadcasts and another session's news, one call is in flight, a deadline redials, a close gives up at
//! once, a new port is dialed, and every frame sent is the engine's own spelling.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use k7_supervisor::engine_client::EngineClient;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

/// What the script does for a frame: send a frame, or wait before the next.
enum Out {
    Send(Value),
    Wait(u64),
}

type Script = Arc<dyn Fn(usize, &Value) -> Vec<Out> + Send + Sync>;

struct Engine {
    port: u16,
    /// Every frame received, in order.
    got: Arc<Mutex<Vec<Value>>>,
    connections: Arc<AtomicUsize>,
    /// The most calls ever waiting for a reply at once.
    deepest: Arc<AtomicUsize>,
}

async fn engine(script: Script) -> Engine {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let e = Engine {
        port,
        got: Arc::default(),
        connections: Arc::default(),
        deepest: Arc::default(),
    };
    let (got, connections, deepest) = (e.got.clone(), e.connections.clone(), e.deepest.clone());
    tokio::spawn(async move {
        let busy = Arc::new(AtomicUsize::new(0));
        while let Ok((tcp, _)) = listener.accept().await {
            let n = connections.fetch_add(1, Ordering::SeqCst) + 1;
            let (script, got, deepest, busy) =
                (script.clone(), got.clone(), deepest.clone(), busy.clone());
            tokio::spawn(async move {
                let Ok(mut ws) = accept_async(tcp).await else {
                    return;
                };
                while let Some(Ok(Message::Text(t))) = ws.next().await {
                    let msg: Value = serde_json::from_str(t.as_str()).unwrap();
                    got.lock().unwrap().push(msg.clone());
                    let now = busy.fetch_add(1, Ordering::SeqCst) + 1;
                    deepest.fetch_max(now, Ordering::SeqCst);
                    for out in script(n, &msg) {
                        match out {
                            Out::Send(f) => {
                                let _ = ws.send(Message::text(f.to_string())).await;
                            }
                            Out::Wait(ms) => tokio::time::sleep(Duration::from_millis(ms)).await,
                        }
                    }
                    busy.fetch_sub(1, Ordering::SeqCst);
                }
            });
        }
    });
    e
}

fn devices(names: &[&str], current: &str) -> Value {
    let rows: Vec<Value> = names
        .iter()
        .map(|n| json!({"name": n, "type": "CoreAudio", "current": *n == current, "inputs": 2, "outputs": 2}))
        .collect();
    json!({"type": "audio-devices", "devices": rows})
}

#[tokio::test]
async fn a_reply_is_found_among_broadcasts_and_other_sessions_news() {
    let e = engine(Arc::new(|_, m| match m["type"].as_str() {
        Some("list-sessions") => vec![
            Out::Send(json!({"type": "state", "meters": [0.1]})),
            Out::Send(json!({"type": "sessions", "session": "s9", "sessions": []})),
            Out::Send(json!({"type": "sessions", "sessions": [
                {"id": "a", "project": "p", "loaded": true, "playing": false, "tracks": 3.0, "listeners": 1}]})),
        ],
        Some("list-plugins") => vec![Out::Send(json!({"type": "plugins", "scanning": true, "plugins": [
            {"name": "zeta", "format": "VST3", "manufacturer": "m", "role": "effect"},
            {"name": "Alpha", "format": "VST3", "manufacturer": "m", "role": "instrument"},
            {"name": "alpha", "format": "AU", "manufacturer": "m", "role": "instrument"}]}))],
        _ => vec![],
    }))
    .await;
    let c = EngineClient::new(e.port);
    let sessions = c.list_sessions().await.expect("the sessions");
    assert_eq!((sessions.len(), sessions[0].tracks), (1, 3));
    let cat = c.list_plugins().await.expect("the catalogue");
    let names: Vec<_> = cat
        .plugins
        .iter()
        .map(|p| (p.name.as_str(), p.format.as_str()))
        .collect();
    assert_eq!(
        names,
        [("alpha", "AU"), ("Alpha", "VST3"), ("zeta", "VST3")]
    );
    assert!(cat.scanning);
    assert_eq!(
        e.connections.load(Ordering::SeqCst),
        1,
        "one connection for both"
    );
}

#[tokio::test]
async fn one_call_is_in_flight_and_each_gets_its_own_reply() {
    let e = engine(Arc::new(|_, m| match m["type"].as_str() {
        Some("set-audio-device") => vec![Out::Wait(150), Out::Send(devices(&["A", "B"], "B"))],
        Some("list-audio-devices") => vec![Out::Send(devices(&["A", "B"], "A"))],
        _ => vec![],
    }))
    .await;
    let c = EngineClient::new(e.port);
    // Both want an `audio-devices` frame: on one socket the second must not take the first's.
    let (set, list) = tokio::join!(c.set_audio_device("B", "CoreAudio"), async {
        tokio::time::sleep(Duration::from_millis(30)).await;
        c.list_audio_devices().await
    });
    assert_eq!(set.unwrap().iter().find(|d| d.current).unwrap().name, "B");
    assert_eq!(list.unwrap().iter().find(|d| d.current).unwrap().name, "A");
    assert_eq!(e.deepest.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_deadline_forces_a_redial() {
    let e = engine(Arc::new(|n, m| match (n, m["type"].as_str()) {
        (1, _) => vec![],
        (_, Some("whoami")) => vec![Out::Send(json!({"type": "whoami", "protocol": 1}))],
        _ => vec![],
    }))
    .await;
    let c = EngineClient::new(e.port);
    let silent = c
        .request(
            json!({"type": "whoami"}),
            "whoami",
            Duration::from_millis(150),
        )
        .await;
    assert_eq!(silent, None);
    assert_eq!(c.whoami().await.unwrap()["protocol"], 1);
    assert_eq!(e.connections.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn a_close_ends_the_call_in_flight_at_once_and_the_next_call_dials_again() {
    let e = engine(Arc::new(|n, m| match (n, m["type"].as_str()) {
        (1, _) => vec![],
        (_, Some("whoami")) => vec![Out::Send(json!({"type": "whoami", "protocol": 1}))],
        _ => vec![],
    }))
    .await;
    let c = EngineClient::new(e.port);
    let waiting = {
        let c = c.clone();
        tokio::spawn(async move {
            let t = Instant::now();
            let r = c
                .request(json!({"type": "whoami"}), "whoami", Duration::from_secs(20))
                .await;
            (r, t.elapsed())
        })
    };
    tokio::time::sleep(Duration::from_millis(150)).await;
    c.close();
    let (r, took) = waiting.await.unwrap();
    assert!(
        r.is_none() && took < Duration::from_secs(3),
        "gave up after {took:?}"
    );
    assert!(c.whoami().await.is_some());
    assert_eq!(e.connections.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn nothing_listening_is_no_answer_and_a_new_port_is_dialed() {
    let a = engine(Arc::new(|_, _| {
        vec![Out::Send(
            json!({"type": "whoami", "protocol": 1, "from": "a"}),
        )]
    }))
    .await;
    let b = engine(Arc::new(|_, _| {
        vec![Out::Send(
            json!({"type": "whoami", "protocol": 1, "from": "b"}),
        )]
    }))
    .await;
    let free = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let c = EngineClient::new(free);
    assert!(c.whoami().await.is_none());
    assert!(!c.set_lan_open(true).await);
    c.set_port(a.port);
    assert_eq!(c.whoami().await.unwrap()["from"], "a");
    c.set_port(b.port);
    assert_eq!(c.whoami().await.unwrap()["from"], "b");
}

#[tokio::test]
async fn the_frames_sent_are_the_engines_own_spelling() {
    let e = engine(Arc::new(|_, m| match m["type"].as_str() {
        Some("set-audio-device") => vec![Out::Send(devices(&["A"], "A"))],
        _ => vec![],
    }))
    .await;
    let c = EngineClient::new(e.port);
    c.set_audio_device("A", "").await.unwrap();
    c.set_audio_device("A", "ASIO").await.unwrap();
    assert!(c.set_lan_open(true).await);
    assert!(c.drop_session("s1").await);
    assert!(c.show_native_plugin_ui("n4", Some("s1")).await);
    assert!(c.show_native_plugin_ui("n4", None).await);
    assert!(c.pair_verb("pair-approve", "r1").await);
    assert!(c.pair_verb("pair-deny", "r2").await);
    assert!(c.pair_verb("unpair-device", "d1").await);
    assert!(c.rescan_plugins().await);
    assert!(c.apply_project("slot", &json!({"tracks": []})).await);
    // A send is a write: let the script's task read the last one before the list is compared.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let want = [
        json!({"type": "set-audio-device", "name": "A"}),
        json!({"type": "set-audio-device", "name": "A", "deviceType": "ASIO"}),
        json!({"type": "lan-open", "open": true}),
        json!({"type": "drop-session", "session": "s1"}),
        json!({"type": "show-native-plugin-ui", "id": "n4", "session": "s1"}),
        json!({"type": "show-native-plugin-ui", "id": "n4"}),
        json!({"type": "pair-approve", "requestId": "r1"}),
        json!({"type": "pair-deny", "requestId": "r2"}),
        json!({"type": "unpair-device", "deviceId": "d1"}),
        json!({"type": "rescan-plugins", "force": true}),
        json!({"type": "project", "session": "slot", "doc": {"tracks": []}}),
    ];
    assert_eq!(*e.got.lock().unwrap(), want);
}
