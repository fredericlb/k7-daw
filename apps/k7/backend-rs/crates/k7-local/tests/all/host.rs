//! The supervisor host against `k7-fake-supervisor`: snapshots and notes handed up, a crash relaunched and a stop
//! not, a dead stdin survived, death read from the process while an engine holds its pipe, and the pidfile proof.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use k7_local::engine::{Config, LocalEngine};
use k7_local::{Backoff, pidfile};

const FAKE: &str = env!("CARGO_BIN_EXE_k7-fake-supervisor");

fn home(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("k7-local-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn quick() -> Backoff {
    Backoff {
        first: Duration::from_millis(50),
        max: Duration::from_millis(200),
        reset_after: Duration::from_secs(10),
    }
}

fn engine_at(home: &Path, env: Vec<(String, String)>) -> Arc<LocalEngine> {
    LocalEngine::new(Config {
        bin: FAKE.into(),
        home: home.to_path_buf(),
        registry: "http://127.0.0.1:18789".into(),
        roots: vec![home.join("files"), home.join("mediatek")],
        env,
        backoff: quick(),
        stop_grace: Duration::from_secs(1),
    })
}

fn log_of(home: &Path) -> String {
    std::fs::read_to_string(home.join(".studio.log")).unwrap_or_default()
}

/// Polls `read` for up to 15 s: a spawned process under a full test run can be slow, which is not what is tested.
async fn until<T>(mut read: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(v) = read() {
            return v;
        }
        assert!(Instant::now() < deadline, "timed out");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// The engine pid the fake supervisor reported in its `engine <pid>` note.
fn engine_pid(home: &Path) -> Option<u32> {
    log_of(home).lines().find_map(|l| {
        l.split("fake: engine ")
            .nth(1)
            .and_then(|p| p.trim().parse().ok())
    })
}

#[tokio::test]
async fn snapshots_notes_and_the_host_pid_reach_the_owner() {
    let h = home("snap");
    let engine = engine_at(&h, vec![]);
    engine.start();
    let s = until(|| engine.state().snapshot).await;
    assert_eq!(s["status"], "running");
    assert_eq!(s["hostPid"], std::process::id().to_string());
    assert_eq!(s["registryUrl"], "http://127.0.0.1:18789");
    let sep = if cfg!(windows) { ";" } else { ":" };
    let roots = format!(
        "{}{sep}{}",
        h.join("files").display(),
        h.join("mediatek").display()
    );
    assert_eq!(s["engineRoots"], roots.as_str());
    assert!(!s.contains_key("pluginCount") && !s.contains_key("owns"));
    engine.send("notify", Some("there"));
    engine.send("fail", None);
    until(|| log_of(&h).contains("command failed: no").then_some(())).await;
    let log = log_of(&h);
    assert!(log.contains("[host] fake: there"), "{log}");
    assert!(log.contains("[host] fake supervisor says hello"), "{log}");
    assert!(log.contains("[host] not a frame"), "{log}");
    let pinned = std::fs::read_to_string(h.join("engine-host.json")).unwrap();
    assert_eq!(
        pinned,
        "{\n  \"registryUrls\": [\n    \"http://127.0.0.1:18789\"\n  ]\n}"
    );
    engine.stop().await;
    assert!(log_of(&h).contains("supervisor exited (0)"));
}

#[tokio::test]
async fn a_crash_is_relaunched_and_a_stop_is_not() {
    let h = home("crash");
    let engine = engine_at(&h, vec![]);
    let mut rx = engine.subscribe();
    let downs = Arc::new(Mutex::new(Vec::new()));
    let seen = downs.clone();
    tokio::spawn(async move {
        while rx.changed().await.is_ok() {
            let d = rx.borrow().down.clone();
            seen.lock().unwrap().push(d);
        }
    });
    engine.start();
    until(|| engine.state().snapshot).await;
    engine.send("crash", None);
    until(|| (log_of(&h).matches("supervisor up").count() >= 2).then_some(())).await;
    until(|| engine.state().down.is_none().then_some(())).await;
    assert!(
        downs
            .lock()
            .unwrap()
            .iter()
            .any(|d| d.as_deref() == Some("supervisor restarting…"))
    );
    engine.stop().await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(log_of(&h).matches("supervisor up").count(), 2);
    engine.send("notify", Some("late"));
    assert!(log_of(&h).contains("dropped `notify` — supervisor is down"));
}

#[tokio::test]
async fn a_supervisor_whose_stdin_is_gone_is_written_to_without_harm() {
    let h = home("closein");
    let engine = engine_at(&h, vec![]);
    engine.start();
    until(|| engine.state().snapshot).await;
    engine.send("closein", None);
    until(|| log_of(&h).contains("fake: stdin closed").then_some(())).await;
    let big = "x".repeat(65_536);
    for _ in 0..20 {
        engine.send("notify", Some(&big));
    }
    until(|| log_of(&h).contains("supervisor stdin:").then_some(())).await;
    engine.stop().await;
}

/// The fake copied as `k7-runtime`, the one name the pidfile proof accepts.
fn fake_engine(h: &Path) -> PathBuf {
    let to = h.join(if cfg!(windows) {
        "k7-runtime.exe"
    } else {
        "k7-runtime"
    });
    std::fs::copy(FAKE, &to).unwrap();
    to
}

/// A hung supervisor that started an engine: started, the pidfile written as the supervisor would, then stopped.
async fn hung_with_engine(name: &str, proved: bool) -> (PathBuf, u32) {
    let h = home(name);
    let bin = fake_engine(&h);
    let engine = engine_at(
        &h,
        vec![("K7_FAKE_ENGINE".into(), bin.display().to_string())],
    );
    engine.start();
    until(|| engine.state().snapshot).await;
    engine.send("engine", None);
    let pid = until(|| engine_pid(&h)).await;
    let live = until(|| pidfile::inspect(pid)).await;
    let start = if proved {
        live.start.unwrap()
    } else {
        "1".into()
    };
    let text = format!("{pid}\nstart={start}\nexe={}\n", live.exe.unwrap());
    std::fs::write(h.join("engine.pid"), text).unwrap();
    engine.send("hang", None);
    engine.stop().await;
    (h, pid)
}

#[tokio::test]
async fn a_hung_supervisor_is_killed_and_its_proved_engine_with_it() {
    let (h, pid) = hung_with_engine("proved", true).await;
    assert!(
        log_of(&h).contains(&format!("engine {pid} outlived the teardown — killed")),
        "{}",
        log_of(&h)
    );
    until(|| (!pidfile::alive(pid)).then_some(())).await;
    assert!(!h.join("engine.pid").exists());
}

#[tokio::test]
async fn an_engine_the_pidfile_does_not_prove_is_left_alone() {
    let (h, pid) = hung_with_engine("unproved", false).await;
    assert!(
        log_of(&h).contains("does not prove it is the engine — left alone"),
        "{}",
        log_of(&h)
    );
    assert!(pidfile::alive(pid));
    pidfile::kill_tree(pid);
}

#[tokio::test]
async fn death_is_read_from_the_process_while_an_engine_holds_its_pipe() {
    let h = home("pipe");
    let bin = fake_engine(&h);
    let engine = engine_at(
        &h,
        vec![("K7_FAKE_ENGINE".into(), bin.display().to_string())],
    );
    engine.start();
    until(|| engine.state().snapshot).await;
    engine.send("engine", None);
    let pid = until(|| engine_pid(&h)).await;
    engine.send("crash", None);
    until(|| log_of(&h).contains("supervisor exited (3)").then_some(())).await;
    until(|| (log_of(&h).matches("supervisor up").count() >= 2).then_some(())).await;
    engine.stop().await;
    pidfile::kill_tree(pid);
}

#[tokio::test]
async fn a_supervisor_that_will_not_start_is_said_and_retried() {
    let h = home("missing");
    let engine = LocalEngine::new(Config {
        bin: h.join("no-such-supervisor"),
        home: h.clone(),
        registry: "http://127.0.0.1:18789".into(),
        roots: vec![],
        env: vec![],
        backoff: quick(),
        stop_grace: Duration::from_secs(1),
    });
    engine.start();
    until(|| (log_of(&h).matches("could not start supervisor").count() >= 2).then_some(())).await;
    assert!(engine.state().down.is_some());
    engine.stop().await;
}
