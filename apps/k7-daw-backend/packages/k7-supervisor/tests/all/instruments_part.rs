//! instruments_part.rs — the instruments part inside the core, over a catalogue of archives served on 127.0.0.1:
//! the first boot keeps what is already there and fetches the newer pin of it, one download runs at a time
//! while the rest wait in the order asked, progress and a failure ride the snapshot, the running one cannot be
//! removed, a queued one can, and a removal leaves the machine as if it had never kept the instrument.

use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use k7_supervisor::core::{self, Boot, Ctx, Event, Input, PartSpec};
use k7_supervisor::frames::Frames;
use k7_supervisor::instruments::Instrument;
use k7_supervisor::parts::instruments as part;
use k7_supervisor::paths::Paths;
use k7_supervisor::protocol::Command;
use k7_supervisor::settings::Store;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::sync::{Notify, mpsc};

use crate::blob_server::{Answer, serve_blobs};
use crate::scratch::Scratch;

static CATALOGUE: OnceLock<Vec<Instrument>> = OnceLock::new();

fn start(cx: Ctx, rx: mpsc::UnboundedReceiver<Event>) {
    let list = CATALOGUE.get().expect("a catalogue").clone();
    tokio::spawn(part::run(cx, rx, list, Duration::from_secs(30)));
}

fn archive(tag: &str) -> Vec<u8> {
    use std::io::Write;
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default();
    zip.start_file(format!("{tag}/{tag}.sfz"), opts).unwrap();
    zip.write_all(format!("<region> sample={tag}.wav").as_bytes())
        .unwrap();
    zip.finish().unwrap().into_inner()
}

fn instrument(name: &str, origin: &str, bytes: &[u8]) -> Instrument {
    Instrument {
        name: name.into(),
        label: format!("The {name}"),
        url: format!("{origin}/{name}.zip"),
        sha256: Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
        bytes: bytes.len() as u64,
    }
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

fn row<'a>(f: &'a Value, key: &str, name: &str) -> Option<&'a Value> {
    f[key].as_array()?.iter().find(|r| r["name"] == name)
}

