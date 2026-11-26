//! supervisor.rs — the program end to end against `k7-fake-engine`: it starts the engine and says so, relaunches
//! one killed under it, stops and starts it on command, refuses a command nobody answers, and on stdin's end
//! stops the engine and exits; from `--config` it writes no frame and leaves on SIGTERM.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use k7_supervisor::pidfile;
use serde_json::Value;

use crate::scratch::Scratch;

struct Run {
    child: Child,
    stdin: Option<ChildStdin>,
    frames: Receiver<Value>,
    home: std::path::PathBuf,
}

pub fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn start(dir: &Scratch, args: &[&str]) -> Run {
    let home = dir.path().join("state");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        home.join("engine-host.json"),
        format!(r#"{{"port": {}}}"#, free_port()),
    )
    .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_k7-supervisor"))
        .args(args)
        .env("STUDIO_HOME", &home)
        .env("STUDIO_ENGINE", env!("CARGO_BIN_EXE_k7-fake-engine"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let (tx, frames) = channel();
    let out = child.stdout.take().unwrap();
    std::thread::spawn(move || {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            let _ = tx.send(serde_json::from_str(&line).unwrap_or(Value::String(line)));
        }
    });
    let stdin = child.stdin.take();
    Run {
        child,
        stdin,
        frames,
        home,
    }
}

impl Run {
    /// The first frame `want` accepts within `secs`; panics with the last one seen otherwise.
    fn until(&self, secs: u64, want: impl Fn(&Value) -> bool) -> Value {
        let end = Instant::now() + Duration::from_secs(secs);
        let mut last = Value::Null;
        while let Some(left) = end.checked_duration_since(Instant::now()) {
            match self.frames.recv_timeout(left) {
                Ok(f) if want(&f) => return f,
                Ok(f) => last = f,
                Err(_) => break,
            }
        }
        panic!("no such frame in {secs}s; last {last}");
    }

    fn send(&mut self, id: i64, cmd: &str) {
        let line = format!("{{\"t\":\"cmd\",\"id\":{id},\"cmd\":\"{cmd}\"}}\n");
        self.stdin
            .as_mut()
            .unwrap()
            .write_all(line.as_bytes())
            .unwrap();
    }

    fn engine_pid(&self) -> u32 {
        pidfile::read(&self.home.join("engine.pid")).pid
    }

    fn exits_within(&mut self, secs: u64) -> bool {
        let end = Instant::now() + Duration::from_secs(secs);
        while Instant::now() < end {
            if self.child.try_wait().unwrap().is_some() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }
}

fn status(f: &Value) -> &str {
    f["status"].as_str().unwrap_or("")
}

fn result(id: i64) -> impl Fn(&Value) -> bool {
    move |f| f["t"] == "result" && f["id"] == id
}

#[cfg(unix)]
#[test]
fn it_runs_relaunches_stops_and_leaves_with_its_app() {
    let dir = Scratch::new("e2e");
    let mut run = start(&dir, &[]);
    let up = run.until(10, |f| status(f) == "running");
    assert_eq!(
        (
            up["ownsEngine"].as_bool(),
            up["engineId"].as_str().map(str::len)
        ),
        (Some(true), Some(36))
    );
    let first = run.engine_pid();
    assert!(first > 0 && pidfile::alive(first));
    pidfile::kill_tree(first);
    run.until(5, |f| f["error"] == "engine exited — restarting…");
    run.until(5, |f| status(f) == "running");
    let second = run.engine_pid();
    assert!(second != first && pidfile::alive(second));
    run.send(1, "engine.stop");
    assert_eq!(run.until(5, result(1))["ok"], true);
    assert!(!run.home.join("engine.pid").exists());
    run.send(2, "engine.start");
    assert_eq!(run.until(5, result(2))["ok"], true);
    let third = run.engine_pid();
    run.send(3, "nobody.answers");
    assert_eq!(
        run.until(5, result(3))["message"],
        "nobody.answers is not a supervisor command"
    );
    drop(run.stdin.take());
    assert!(run.exits_within(5), "the end of stdin ends the supervisor");
    std::thread::sleep(Duration::from_millis(200));
    assert!(!pidfile::alive(third), "and its engine");
    assert!(!run.home.join("engine-host.pid").exists());
}

#[cfg(unix)]
#[test]
fn from_a_config_file_it_writes_no_frame_and_leaves_on_sigterm() {
    let dir = Scratch::new("config");
    let config = dir.path().join("engine.json");
    std::fs::write(&config, format!(r#"{{"port": {}}}"#, free_port())).unwrap();
    let mut run = start(&dir, &["--config", config.to_str().unwrap()]);
    let end = Instant::now() + Duration::from_secs(10);
    while run.engine_pid() == 0 && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(50));
    }
    let engine = run.engine_pid();
    assert!(engine > 0);
    let log = run.home.join(".studio.log");
    let said = || {
        std::fs::read_to_string(&log)
            .unwrap_or_default()
            .contains("[engine] fake engine on :")
    };
    while !said() && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(said(), "the engine's output is in the merged log");
    // SAFETY: a signal to our own child, whose pid we hold.
    unsafe { libc::kill(run.child.id() as i32, libc::SIGTERM) };
    assert!(run.exits_within(5));
    assert!(run.frames.try_recv().is_err(), "no frame on stdout");
    assert!(!pidfile::alive(engine));
}
