//! ws_engine.rs — an engine that speaks the engine's WebSocket from files, for the queries part's tests:
//! `k7-ws-engine <port>` serves `$STUDIO_ENGINE_HOST:<port>`. Its answers are `state.json` in
//! `<temp>/k7-fake-engine-<port>/`, read at each request; every frame it receives is a line of `frames.ndjson`
//! there. A reply is preceded by two frames a client must ignore: a broadcast, and a same-typed one naming a session.

use std::io::Write;
use std::path::{Path, PathBuf};

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

fn dir(port: &str) -> PathBuf {
    std::env::temp_dir().join(format!("k7-fake-engine-{port}"))
}

fn read_state(dir: &Path) -> Value {
    std::fs::read_to_string(dir.join("state.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({}))
}

fn write_state(dir: &Path, state: &Value) {
    let tmp = dir.join("state.json.tmp");
    if std::fs::write(&tmp, state.to_string()).is_ok() {
        let _ = std::fs::rename(&tmp, dir.join("state.json"));
    }
}

fn rows(state: &Value, key: &str) -> Vec<Value> {
    state[key].as_array().cloned().unwrap_or_default()
}

fn same(row: &Value, key: &str, want: &Value) -> bool {
    &row[key] == want
}

/// The reply to `msg`, and the state it changes.
fn answer(msg: &Value, state: &mut Value) -> Option<Value> {
    let ty = msg["type"].as_str()?;
    match ty {
        "whoami" => {
            let mut who = state["whoami"].clone();
            if !who.is_object() {
                who = json!({"protocol": 1, "build": k7_supervisor::wire::build()});
            }
            who["type"] = json!("whoami");
            Some(who)
        }
        "list-audio-devices" => {
            Some(json!({"type": "audio-devices", "devices": rows(state, "devices")}))
        }
        "set-audio-device" => {
            if state["refuse"] != json!(true) {
                let mut devices = rows(state, "devices");
                for d in &mut devices {
                    d["current"] = json!(same(d, "name", &msg["name"]));
                }
                state["devices"] = json!(devices);
            }
            Some(json!({"type": "audio-devices", "devices": rows(state, "devices")}))
        }
        "list-plugins" => Some(json!({
            "type": "plugins", "plugins": rows(state, "plugins"), "scanning": state["scanning"] == json!(true),
        })),
        "list-edit-plugins" => {
            Some(json!({"type": "edit-plugins", "plugins": rows(state, "editPlugins")}))
        }
        "list-sessions" => Some(json!({"type": "sessions", "sessions": rows(state, "sessions")})),
        "drop-session" => {
            let kept: Vec<Value> = rows(state, "sessions")
                .into_iter()
                .filter(|s| !same(s, "id", &msg["session"]))
                .collect();
            state["sessions"] = json!(kept);
            None
        }
        "list-pair-requests" => {
            Some(json!({"type": "pair-requests", "requests": rows(state, "pairRequests")}))
        }
        "list-paired-devices" => {
            Some(json!({"type": "paired-devices", "devices": rows(state, "pairedDevices")}))
        }
        "pair-approve" | "pair-deny" => {
            let (mine, rest): (Vec<Value>, Vec<Value>) = rows(state, "pairRequests")
                .into_iter()
                .partition(|r| same(r, "requestId", &msg["requestId"]));
            state["pairRequests"] = json!(rest);
            if ty == "pair-approve" {
                let mut paired = rows(state, "pairedDevices");
                for r in mine {
                    paired.push(json!({"deviceId": format!("dev-{}", r["requestId"].as_str().unwrap_or("")), "name": r["name"], "created": 1, "lastSeen": 1}));
                }
                state["pairedDevices"] = json!(paired);
            }
            None
        }
        "unpair-device" => {
            let kept: Vec<Value> = rows(state, "pairedDevices")
                .into_iter()
                .filter(|d| !same(d, "deviceId", &msg["deviceId"]))
                .collect();
            state["pairedDevices"] = json!(kept);
            None
        }
        "project" => {
            // A document played in a session: the row the engine lists for it once it is loaded.
            let mut sessions: Vec<Value> = rows(state, "sessions")
                .into_iter()
                .filter(|r| !same(r, "id", &msg["session"]))
                .collect();
            sessions.push(json!({"id": msg["session"], "project": msg["doc"]["name"], "loaded": true, "playing": false, "tracks": 0, "listeners": 0}));
            state["sessions"] = json!(sessions);
            None
        }
        "rescan-plugins" => {
            state["rescanned"] = json!(true);
            None
        }
        _ => None,
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let port = std::env::args().nth(1).unwrap_or_default();
    let host = std::env::var("STUDIO_ENGINE_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let dir = dir(&port);
    let _ = std::fs::create_dir_all(&dir);
    let listener = TcpListener::bind(format!("{host}:{port}"))
        .await
        .expect("the engine's port");
    println!("fake engine on :{port}");
    let _ = std::io::stdout().flush();
    loop {
        let Ok((tcp, _)) = listener.accept().await else {
            continue;
        };
        let dir = dir.clone();
        tokio::spawn(async move {
            let Ok(mut ws) = accept_async(tcp).await else {
                return;
            };
            while let Some(Ok(Message::Text(t))) = ws.next().await {
                let Ok(msg) = serde_json::from_str::<Value>(t.as_str()) else {
                    continue;
                };
                if let Ok(mut log) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(dir.join("frames.ndjson"))
                {
                    let _ = writeln!(log, "{msg}");
                }
                let before = read_state(&dir);
                let mut state = before.clone();
                let reply = answer(&msg, &mut state);
                // Written only when a frame changed it, so a test's own write is never undone by a read.
                if state != before {
                    write_state(&dir, &state);
                }
                let Some(reply) = reply else { continue };
                let ty = reply["type"].clone();
                let news = [
                    json!({"type": "state", "tick": 1}),
                    json!({"type": ty, "session": "other", "devices": [], "plugins": [], "sessions": [], "requests": []}),
                    reply,
                ];
                for f in news {
                    if ws.send(Message::text(f.to_string())).await.is_err() {
                        return;
                    }
                }
            }
        });
    }
}