fn cmd(id: i64, name: &str, arg: &str) -> Input {
    Input::Command(Command {
        id,
        cmd: name.into(),
        arg: arg.into(),
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_part_keeps_fetches_one_at_a_time_and_removes() {
    let gate = Arc::new(Notify::new());
    let held = gate.clone();
    let server = serve_blobs(move |a| match a.path.as_str() {
        "/old.zip" => Answer {
            gate: Some(held.clone()),
            ..Answer::ok(archive("old"))
        },
        "/a.zip" => Answer::ok(archive("a")),
        "/b.zip" => Answer::ok(archive("b")),
        // Not what its pin says.
        _ => Answer::ok(archive("changed")),
    })
    .await;
    let list = vec![
        instrument("old", &server.origin, &archive("old")),
        instrument("a", &server.origin, &archive("a")),
        instrument("b", &server.origin, &archive("b")),
        instrument("bad", &server.origin, &archive("bad")),
    ];
    CATALOGUE.set(list).unwrap();

    let dir = Scratch::new("ins-part");
    let paths = Paths::at(dir.path(), None);
    std::fs::write(
        paths.settings(),
        format!(r#"{{"port": {}}}"#, crate::supervisor::free_port()),
    )
    .unwrap();
    // An older archive of `old` is already here: a first boot keeps it, and fetches the newer pin of it.
    let root = paths.instruments_dir();
    std::fs::create_dir_all(root.join("old")).unwrap();
    std::fs::write(root.join("old/.k7-archive"), "an-older-hash\n").unwrap();

    let (ftx, mut frames) = mpsc::unbounded_channel();
    let boot = Boot {
        store: Store::new(paths.settings()),
        paths: paths.clone(),
        frames: Frames::channel(ftx),
        engine_binary: Some(env!("CARGO_BIN_EXE_k7-fake-engine").into()),
        parts: vec![PartSpec {
            name: "instruments",
            commands: &["instruments.download", "instruments.remove"],
            start,
        }],
    };
    let (tx, rx) = mpsc::unbounded_channel();
    let running = tokio::spawn(core::run(boot, rx));

    // The Downloads tab: only what was there is kept, and its newer pin is on its way.
    let first = until(&mut frames, |f| {
        row(f, "downloads", "old").is_some_and(|d| d["phase"] == "downloading")
    })
    .await;
    assert_eq!(first["instruments"].as_array().unwrap().len(), 4);
    assert_eq!(row(&first, "instruments", "old").unwrap()["wanted"], true);
    assert_eq!(
        row(&first, "instruments", "old").unwrap()["installed"],
        false
    );
    assert_eq!(row(&first, "instruments", "a").unwrap()["wanted"], false);
    assert!(row(&first, "downloads", "a").is_none());
    let settings = std::fs::read_to_string(paths.settings()).unwrap();
    assert!(
        settings.contains(r#""instruments": ["#) && settings.contains("old"),
        "{settings}"
    );

    // Asked in an order, waiting behind the one that runs; a queued one can be dropped, the running one not.
    tx.send(cmd(1, "instruments.download", "a")).unwrap();
    tx.send(cmd(2, "instruments.download", "b")).unwrap();
    tx.send(cmd(3, "instruments.remove", "b")).unwrap();
    tx.send(cmd(4, "instruments.remove", "old")).unwrap();
    tx.send(cmd(5, "instruments.download", "nothing")).unwrap();
    let mut answers = Vec::new();
    let mut waiting = None;
    while answers.len() < 5 || waiting.is_none() {
        let f = until(&mut frames, |f| {
            f["t"] == "result" || row(f, "downloads", "a").is_some()
        })
        .await;
        if f["t"] == "result" {
            answers.push((
                f["id"].as_i64().unwrap(),
                f["ok"].as_bool().unwrap(),
                f["message"].as_str().unwrap_or("").to_string(),
            ));
        } else if row(&f, "downloads", "a").is_some_and(|d| d["phase"] == "queued") {
            waiting = Some(f);
        }
    }
    answers.sort();
    assert_eq!(answers[0], (1, true, String::new()));
    assert_eq!(answers[1], (2, true, String::new()));
    assert_eq!(answers[2], (3, true, String::new()));
    assert!(
        !answers[3].1 && answers[3].2.contains("is downloading"),
        "{answers:?}"
    );
    assert!(
        !answers[4].1
            && answers[4]
                .2
                .contains("is not an instrument this engine plays"),
        "{answers:?}"
    );
    let phases: Vec<_> = waiting.unwrap()["downloads"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            (
                d["name"].as_str().unwrap().to_string(),
                d["phase"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(
        phases,
        [
            ("old".to_string(), "downloading".to_string()),
            ("a".to_string(), "queued".to_string())
        ]
    );

    // The held one lands, the next one runs and lands, and the list empties.
    gate.notify_one();
    let done = until(&mut frames, |f| {
        f.get("downloads").is_none()
            && row(f, "instruments", "a").is_some_and(|r| r["installed"] == true)
    })
    .await;
    assert_eq!(row(&done, "instruments", "old").unwrap()["installed"], true);
    assert!(root.join("old/old.sfz").is_file() && root.join("a/a.sfz").is_file());
    assert!(!root.join("b").exists());

    // A pin that no longer matches what its origin serves fails and says why, and stays until retried.
    tx.send(cmd(6, "instruments.download", "bad")).unwrap();
    let failed = until(&mut frames, |f| {
        row(f, "downloads", "bad").is_some_and(|d| d["phase"] == "failed")
    })
    .await;
    assert!(
        row(&failed, "downloads", "bad").unwrap()["error"]
            .as_str()
            .unwrap()
            .contains("changed upstream")
    );
    assert!(!root.join("bad").exists() && !root.join("bad.zip.part").exists());

    // Removing leaves the machine as if it never kept it.
    tx.send(cmd(7, "instruments.remove", "a")).unwrap();
    let removed = until(&mut frames, |f| f["t"] == "result" && f["id"] == 7).await;
    assert_eq!(removed["ok"], true);
    assert!(!root.join("a").exists());
    let after = until(&mut frames, |f| {
        row(f, "instruments", "a").is_some_and(|r| r["wanted"] == false && r["installed"] == false)
    })
    .await;
    assert_eq!(
        row(&after, "instruments", "old").unwrap()["installed"],
        true
    );
    let kept = std::fs::read_to_string(paths.settings()).unwrap();
    assert!(
        !kept.contains("\"a\"") && kept.contains("\"bad\"") && kept.contains("\"old\""),
        "{kept}"
    );

    tx.send(Input::Shutdown).unwrap();
    assert_eq!(running.await.unwrap(), Ok(()));
}
