//! host.rs — `apps/k7-daw-engine/k7-supervisor` as a child over NDJSON on stdio, kept running: relaunched on a 1 s→30 s backoff
//! reset after a 20 s run (`apps/k7-daw-backend/docs/supervisorHost.md`).
//! What a reader gets wrong: death is read from the PROCESS, never the pipe, since an engine may hold a copy of it.

use std::path::PathBuf;
use std::process::{ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Map, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Notify, mpsc, watch};

use crate::pidfile;
use crate::protocol::{Frame, command_line, js_text, parse_frame, snapshot_of};

/// The relaunch delays: the first, the ceiling, and the run after which the next crash starts over.
#[derive(Clone, Copy, Debug)]
pub struct Backoff {
    pub first: Duration,
    pub max: Duration,
    pub reset_after: Duration,
}

impl Default for Backoff {
    fn default() -> Self {
        Backoff {
            first: Duration::from_secs(1),
            max: Duration::from_secs(30),
            reset_after: Duration::from_secs(20),
        }
    }
}

/// A line for the log.
pub type LogFn = Box<dyn Fn(&str) + Send + Sync>;
/// A `notify` frame, as it arrived.
pub type NotifyFn = Box<dyn Fn(&Map<String, Value>) + Send + Sync>;

/// What the host tells its owner; each is called on the runtime's threads and must not block.
pub struct Events {
    /// The host's own lines and the supervisor's stderr.
    pub log: LogFn,
    /// A whole snapshot; its arrival IS the supervisor working, so it also ends whatever `down` said.
    pub snapshot: Box<dyn Fn(Map<String, Value>) + Send + Sync>,
    pub notify: NotifyFn,
    /// Down between launches with why; None once a snapshot arrives again.
    pub down: Box<dyn Fn(Option<String>) + Send + Sync>,
}

pub struct Options {
    pub bin: PathBuf,
    /// Set on the child over this process's own environment.
    pub env: Vec<(String, String)>,
    /// The supervisor's `engine.pid`, read at stop to kill an engine that outlived the teardown.
    pub engine_pid_file: Option<PathBuf>,
    pub backoff: Backoff,
}

/// One launched supervisor: its stdin's queue, the kill switch, and whether it has exited.
struct Running {
    stdin: Option<mpsc::UnboundedSender<String>>,
    writable: Arc<AtomicBool>,
    kill: Arc<Notify>,
    exited: watch::Receiver<bool>,
}

pub struct SupervisorHost {
    opts: Options,
    events: Events,
    next_id: AtomicU64,
    stopping: AtomicBool,
    running: Mutex<Option<Running>>,
    wake: Notify,
}

impl SupervisorHost {
    pub fn new(opts: Options, events: Events) -> Arc<Self> {
        Arc::new(SupervisorHost {
            opts,
            events,
            next_id: AtomicU64::new(1),
            stopping: AtomicBool::new(false),
            running: Mutex::new(None),
            wake: Notify::new(),
        })
    }

    fn log(&self, text: &str) {
        (self.events.log)(text);
    }

    /// Launches the supervisor and keeps it running until `stop`; must be called inside a tokio runtime.
    pub fn start(self: &Arc<Self>) {
        self.stopping.store(false, Ordering::SeqCst);
        let host = self.clone();
        tokio::spawn(async move { host.run().await });
    }

    async fn run(self: Arc<Self>) {
        let mut delay = self.opts.backoff.first;
        while !self.stopping.load(Ordering::SeqCst) {
            let started = Instant::now();
            if let Some(status) = self.launch().await {
                self.log(&format!("supervisor exited ({})", exit_text(status)));
            }
            *self.running.lock().unwrap_or_else(|p| p.into_inner()) = None;
            if self.stopping.load(Ordering::SeqCst) {
                return;
            }
            // One that ran a while and died is a one-off; one that dies at once is broken and would bury the log.
            if started.elapsed() > self.opts.backoff.reset_after {
                delay = self.opts.backoff.first;
            }
            (self.events.down)(Some("supervisor restarting…".into()));
            tokio::select! {
                _ = tokio::time::sleep(delay) => {}
                _ = self.wake.notified() => {}
            }
            delay = (delay * 2).min(self.opts.backoff.max);
        }
    }

