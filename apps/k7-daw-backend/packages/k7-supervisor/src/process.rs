//! process.rs — a child started in a process group of its own, its output drained line by line into the
//! merged log, and its exit told by the thread that reaps it: the one liveness a reused pid cannot fake.
//! Its stdin is null and every pipe Rust makes is close-on-exec, so no child holds the app's pipe and an
//! app reading our stdout sees its EOF when we die (`apps/k7-daw-backend/packages/k7-supervisor/docs/supervisor.md`).

use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;

/// One call per complete output line, from a pump thread; `true` for stderr.
pub type OnLine = Arc<dyn Fn(&str, bool) + Send + Sync>;

pub struct SpawnOptions {
    pub program: PathBuf,
    pub args: Vec<String>,
    /// Merged over this process's environment.
    pub env: Vec<(String, String)>,
    pub on_line: OnLine,
    /// Called once with the pid when the child has exited, from the thread that reaped it.
    pub on_exit: Box<dyn FnOnce(u32) + Send>,
}

fn pump(from: impl Read + Send + 'static, is_err: bool, on_line: OnLine) {
    std::thread::spawn(move || {
        let mut r = BufReader::new(from);
        let mut buf = Vec::new();
        while matches!(r.read_until(b'\n', &mut buf), Ok(n) if n > 0) {
            let line = String::from_utf8_lossy(&buf);
            on_line(line.trim_end_matches(['\n', '\r']), is_err);
            buf.clear();
        }
    });
}

/// Start a detached child in its own process group; its pid, or why it could not start.
pub fn spawn(opts: SpawnOptions) -> Result<u32, String> {
    let mut cmd = Command::new(&opts.program);
    cmd.args(&opts.args)
        .envs(opts.env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW: no console of its own, out of the console's Ctrl-C.
        cmd.creation_flags(0x0000_0200 | 0x0800_0000);
    }
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let pid = child.id();
    if let Some(out) = child.stdout.take() {
        pump(out, false, opts.on_line.clone());
    }
    if let Some(err) = child.stderr.take() {
        pump(err, true, opts.on_line.clone());
    }
    let on_exit = opts.on_exit;
    std::thread::spawn(move || {
        let _ = child.wait();
        on_exit(pid);
    });
    Ok(pid)
}

/// The process the engine is a thread of when it lives in an app: our parent on Android, ourselves on iOS.
pub fn app_pid() -> u32 {
    #[cfg(target_os = "android")]
    {
        // SAFETY: getppid has no preconditions and cannot fail.
        unsafe { libc::getppid() as u32 }
    }
    #[cfg(not(target_os = "android"))]
    {
        std::process::id()
    }
}

/// Where a package manager or the OS puts a tool that an app's PATH may not list (macOS keeps `lsof` in /usr/sbin).
#[cfg(unix)]
const TOOL_DIRS: &[&str] = &[
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "/usr/bin",
    "/usr/sbin",
];
#[cfg(not(unix))]
const TOOL_DIRS: &[&str] = &[];

/// `name` in the directories of `path` (a PATH value), then in `extra`; absolute directories only, so the working directory never picks.
pub fn find_tool_in(name: &str, path: Option<&std::ffi::OsStr>, extra: &[&str]) -> Option<PathBuf> {
    let file = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    let listed = path
        .map(|p| std::env::split_paths(p).collect::<Vec<_>>())
        .unwrap_or_default();
    listed
        .into_iter()
        .chain(extra.iter().map(PathBuf::from))
        .filter(|d| d.is_absolute())
        .map(|d| d.join(&file))
        .find(|f| f.is_file())
}

/// `name` on this process's PATH or where a package manager puts it; None rather than a bare name.
pub fn find_tool(name: &str) -> Option<PathBuf> {
    find_tool_in(name, std::env::var_os("PATH").as_deref(), TOOL_DIRS)
}
