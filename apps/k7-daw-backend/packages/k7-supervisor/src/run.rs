//! run.rs — a whole supervisor run, three ways: driven by an app over stdio (stdin's end IS the app gone,
//! and ends us), inside an app over two pipes it holds (iOS, where no program may run), or from `--config`,
//! with no frame on stdout, until SIGINT or SIGTERM. Each builds its runtime and hands the core its inputs.

use std::io::BufRead;
use std::path::PathBuf;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;

use crate::core::{self, Boot, Input};
use crate::frames::Frames;
use crate::log;
use crate::parts;
use crate::paths::Paths;
use crate::protocol::parse_command;
use crate::settings::{Store, config_environment};

/// How the run is driven.
pub enum Mode {
    /// An app on our stdio.
    Stdio,
    /// `--config <file>`, `--port <n>` when given.
    Config { file: PathBuf, port: Option<u16> },
}

/// A panic says why in the merged log before the default hook prints it: an app restarts us before anyone looks.
pub fn log_panics() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::try_line("host", &format!("fatal: {info}"));
        #[cfg(windows)]
        crate::win::crash::dump_once();
        default(info);
    }));
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a tokio runtime")
}

/// The boot every mode shares, from this process's environment.
pub fn boot(paths: Paths, store: Store, frames: Frames) -> Boot {
    Boot {
        paths,
        store,
        frames,
        engine_binary: std::env::var_os("STUDIO_ENGINE")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from),
        parts: parts::all(),
    }
}

/// `--config`'s file as a JSON object; Err with a message for stderr.
pub fn read_config(file: &std::path::Path) -> Result<Value, String> {
    let text = std::fs::read(file).map_err(|_| format!("cannot read {}", file.display()))?;
    match serde_json::from_slice::<Value>(&text) {
        Ok(v) if v.is_object() => Ok(v),
        _ => Err(format!("{} is not a JSON object", file.display())),
    }
}

/// A run of the program; the process's exit code. Call it before any other thread exists.
pub fn main(mode: Mode) -> i32 {
    // Before the engine is spawned, so it is born inside the host's job (`docs/windows.md`).
    #[cfg(windows)]
    {
        // A crash may come while the log is held: its lines never wait for the lock.
        crate::win::crash::install(Paths::from_env().studio_dir().to_path_buf(), |t| {
            log::try_line("host", t)
        });
        crate::win::job::keep_stdio_from_children();
        if let Err(why) = crate::win::job::join_host_job() {
            log::stderr_only(&format!("[host] no kill-on-close job: {why}"));
        }
    }
    let paths = Paths::from_env();
    let (store, frames, standalone) = match &mode {
        Mode::Stdio => (Store::new(paths.settings()), Frames::stdout(), false),
        Mode::Config { file, port } => {
            let raw = match read_config(file) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("{e}");
                    return 2;
                }
            };
            for (var, value) in config_environment(&raw) {
                // SAFETY: called from `main` before the runtime or any other thread starts.
                unsafe { std::env::set_var(var, value) };
            }
            let store = Store {
                file: file.clone(),
                read_only: true,
                fixed_port: *port,
            };
            (store, Frames::silent(), true)
        }
    };
    log::init(paths.merged_log(), standalone);
    log_panics();
    let rt = runtime();
    let code = rt.block_on(async move {
        let (tx, rx) = mpsc::unbounded_channel();
        if standalone {
            log::stderr_only("[host] running from the config file; Ctrl-C to stop");
        } else {
            tokio::spawn(read_commands(tokio::io::stdin(), tx.clone()));
        }
        tokio::spawn(on_signal(tx));
        finish(core::run(boot(paths, store, frames), rx).await)
    });
    // A blocked stdin read may still hold a thread; leaving is what ends it.
    rt.shutdown_timeout(std::time::Duration::from_millis(100));
    code
}

fn finish(r: Result<(), String>) -> i32 {
    match r {
        Ok(()) => 0,
        Err(e) => {
            // Nothing above us reports this, an app sees only a process that exited, so it is said.
            log::stderr_only(&format!("[host] fatal: {e}"));
            1
        }
    }
}

/// The app's commands, one per line, then Shutdown when the stream ends.
pub async fn read_commands(
    from: impl tokio::io::AsyncRead + Unpin,
    tx: mpsc::UnboundedSender<Input>,
) {
    let mut lines = BufReader::new(from).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if let Some(c) = parse_command(&line) {
            let _ = tx.send(Input::Command(c));
        }
    }
    let _ = tx.send(Input::Shutdown);
}

async fn on_signal(tx: mpsc::UnboundedSender<Input>) {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let (Ok(mut int), Ok(mut term), Ok(mut hup)) = (
            signal(SignalKind::interrupt()),
            signal(SignalKind::terminate()),
            signal(SignalKind::hangup()),
        ) else {
            return;
        };
        tokio::select! { _ = int.recv() => {}, _ = term.recv() => {}, _ = hup.recv() => {} }
    }
    #[cfg(not(unix))]
    let _ = tokio::signal::ctrl_c().await;
    let _ = tx.send(Input::Shutdown);
}

/// Inside an app: commands read from `commands`, frames written to `frames`, until the app closes its end.
pub fn embedded(commands: std::fs::File, frames: std::fs::File) -> i32 {
    let paths = Paths::from_env();
    log::init(paths.merged_log(), false);
    log_panics();
    let store = Store::new(paths.settings());
    let rt = runtime();
    let code = rt.block_on(async move {
        let (tx, rx) = mpsc::unbounded_channel();
        let reader = tx.clone();
        // A plain thread: the app's pipe is a blocking descriptor, and its end is the app's quit.
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(commands)
                .lines()
                .map_while(Result::ok)
            {
                if let Some(c) = parse_command(&line) {
                    let _ = reader.send(Input::Command(c));
                }
            }
            let _ = reader.send(Input::Shutdown);
        });
        drop(tx);
        finish(core::run(boot(paths, store, Frames::file(frames)), rx).await)
    });
    rt.shutdown_timeout(std::time::Duration::from_millis(100));
    code
}
