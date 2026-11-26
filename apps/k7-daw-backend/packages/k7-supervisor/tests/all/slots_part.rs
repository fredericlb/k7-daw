//! slots_part.rs — the slots part inside the core, against `k7-ws-engine`: the enabled slot is sent at every
//! engine start and followed in the sessions (a project taking the engine over disables it), and the `slot.*`
//! commands change the store, tell the engine and answer.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use k7_supervisor::core::{self, Boot, Input};
use k7_supervisor::frames::Frames;
use k7_supervisor::parts::slots as part;
use k7_supervisor::paths::Paths;
use k7_supervisor::ports;
use k7_supervisor::protocol::Command;
use k7_supervisor::settings::Store;
use k7_supervisor::slots::{self, Installed};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::scratch::Scratch;

const ENGINE: &str = env!("CARGO_BIN_EXE_k7-ws-engine");

struct Bed {
    dir: Scratch,
    paths: Paths,
    control: PathBuf,
    tx: mpsc::UnboundedSender<Input>,
    frames: mpsc::UnboundedReceiver<String>,
    /// The last state frame seen, for what a command's answer overtook.
    state: Value,
    running: Option<JoinHandle<Result<(), String>>>,
    next_id: i64,
}

fn free_port() -> u16 {
    loop {
        let mut b = [0u8; 2];
        getrandom::fill(&mut b).unwrap();
        let port = 30_000 + u16::from_le_bytes(b) % 10_000;
        if ports::bindable(port) {
            return port;
        }
    }
}

fn a_slot(id: &str) -> Value {
    json!({"id": id, "name": "Warm", "chain": "@account/warm", "savedAt": "2026-11-01T18:00:00Z", "protocol": 7,
           "chainPlugins": ["IN:amp"], "doc": {"name": "warm", "tracks": []}})
}

impl Bed {
    /// A bed with these slots stored (the first enabled when `enable`), the core not yet started.
    fn prepare(stored: &[&str], enable: bool) -> Bed {
        part::set_pace(Duration::from_millis(80));
        let dir = Scratch::new("slots-part");
        let port = free_port();
        let paths = Paths::at(dir.path(), None);
        let control = std::env::temp_dir().join(format!("k7-fake-engine-{port}"));
        let _ = std::fs::remove_dir_all(&control);
        std::fs::write(paths.settings(), json!({"port": port}).to_string()).unwrap();
        for id in stored {
            let staged = dir.path().join("staged.json");
            std::fs::write(&staged, a_slot(id).to_string()).unwrap();
            slots::save(&paths.slots_dir(), &staged, &Installed::default()).unwrap();
        }
        if enable {
            slots::set_enabled(&paths.slots_dir(), Some(stored[0]));
        }
        let (_, frames) = mpsc::unbounded_channel();
        let (tx, _) = mpsc::unbounded_channel();
        Bed {
            dir,
            paths,
            control,
            tx,
            frames,
            state: Value::Null,
            running: None,
            next_id: 0,
        }
    }

    fn start(&mut self) {
        let (ftx, frames) = mpsc::unbounded_channel();
        let boot = Boot {
            store: Store::new(self.paths.settings()),
            paths: self.paths.clone(),
            frames: Frames::channel(ftx),
            engine_binary: Some(Path::new(ENGINE).into()),
            parts: vec![part::spec()],
        };
        let (tx, rx) = mpsc::unbounded_channel();
        self.frames = frames;
        self.tx = tx;
        self.running = Some(tokio::spawn(core::run(boot, rx)));
    }

    async fn until(&mut self, secs: u64, want: impl Fn(&Value) -> bool) -> Value {
        let end = Instant::now() + Duration::from_secs(secs);
        let mut last = Value::Null;
        loop {
            let left = end.saturating_duration_since(Instant::now());
            let Ok(Some(line)) = tokio::time::timeout(left, self.frames.recv()).await else {
                panic!("no such frame in {secs}s; last {last}");
            };
            let f: Value = serde_json::from_str(&line).unwrap();
            if f["t"] == "state" {
                self.state = f.clone();
            }
            if want(&f) {
                return f;
            }
            last = f;
        }
    }

    /// The state as it is now: the last one seen, or the next that satisfies `want`.
    async fn settled(&mut self, want: impl Fn(&Value) -> bool) {
        if !want(&self.state) {
            self.until(10, want).await;
        }
    }

