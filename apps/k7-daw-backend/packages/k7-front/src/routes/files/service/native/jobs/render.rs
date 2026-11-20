//! render.rs — the local renderer: this machine's `k7-runtime --render`, spawned, bounded, staged and renamed (2320 to 2323, 2313).
//! What a reader gets wrong: the child gets the account's environment alone; a render lands by one rename of a whole
//! file, so a killed one leaves a `.part-` folder the next render of that name sweeps, never a half file under the name.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use k7_file_service::confine::{Disk, confine_to};
use k7_file_service::jobs::manifest::{Moved, parse_manifest};
use k7_file_service::paths::join;
use serde_json::Value;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

/// A wedged engine is killed after this (2321); a song bounces in seconds, a share with stems in minutes.
pub const RENDER_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// The manifest is the last line, so only the tail of a chatty child's stdout is kept.
const TAIL: usize = 1 << 20;

/// How long a pipe a plugin's helper holds open is read after the engine exited; its last line is already written.
const EXIT_GRACE: Duration = Duration::from_secs(1);

/// Sentences of the refusals the TypeScript did not make: it trusted the manifest (DEPARTS 2321b).
pub const NO_MIX: &str = "the engine rendered no mixdown";
pub const NOT_WAV: &str = "the engine's mixdown is not a .wav file";
pub const OUTSIDE: &str = "the engine named a file outside its render";

/// How to bounce: per-track stems beside the mix, and the folder relative clip paths resolve under.
#[derive(Clone, Debug, Default)]
pub struct RenderOpts {
    pub stems: bool,
    pub base: Option<String>,
}

/// This machine's engine binary and the account's environment, the only one its child sees.
pub struct LocalRenderer {
    bin: PathBuf,
    env: Vec<(String, String)>,
    timeout: Duration,
}

impl LocalRenderer {
    pub fn new(bin: PathBuf, env: Vec<(String, String)>) -> LocalRenderer {
        LocalRenderer {
            bin,
            env,
            timeout: RENDER_TIMEOUT,
        }
    }

    /// A shorter bound, for a test that waits on the kill.
    #[cfg(all(test, unix))]
    pub fn with_timeout(mut self, timeout: Duration) -> LocalRenderer {
        self.timeout = timeout;
        self
    }

    /// Bounces `doc` (a ProjectDoc's JSON) into `out_dir`: the manifest, its files checked to be wavs of that folder.
    pub async fn render(
        &self,
        doc: &str,
        out_dir: &Path,
        opts: &RenderOpts,
    ) -> Result<Moved, String> {
        std::fs::create_dir_all(out_dir).map_err(|e| node_error(&e, "mkdir", out_dir))?;
        let doc_file = out_dir.join("doc.json");
        tokio::fs::write(&doc_file, doc)
            .await
            .map_err(|e| node_error(&e, "open", &doc_file))?;
        let mut cmd = Command::new(&self.bin);
        cmd.arg("--render").arg(&doc_file).arg("--out").arg(out_dir);
        if opts.stems {
            cmd.arg("--stems");
        }
        if let Some(base) = opts.base.as_deref().filter(|b| !b.is_empty()) {
            cmd.arg("--base").arg(base);
        }
        // Dropping this future kills the child: a request that went away leaves no engine behind (2352).
        cmd.env_clear()
            .envs(self.env.iter().cloned())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);
        #[cfg(unix)]
        cmd.process_group(0);
        let mut child = cmd.spawn().map_err(|e| spawn_error(&self.bin, &e))?;
        // The plugins' helpers share the child's group: a kill before its exit takes them too (Unix, K4 decision).
        let mut group = Group(child.id());
        let mut stdout = child
            .stdout
            .take()
            .ok_or("the engine's stdout was not piped")?;
        let mut tail = Vec::new();
        let run = async {
            let read = read_tail(&mut stdout, &mut tail);
            tokio::pin!(read);
            tokio::select! {
                status = child.wait() => {
                    // A helper that inherited the pipe keeps it open: what was written by the exit is all there is.
                    let _ = tokio::time::timeout(EXIT_GRACE, &mut read).await;
                    status
                }
                () = &mut read => child.wait().await,
            }
        };
        let Ok(status) = tokio::time::timeout(self.timeout, run).await else {
            // Reaped before the answer, so the staging is removed after the child is gone.
            let _ = child.kill().await;
            return Err(failed(None));
        };
        group.disarm();
        match status {
            Ok(s) if s.success() => {}
            Ok(s) => return Err(failed(s.code())),
            Err(_) => return Err(failed(None)),
        }
        let manifest = parse_manifest(&String::from_utf8_lossy(&tail))?;
        checked(&manifest, out_dir)
    }

    /// Bounces `doc`'s mixdown to exactly `out_wav`: made whole in a `.part-` folder beside it, renamed once (2313).
    pub async fn render_to_wav(&self, doc: &str, out_wav: &Path) -> Result<(), String> {
        let parent = out_wav.parent().ok_or("a render needs a folder")?;
        std::fs::create_dir_all(parent).map_err(|e| node_error(&e, "mkdir", parent))?;
        sweep_parts(out_wav);
        let staging = Staging::new(out_wav);
        let moved = self.render(doc, &staging.0, &RenderOpts::default()).await?;
        let mix = staging.0.join(&moved.mix);
        tokio::fs::rename(&mix, out_wav)
            .await
            .map_err(|e| node_error(&e, "rename", &mix))
    }
}

