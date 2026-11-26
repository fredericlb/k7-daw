//! parts.rs — what a part can count on, run in this process: its boot is awaited before the engine starts,
//! its slice of the snapshot rides every frame, its commands reach it, an engine-scoped slice goes with a stop,
//! and a settings change it asks for is saved by the core.

use std::time::{Duration, Instant};

use k7_supervisor::core::{self, Boot, Ctx, Event, Input, PartSpec};
use k7_supervisor::frames::Frames;
use k7_supervisor::paths::Paths;
use k7_supervisor::protocol::Command;
use k7_supervisor::settings::Store;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::scratch::Scratch;

fn echo(cx: Ctx, mut rx: mpsc::UnboundedReceiver<Event>) {
    tokio::spawn(async move {
        while let Some(ev) = rx.recv().await {
            match ev {
                Event::Boot(ack) => {
                    cx.patch(false, |s| s.registry_url = "http://127.0.0.1:1".into());
                    cx.set_origins("http://127.0.0.1:1".into());
                    let _ = ack.send(());
                }
                Event::EngineUp(up) => {
                    cx.patch(true, move |s| s.sessions_sig = format!("up:{}", up.port))
                }
                Event::Command(c) => {
                    cx.update_settings(|s| s.lan_open = true);
                    cx.reply(c.id, true, &c.arg);
                }
                Event::EngineDown { ack, .. } => {
                    if let Some(a) = ack {
                        let _ = a.send(());
                    }
                }
                Event::Shutdown(ack) => {
                    let _ = ack.send(());
                }
            }
        }
    });
}

async fn until(rx: &mut mpsc::UnboundedReceiver<String>, want: impl Fn(&Value) -> bool) -> Value {
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        let left = end.saturating_duration_since(Instant::now());
        let line = tokio::time::timeout(left, rx.recv())
            .await
            .expect("a frame in time")
            .expect("frames");
        let f: Value = serde_json::from_str(&line).unwrap();
        if want(&f) {
            return f;
        }
    }
}

fn cmd(id: i64, name: &str, arg: &str) -> Input {
    Input::Command(Command {
        id,
        cmd: name.into(),
        arg: arg.into(),
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_part_boots_patches_answers_and_is_cleared_by_a_stop() {
    let dir = Scratch::new("parts");
    let paths = Paths::at(dir.path(), None);
    std::fs::write(
        paths.settings(),
        format!(r#"{{"port": {}}}"#, crate::supervisor::free_port()),
    )
    .unwrap();
    let (ftx, mut frames) = mpsc::unbounded_channel();
    let boot = Boot {
        store: Store::new(paths.settings()),
        paths: paths.clone(),
        frames: Frames::channel(ftx),
        engine_binary: Some(env!("CARGO_BIN_EXE_k7-fake-engine").into()),
        parts: vec![PartSpec {
            name: "echo",
            commands: &["echo.say"],
            start: echo,
        }],
    };
    let (tx, rx) = mpsc::unbounded_channel();
    let running = tokio::spawn(core::run(boot, rx));
    let up = until(&mut frames, |f| {
        f["status"] == "running"
            && f["sessionsSig"]
                .as_str()
                .is_some_and(|s| s.starts_with("up:"))
    })
    .await;
    assert_eq!(up["registryUrl"], "http://127.0.0.1:1");
    tx.send(cmd(1, "echo.say", "hi")).unwrap();
    assert_eq!(
        until(&mut frames, |f| f["t"] == "result" && f["id"] == 1).await["message"],
        "hi"
    );
    until(&mut frames, |f| f["lanOpen"] == true).await;
    assert!(
        std::fs::read_to_string(paths.settings())
            .unwrap()
            .contains(r#""lanOpen": true"#)
    );
    tx.send(cmd(2, "engine.stop", "")).unwrap();
    let stopped = until(&mut frames, |f| f["status"] == "stopped").await;
    assert_eq!(
        (
            stopped["sessionsSig"].as_str(),
            stopped["registryUrl"].as_str()
        ),
        (Some(""), Some("http://127.0.0.1:1"))
    );
    assert_eq!(
        until(&mut frames, |f| f["t"] == "result" && f["id"] == 2).await["ok"],
        true
    );
    tx.send(Input::Shutdown).unwrap();
    assert_eq!(running.await.unwrap(), Ok(()));
}
