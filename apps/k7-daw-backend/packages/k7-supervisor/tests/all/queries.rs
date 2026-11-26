//! queries.rs — the queries part inside the core, against `k7-ws-engine` (an engine that speaks the engine wire
//! from files): the polls fill the snapshot with the fixtures' rows and a stop empties it, the ranking
//! switches the interface once per device set, every command reaches the engine and answers, an engine
//! in safe mode names its suspect, a frozen one is killed, and an adopted one of another build is replaced once.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use k7_supervisor::core::{self, Boot, Input};
use k7_supervisor::frames::Frames;
use k7_supervisor::parts::queries::{self, Paces};
use k7_supervisor::paths::Paths;
use k7_supervisor::ports;
use k7_supervisor::protocol::Command;
use k7_supervisor::settings::Store;
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::scratch::Scratch;

const ENGINE: &str = env!("CARGO_BIN_EXE_k7-ws-engine");

fn fixture() -> Value {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/schema/fixtures");
    serde_json::from_str(&std::fs::read_to_string(format!("{dir}/state-full.json")).unwrap())
        .unwrap()
}

/// The fixture's rows as the engine would report them: the device's `deviceType` is `type` on that wire.
fn engine_state() -> Value {
    let full = fixture();
    let devices: Vec<Value> = full["audioDevices"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            let mut d = d.clone();
            d["type"] = d["deviceType"].take();
            d
        })
        .collect();
    json!({
        "devices": devices, "plugins": full["plugins"], "editPlugins": full["editPlugins"],
        "sessions": full["sessions"], "pairRequests": full["pairRequests"], "pairedDevices": full["pairedDevices"],
    })
}

struct Bed {
    _dir: Scratch,
    port: u16,
    paths: Paths,
    control: PathBuf,
    tx: mpsc::UnboundedSender<Input>,
    frames: mpsc::UnboundedReceiver<String>,
    running: Option<JoinHandle<Result<(), String>>>,
    next_id: i64,
}

/// A port from below the OS's ephemeral range, so another test's `bind(0)` never lands on a port an engine here is about to take.
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

/// The write the engine reads at its next request: whole or not at all.
fn put_state(control: &Path, state: &Value) {
    std::fs::create_dir_all(control).unwrap();
    let tmp = control.join("state.json.test");
    std::fs::write(&tmp, state.to_string()).unwrap();
    std::fs::rename(tmp, control.join("state.json")).unwrap();
}

impl Bed {
    /// A bed whose engine answers `state`, with these extra settings; no core runs yet.
    fn prepare(state: &Value, settings: Value) -> Bed {
        queries::set_paces(Paces::default().divided(25));
        let dir = Scratch::new("queries");
        let port = free_port();
        let paths = Paths::at(dir.path(), None);
        let control = std::env::temp_dir().join(format!("k7-fake-engine-{port}"));
        let _ = std::fs::remove_dir_all(&control);
        put_state(&control, state);
        let mut s = settings;
        s["port"] = json!(port);
        std::fs::write(paths.settings(), s.to_string()).unwrap();
        let (_, frames) = mpsc::unbounded_channel();
        let (tx, _) = mpsc::unbounded_channel();
        Bed {
            _dir: dir,
            port,
            paths,
            control,
            tx,
            frames,
            running: None,
            next_id: 0,
        }
    }

    /// The core with the queries part, its engine the program at `binary`.
    fn start(&mut self, binary: &Path) {
        let (ftx, frames) = mpsc::unbounded_channel();
        let boot = Boot {
            store: Store::new(self.paths.settings()),
            paths: self.paths.clone(),
            frames: Frames::channel(ftx),
            engine_binary: Some(binary.into()),
            parts: vec![queries::spec()],
        };
        let (tx, rx) = mpsc::unbounded_channel();
        self.frames = frames;
        self.tx = tx;
        self.running = Some(tokio::spawn(core::run(boot, rx)));
    }

    fn new(state: &Value, settings: Value) -> Bed {
        let mut bed = Bed::prepare(state, settings);
        bed.start(Path::new(ENGINE));
        bed
    }