    async fn ask(&mut self, cmd: &str, arg: &str) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        let c = Command {
            id,
            cmd: cmd.into(),
            arg: arg.into(),
        };
        self.tx.send(Input::Command(c)).unwrap();
        self.until(30, |f| f["t"] == "result" && f["id"] == id)
            .await
    }

    fn received(&self) -> Vec<Value> {
        std::fs::read_to_string(self.control.join("frames.ndjson"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }

    fn count(&self, ty: &str) -> usize {
        self.received().iter().filter(|f| f["type"] == ty).count()
    }

    async fn until_count(&self, ty: &str, n: usize) {
        let end = Instant::now() + Duration::from_secs(30);
        while self.count(ty) < n {
            assert!(
                Instant::now() < end,
                "never got {n} {ty}; got {:?}",
                self.received()
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    async fn finish(mut self) {
        self.tx.send(Input::Shutdown).unwrap();
        let _ = self.running.take().unwrap().await;
        let _ = std::fs::remove_dir_all(&self.control);
    }

    fn staged(&self, slot: &Value) -> String {
        let p = self.dir.path().join("incoming.json");
        std::fs::write(&p, slot.to_string()).unwrap();
        p.to_string_lossy().into_owned()
    }
}

fn running(f: &Value) -> bool {
    f["status"] == "running"
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_enabled_slot_is_sent_at_every_start_and_a_project_taking_over_disables_it() {
    let mut bed = Bed::prepare(&["warm"], true);
    bed.start();
    let f = bed
        .until(30, |f| running(f) && f["enabledSlot"] == "warm")
        .await;
    assert_eq!(f["slots"][0]["id"], "warm");
    assert_eq!(f["slots"][0]["chain"], "@account/warm");
    assert!(
        f["slots"][0].get("doc").is_none(),
        "the document is the engine's to read"
    );
    bed.until_count("project", 1).await;
    let sent = bed
        .received()
        .into_iter()
        .find(|f| f["type"] == "project")
        .unwrap();
    assert_eq!(
        (sent["session"].as_str(), sent["doc"]["name"].as_str()),
        (Some("slot"), Some("warm"))
    );
    // A relaunched engine finds it again.
    bed.ask("engine.stop", "").await;
    bed.ask("engine.start", "").await;
    bed.until_count("project", 2).await;
    // Followed: once a poll saw it loaded, its going from the sessions is a project taking the engine over.
    let seen = bed.count("list-sessions");
    let end = Instant::now() + Duration::from_secs(30);
    while bed.count("list-sessions") < seen + 2 {
        assert!(Instant::now() < end);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    std::fs::write(
        bed.control.join("state.json"),
        json!({"sessions": []}).to_string(),
    )
    .unwrap();
    bed.until(30, |f| {
        f["enabledSlot"].is_null() && f["slots"].as_array().is_some_and(|s| s.len() == 1)
    })
    .await;
    assert_eq!(slots::enabled(&bed.paths.slots_dir()), None);
    bed.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_commands_change_the_store_tell_the_engine_and_answer() {
    let mut bed = Bed::prepare(&[], false);
    bed.start();
    bed.until(30, running).await;
    let staged = bed.staged(&a_slot("warm"));
    let r = bed.ask("slot.save", &staged).await;
    assert_eq!(
        (r["ok"].clone(), r["message"].clone()),
        (json!(true), json!("warm"))
    );
    bed.until(10, |f| {
        f["slots"].as_array().is_some_and(|s| s.len() == 1) && f["enabledSlot"].is_null()
    })
    .await;
    assert_eq!(bed.ask("slot.save", "/no/such/file").await["ok"], false);
    assert_eq!(
        bed.ask("slot.enable", "ghost").await["message"],
        "no slot 'ghost'"
    );
    assert_eq!(bed.ask("slot.enable", "warm").await["ok"], true);
    bed.settled(|f| f["enabledSlot"] == "warm").await;
    bed.until_count("project", 1).await;
    // The pedal is bound, sent again at once, with the chain's plugins, and refused for what is no CC.
    assert_eq!(bed.ask("slot.footswitch", "warm\u{1f}64").await["ok"], true);
    bed.until_count("project", 2).await;
    let doc = bed
        .received()
        .into_iter()
        .rfind(|f| f["type"] == "project")
        .unwrap()["doc"]
        .clone();
    assert_eq!(doc["footswitch"], json!({"cc": 64, "plugins": ["IN:amp"]}));
    bed.settled(|f| f["slots"][0]["footswitch"] == 64).await;
    assert_eq!(
        bed.ask("slot.footswitch", "warm\u{1f}abc").await["message"],
        "a footswitch is a CC number, 0 to 127"
    );
    assert_eq!(
        bed.ask("slot.footswitch", "warm\u{1f}200").await["message"],
        "no slot 'warm', or no CC 200"
    );
    // Saved again while enabled, the new version is heard at once.
    let staged = bed.staged(&a_slot("warm"));
    assert_eq!(bed.ask("slot.save", &staged).await["ok"], true);
    bed.until_count("project", 3).await;
    // Disabled: the engine is told to let it go, and the answer waits for that.
    assert_eq!(bed.ask("slot.disable", "").await["ok"], true);
    bed.until_count("drop-session", 1).await;
    assert_eq!(
        bed.received()
            .iter()
            .find(|f| f["type"] == "drop-session")
            .unwrap()["session"],
        "slot"
    );
    bed.settled(|f| f["enabledSlot"].is_null()).await;
    assert_eq!(bed.ask("slot.remove", "warm").await["ok"], true);
    assert_eq!(
        bed.ask("slot.remove", "warm").await["message"],
        "no slot 'warm'"
    );
    bed.settled(|f| f["slots"] == json!([])).await;
    bed.finish().await;
}