/// `ready` of a freeze: false only when the render is not there; any other error of the stat is the pass's failure, never a missing file (2313).
pub fn render_ready(render: &Path) -> Result<bool, String> {
    match std::fs::metadata(render) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(node_error(&e, "stat", render)),
    }
}

/// The child's process group, killed when this is dropped before the child exited (a timeout, a request that went away).
struct Group(Option<u32>);

impl Group {
    fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for Group {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(pid) = self.0 {
            // The shell's own `kill` takes a negative pid as a group, which `std` has no call for without libc.
            let _ = std::process::Command::new("sh")
                .args(["-c", "kill -KILL -- \"-$0\" 2>/dev/null", &pid.to_string()])
                .status();
        }
        #[cfg(not(unix))]
        let _ = self.0;
    }
}

/// A scratch folder named for the render it will become, removed whatever happens to the render.
struct Staging(PathBuf);

impl Staging {
    fn new(out_wav: &Path) -> Staging {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let tag = format!(
            "{:x}-{:x}-{}",
            nanos,
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        Staging(PathBuf::from(format!("{}.part-{tag}", out_wav.display())))
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The `.part-` leftovers of a killed render of `out_wav`: only those older than a render may live, so a running one stays.
pub(super) fn sweep_parts(out_wav: &Path) {
    let (Some(parent), Some(name)) = (out_wav.parent(), out_wav.file_name()) else {
        return;
    };
    let prefix = format!("{}.part-", name.to_string_lossy());
    let Ok(read) = std::fs::read_dir(parent) else {
        return;
    };
    for entry in read.filter_map(Result::ok) {
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .is_ok_and(|t| t.elapsed().is_ok_and(|age| age > RENDER_TIMEOUT));
        if old && entry.file_name().to_string_lossy().starts_with(&prefix) {
            let path = entry.path();
            let _ = std::fs::remove_dir_all(&path).or_else(|_| std::fs::remove_file(&path));
        }
    }
}

/// Keeps the last `TAIL` bytes of the child's stdout, draining the rest so the child never blocks on a full pipe.
async fn read_tail(stdout: &mut tokio::process::ChildStdout, tail: &mut Vec<u8>) {
    let mut buf = vec![0u8; 64 * 1024];
    while let Ok(n) = stdout.read(&mut buf).await {
        if n == 0 {
            break;
        }
        push_tail(tail, &buf[..n]);
    }
}

fn push_tail(tail: &mut Vec<u8>, chunk: &[u8]) {
    tail.extend_from_slice(chunk);
    if tail.len() > TAIL {
        tail.drain(..tail.len() - TAIL);
    }
}

/// The manifest's files, each a file of `out_dir` reached through no link out of it; the mix a wav.
fn checked(manifest: &Value, out_dir: &Path) -> Result<Moved, String> {
    let mix = manifest.get("mix").and_then(Value::as_str).unwrap_or("");
    if mix.is_empty() {
        return Err(NO_MIX.to_owned());
    }
    if !mix.to_ascii_lowercase().ends_with(".wav") {
        return Err(NOT_WAV.to_owned());
    }
    let stems: Vec<(String, String)> = manifest
        .get("stems")
        .and_then(Value::as_array)
        .map(|l| {
            l.iter()
                .filter_map(|s| {
                    Some((
                        s.get("name")?.as_str()?.to_owned(),
                        s.get("file")?.as_str()?.to_owned(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    let root = out_dir.to_string_lossy().replace('\\', "/");
    for file in std::iter::once(mix).chain(stems.iter().map(|(_, f)| f.as_str())) {
        // A path that names its own root is outside whatever `join` would make of it.
        if file.starts_with(['/', '\\']) || file.as_bytes().get(1) == Some(&b':') {
            return Err(OUTSIDE.to_owned());
        }
        let Some(real) = confine_to(&[&root], &join(&[&root, file]), &Disk) else {
            return Err(OUTSIDE.to_owned());
        };
        if !Path::new(&real).is_file() {
            return Err(format!(
                "the engine named a file it did not write: '{file}'"
            ));
        }
    }
    Ok(Moved {
        mix: mix.to_owned(),
        stems,
    })
}

fn failed(code: Option<i32>) -> String {
    let code = code.map_or("?".to_owned(), |c| c.to_string());
    format!("engine render failed (exit {code})")
}

/// node's spawn errors: `spawn <file> <CODE>`.
fn spawn_error(bin: &Path, e: &std::io::Error) -> String {
    let code = match e.kind() {
        std::io::ErrorKind::NotFound => "ENOENT",
        std::io::ErrorKind::PermissionDenied => "EACCES",
        _ => "EIO",
    };
    format!("spawn {} {code}", bin.display())
}

/// node's file errors: the code, its words, the call and the path.
fn node_error(e: &std::io::Error, call: &str, path: &Path) -> String {
    let (code, words) = match e.kind() {
        std::io::ErrorKind::NotFound => ("ENOENT", "no such file or directory"),
        std::io::ErrorKind::PermissionDenied => ("EACCES", "permission denied"),
        _ => return format!("{e}, {call} '{}'", path.display()),
    };
    format!("{code}: {words}, {call} '{}'", path.display())
}

#[cfg(all(test, unix))]
#[path = "tests_render.rs"]
mod tests;