    /// The first frame `want` accepts within `secs`, panicking with the last seen otherwise.
    async fn until(&mut self, secs: u64, want: impl Fn(&Value) -> bool) -> Value {
        let end = Instant::now() + Duration::from_secs(secs);
        let mut last = Value::Null;
        loop {
            let left = end.saturating_duration_since(Instant::now());
            let Ok(Some(line)) = tokio::time::timeout(left, self.frames.recv()).await else {
                panic!("no such frame in {secs}s; last {last}");
            };
            let f: Value = serde_json::from_str(&line).unwrap();
            if want(&f) {
                return f;
            }
            last = f;
        }
    }

    fn send(&mut self, cmd: &str, arg: &str) -> i64 {
        self.next_id += 1;
        let c = Command {
            id: self.next_id,
            cmd: cmd.into(),
            arg: arg.into(),
        };
        self.tx.send(Input::Command(c)).unwrap();
        self.next_id
    }

    /// A command's `result`.
    async fn ask(&mut self, cmd: &str, arg: &str) -> Value {
        let id = self.send(cmd, arg);
        self.until(30, |f| f["t"] == "result" && f["id"] == id)
            .await
    }

    /// What the engine has received so far.
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

    async fn engine_got(&self, want: impl Fn(&Value) -> bool) -> Value {
        let end = Instant::now() + Duration::from_secs(30);
        while Instant::now() < end {
            if let Some(f) = self.received().into_iter().find(&want) {
                return f;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("the engine never got it; got {:?}", self.received());
    }

    async fn finish(mut self) {
        self.tx.send(Input::Shutdown).unwrap();
        let _ = self.running.take().unwrap().await;
        let _ = std::fs::remove_dir_all(&self.control);
    }
}

fn running(f: &Value) -> bool {
    f["status"] == "running"
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_polls_fill_the_snapshot_with_the_fixtures_rows_and_a_stop_empties_it() {
    let mut bed = Bed::new(&engine_state(), json!({"lanOpen": true}));
    let full = fixture();
    let f = bed
        .until(10, |f| {
            running(f)
                && f["pairedDevices"] == full["pairedDevices"]
                && f["pluginCount"] == 1
                && f["sessions"] == full["sessions"]
                && f["editPlugins"] == full["editPlugins"]
                && f["pairRequests"] == full["pairRequests"]
                && f["audioDevices"] == full["audioDevices"]
        })
        .await;
    assert_eq!(f["plugins"], full["plugins"]);
    assert_eq!(f["pluginsSig"], "VST3\0Diva\n");
    assert_eq!(f["editPluginsSig"], "s1\0n4\n");
    assert_eq!(
        f["sessionsSig"],
        "s1\u{1f}@local/w/song\u{1f}1\u{1f}4\u{1f}2\n"
    );
    assert_eq!(
        f["audioMenuSig"],
        "Focusrite USB ASIO|ASIO|false|false\u{1f}Speakers (Focusrite USB Audio)|Windows Audio|true|true\u{1e}"
    );
    // The musician's setting is told at every start, before anything else is asked.
    assert_eq!(bed.received()[0], json!({"type": "lan-open", "open": true}));
    bed.send("engine.stop", "");
    let s = bed.until(30, |f| f["status"] == "stopped").await;
    assert_eq!(
        (
            &s["plugins"],
            &s["sessions"],
            &s["pairedDevices"],
            &s["pluginCount"],
            &s["audioMenuSig"]
        ),
        (&json!([]), &json!([]), &json!([]), &Value::Null, &json!(""))
    );
    // A stale answer arriving after the stop must not bring it back: wait out several polls.
    tokio::time::sleep(Duration::from_millis(400)).await;
    bed.send("engine.start", "");
    bed.until(30, |f| running(f) && f["pluginCount"] == 1).await;
    bed.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_ranking_switches_once_and_a_refusal_waits_for_the_device_set_to_move() {
    let mut state = engine_state();
    state["refuse"] = json!(true);
    let mut bed = Bed::new(
        &state,
        json!({"audioPriority": ["Focusrite USB ASIO", "Speakers (Focusrite USB Audio)"]}),
    );
    bed.engine_got(|f| f["type"] == "set-audio-device").await;
    assert_eq!(
        bed.received()
            .into_iter()
            .find(|f| f["type"] == "set-audio-device")
            .unwrap(),
        json!({"type": "set-audio-device", "name": "Focusrite USB ASIO", "deviceType": "ASIO"})
    );
    // The engine refused, so it is still where it was; polling on does not ask again.
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert_eq!(bed.count("set-audio-device"), 1);
    // A device appearing is a new set: asked again, and this time it takes.
    let mut state = engine_state();
    let mut devices = state["devices"].as_array().unwrap().clone();
    devices.push(
        json!({"name": "HDMI", "type": "CoreAudio", "current": false, "inputs": 0, "outputs": 2}),
    );
    state["devices"] = json!(devices);
    put_state(&bed.control, &state);
    bed.until(30, |f| {
        f["audioDevices"].as_array().is_some_and(|d| {
            d.iter()
                .any(|d| d["name"] == "Focusrite USB ASIO" && d["current"] == true)
        })
    })
    .await;
    assert_eq!(bed.count("set-audio-device"), 2);
    bed.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn audio_priority_is_remembered_then_switched_and_an_empty_ranking_ranks_nothing() {
    let mut bed = Bed::new(&engine_state(), json!({}));
    bed.until(30, |f| {
        running(f) && f["audioDevices"].as_array().is_some_and(|d| d.len() == 2)
    })
    .await;
    assert_eq!(
        bed.count("set-audio-device"),
        0,
        "an empty ranking never switches"
    );
    let r = bed
        .ask(
            "audio.priority",
            "Focusrite USB ASIO\u{1f}Speakers (Focusrite USB Audio)\u{1f}Focusrite USB ASIO",
        )
        .await;
    assert_eq!(r["ok"], true);
    let saved: Value =
        serde_json::from_str(&std::fs::read_to_string(bed.paths.settings()).unwrap()).unwrap();
    assert_eq!(
        saved["audioPriority"],
        json!(["Focusrite USB ASIO", "Speakers (Focusrite USB Audio)"])
    );
    bed.engine_got(|f| f["type"] == "set-audio-device" && f["name"] == "Focusrite USB ASIO")
        .await;
    let f = bed
        .until(10, |f| {
            f["audioPriority"].as_array().is_some_and(|p| p.len() == 2)
                && f["audioDevices"].as_array().is_some_and(|d| {
                    d.iter()
                        .any(|d| d["name"] == "Focusrite USB ASIO" && d["current"] == true)
                })
        })
        .await;
    assert!(
        f["audioMenuSig"]
            .as_str()
            .unwrap()
            .ends_with("\u{1e}Focusrite USB ASIO\u{1f}Speakers (Focusrite USB Audio)")
    );
    assert_eq!(bed.ask("audio.priority", "").await["ok"], true);
    let saved: Value =
        serde_json::from_str(&std::fs::read_to_string(bed.paths.settings()).unwrap()).unwrap();
    assert_eq!(saved["audioPriority"], json!([]));
    bed.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_command_reaches_the_engine_and_answers() {
    let mut bed = Bed::new(&engine_state(), json!({}));
    bed.until(30, |f| {
        running(f)
            && f["pluginCount"] == 1
            && f["pairedDevices"].as_array().is_some_and(|d| !d.is_empty())
    })
    .await;
    assert_eq!(bed.ask("lan.open", "1").await["ok"], true);
    bed.engine_got(|f| *f == json!({"type": "lan-open", "open": true}))
        .await;
    assert!(
        std::fs::read_to_string(bed.paths.settings())
            .unwrap()
            .contains("\"lanOpen\": true")
    );
    bed.until(30, |f| f["lanOpen"] == true).await;
    assert_eq!(bed.ask("plugin.showUI", "s1\u{1f}n4").await["ok"], true);
    bed.engine_got(|f| *f == json!({"type": "show-native-plugin-ui", "id": "n4", "session": "s1"}))
        .await;
    assert_eq!(bed.ask("plugin.showUI", "n9").await["ok"], true);
    bed.engine_got(|f| *f == json!({"type": "show-native-plugin-ui", "id": "n9"}))
        .await;
    assert_eq!(bed.ask("session.drop", "s1").await["ok"], true);
    bed.until(30, |f| f["sessions"] == json!([])).await;
    assert_eq!(bed.ask("pair.approve", "r1").await["ok"], true);
    let f = bed
        .until(10, |f| {
            f["pairRequests"] == json!([])
                && f["pairedDevices"].as_array().is_some_and(|d| d.len() == 2)
        })
        .await;
    assert_eq!(f["pairedDevices"][1]["deviceId"], "dev-r1");
    assert_eq!(bed.ask("pair.revoke", "d1").await["ok"], true);
    bed.until(30, |f| {
        f["pairedDevices"].as_array().is_some_and(|d| d.len() == 1)
    })
    .await;
    assert_eq!(bed.ask("pair.deny", "nobody").await["ok"], true);
    // The scan cache goes first, the spinner shows at once, and the count comes back through the poll.
    std::fs::write(bed.paths.exts_cache(), "{}").unwrap();
    assert_eq!(bed.ask("plugins.rescan", "").await["ok"], true);
    assert!(!bed.paths.exts_cache().exists());
    bed.engine_got(|f| *f == json!({"type": "rescan-plugins", "force": true}))
        .await;
    bed.until(30, |f| f["pluginCount"].is_null()).await;
    bed.until(30, |f| f["pluginCount"] == 1).await;
    bed.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_safe_engine_names_its_suspect_and_a_frozen_one_is_killed_to_be_relaunched() {
    let build = k7_supervisor::wire::build();
    let who = |age: u64| {
        json!({"whoami": {"protocol": 1, "build": build, "messageThreadMs": age,
        "safeMode": {"crashes": 4, "diedLoading": "", "lastPlugin": "Diva.vst3"}}})
    };
    let mut state = engine_state();
    state["whoami"] = who(1000)["whoami"].clone();
    let mut bed = Bed::new(&state, json!({}));
    let f = bed
        .until(10, |f| running(f) && f["safeMode"].is_object())
        .await;
    assert_eq!(f["safeMode"], json!({"crashes": 4, "suspect": "Diva.vst3"}));
    let first = std::fs::read_to_string(bed.paths.engine_pid()).unwrap();
    // Several hang checks pass on an engine whose thread is alive.
    tokio::time::sleep(Duration::from_millis(1300)).await;
    assert_eq!(
        std::fs::read_to_string(bed.paths.engine_pid()).unwrap(),
        first
    );
    state["whoami"] = who(95_000)["whoami"].clone();
    put_state(&bed.control, &state);
    bed.until(30, |f| f["error"] == "engine exited — restarting…")
        .await;
    bed.until(30, running).await;
    assert_ne!(
        std::fs::read_to_string(bed.paths.engine_pid()).unwrap(),
        first
    );
    bed.finish().await;
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_adopted_engine_of_another_build_is_replaced_once() {
    // Named as the engine is, or the supervisor will not take the process on the port for it.
    let dir = Scratch::new("adopted-engine");
    let engine = dir.path().join("k7-runtime");
    std::fs::copy(ENGINE, &engine).unwrap();
    let mut state = engine_state();
    state["whoami"] = json!({"protocol": 99, "build": "another"});
    let mut bed = Bed::prepare(&state, json!({}));
    let mut old = std::process::Command::new(&engine)
        .arg(bed.port.to_string())
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let up = Instant::now() + Duration::from_secs(10);
    while !k7_supervisor::ports::is_serving(bed.port) && Instant::now() < up {
        std::thread::sleep(Duration::from_millis(20));
    }
    bed.start(&engine);
    let f = bed
        .until(10, |f| {
            f["error"] == "replacing the engine already running…"
        })
        .await;
    assert_eq!(f["ownsEngine"], false);
    bed.until(15, |f| running(f) && f["ownsEngine"] == true)
        .await;
    // The new engine reports the same foreign build, but it is ours now: no second replacement.
    tokio::time::sleep(Duration::from_millis(800)).await;
    assert!(
        old.try_wait().unwrap().is_some(),
        "the engine it found was stopped"
    );
    let _ = old.kill();
    bed.finish().await;
}