    /// One run of the supervisor, to its exit; None when it could not be started.
    async fn launch(self: &Arc<Self>) -> Option<ExitStatus> {
        self.log(&format!("starting supervisor: {}", self.opts.bin.display()));
        let mut cmd = Command::new(&self.opts.bin);
        cmd.envs(self.opts.env.iter().map(|(k, v)| (k, v)))
            .env("STUDIO_HOST_PID", std::process::id().to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // No console of its own, and out of the console's Ctrl-C group: the supervisor's own job covers it.
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000 | 0x0000_0200);
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                self.log(&format!("could not start supervisor: {e}"));
                (self.events.down)(Some(format!("supervisor won't start: {e}")));
                return None;
            }
        };
        if let Some(pid) = child.id() {
            self.log(&format!("supervisor up (pid {pid})"));
        }
        let (exited_tx, exited) = watch::channel(false);
        let kill = Arc::new(Notify::new());
        let writable = Arc::new(AtomicBool::new(true));
        let stdin = child
            .stdin
            .take()
            .map(|pipe| self.writer(pipe, writable.clone()));
        *self.running.lock().unwrap_or_else(|p| p.into_inner()) = Some(Running {
            stdin,
            writable,
            kill: kill.clone(),
            exited,
        });
        // A stop that came while this one was spawning found nothing to stop: it is told now, by its pipe.
        if self.stopping.load(Ordering::SeqCst) {
            self.send("shutdown", None);
            if let Some(r) = self
                .running
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .as_mut()
            {
                r.stdin = None;
            }
        }
        self.readers(&mut child);
        let status = tokio::select! {
            s = child.wait() => s.ok(),
            _ = kill.notified() => {
                let _ = child.start_kill();
                child.wait().await.ok()
            }
        };
        let _ = exited_tx.send(true);
        status
    }

    /// The task writing queued lines to the supervisor's stdin; the pipe closes once the queue's sender is dropped.
    fn writer(
        self: &Arc<Self>,
        mut pipe: tokio::process::ChildStdin,
        writable: Arc<AtomicBool>,
    ) -> mpsc::UnboundedSender<String> {
        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        let host = self.clone();
        tokio::spawn(async move {
            while let Some(line) = rx.recv().await {
                if let Err(e) = pipe.write_all(line.as_bytes()).await {
                    // A write to a supervisor that just died: said, never thrown.
                    writable.store(false, Ordering::SeqCst);
                    host.log(&format!("supervisor stdin: {e}"));
                    return;
                }
            }
            let _ = pipe.shutdown().await;
        });
        tx
    }

    /// Stdout read as frames, stderr filed in the log, each line as it comes.
    fn readers(self: &Arc<Self>, child: &mut Child) {
        if let Some(out) = child.stdout.take() {
            let host = self.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(out).lines();
                while let Ok(Some(l)) = lines.next_line().await {
                    host.on_line(&l);
                }
            });
        }
        if let Some(err) = child.stderr.take() {
            let host = self.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(err).lines();
                while let Ok(Some(l)) = lines.next_line().await {
                    if !l.trim().is_empty() {
                        host.log(l.trim_end_matches('\r'));
                    }
                }
            });
        }
    }

    fn on_line(&self, raw: &str) {
        let raw = raw.trim_end_matches('\r');
        if raw.trim().is_empty() {
            return;
        }
        match parse_frame(raw) {
            None => self.log(raw),
            Some(Frame::State(f)) => (self.events.snapshot)(snapshot_of(&f)),
            Some(Frame::Notify(n)) => (self.events.notify)(&n),
            Some(Frame::Result(r)) => {
                if r.get("ok") != Some(&Value::Bool(true)) {
                    let said = r.get("message").filter(|m| !m.is_null()).or(r.get("id"));
                    self.log(&format!("command failed: {}", js_text(said)));
                }
            }
        }
    }

    /// Fire-and-forget; a reply comes back as a `result` frame, surfaced only on failure.
    pub fn send(&self, cmd: &str, arg: Option<&str>) {
        let guard = self.running.lock().unwrap_or_else(|p| p.into_inner());
        let queue = guard
            .as_ref()
            .filter(|r| r.writable.load(Ordering::SeqCst))
            .and_then(|r| r.stdin.as_ref());
        let Some(queue) = queue else {
            drop(guard);
            self.log(&format!("dropped `{cmd}` — supervisor is down"));
            return;
        };
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        if queue.send(command_line(id, cmd, arg)).is_err() {
            drop(guard);
            self.log(&format!("dropped `{cmd}` — supervisor is down"));
        }
    }

    /// `shutdown`, then the pipe closing; a supervisor still up after `grace` is killed, then an engine the pidfile proves.
    pub async fn stop(&self, grace: Duration) {
        self.stopping.store(true, Ordering::SeqCst);
        self.wake.notify_one();
        // Read BEFORE the teardown, which removes the pidfile.
        let pid_file = self.opts.engine_pid_file.clone();
        let engine = pid_file.as_deref().and_then(pidfile::read);
        let mut exited = None;
        let kill = {
            let mut guard = self.running.lock().unwrap_or_else(|p| p.into_inner());
            match guard.as_mut() {
                Some(r) if !*r.exited.borrow() => {
                    exited = Some(r.exited.clone());
                    Some(r.kill.clone())
                }
                _ => None,
            }
        };
        if let (Some(kill), Some(mut exited)) = (kill, exited) {
            self.send("shutdown", None);
            if let Some(r) = self
                .running
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .as_mut()
            {
                r.stdin = None;
            }
            let done = tokio::time::timeout(grace, exited.wait_for(|e| *e))
                .await
                .is_ok();
            if !done {
                // The supervisor alone: what it left running is the engine, which the backstop below reaches.
                kill.notify_one();
                let _ = exited.wait_for(|e| *e).await;
            }
        }
        let Some(engine) = engine else {
            return;
        };
        let log = |t: &str| self.log(t);
        let verdict = tokio::task::spawn_blocking(move || {
            for _ in 0..20 {
                if !pidfile::alive(engine.pid) {
                    return None;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            let live = pidfile::inspect(engine.pid);
            let same = pidfile::check(&engine, live.as_ref(), true) == pidfile::Check::Same;
            if same {
                pidfile::kill_tree(engine.pid);
            }
            Some((engine.pid, same))
        })
        .await
        .ok()
        .flatten();
        match verdict {
            None => {}
            Some((pid, false)) => log(&format!(
                "pid {pid} outlived the teardown, but the pidfile does not prove it is the engine — left alone"
            )),
            Some((pid, true)) => {
                log(&format!("engine {pid} outlived the teardown — killed"));
                if let Some(f) = pid_file {
                    let _ = std::fs::remove_file(f);
                }
            }
        }
    }
}

/// Node's `code ?? signal`: the exit code, or the signal's name.
fn exit_text(status: ExitStatus) -> String {
    if let Some(code) = status.code() {
        return code.to_string();
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            let name = match sig {
                1 => "SIGHUP",
                2 => "SIGINT",
                3 => "SIGQUIT",
                6 => "SIGABRT",
                9 => "SIGKILL",
                11 => "SIGSEGV",
                13 => "SIGPIPE",
                15 => "SIGTERM",
                _ => return format!("signal {sig}"),
            };
            return name.into();
        }
    }
    "null".into()
}
