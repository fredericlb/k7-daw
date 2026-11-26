//! render.rs — the `render` job's work: bounce a ProjectDoc through THIS machine's engine, into a scratch folder
//! this call made, and copy the files out to where the server takes them back. A computer runs
//! `k7-runtime --render`; a phone, which may start no program, asks the RUNNING engine over its socket. What a
//! reader gets wrong: the scratch folder is owner-only and made here, never one another process could have
//! prepared, and every file the engine names is checked against the job's writable folders before it is copied.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::client_async;
use tokio_tungstenite::tungstenite::Message;

use crate::paths::utf8;

/// How long the engine gets to open its socket for a render.
const OPEN: Duration = Duration::from_secs(4);
/// A render outlasts any query's deadline.
pub const RENDER_LIMIT: Duration = Duration::from_secs(2 * 3600);

/// What the server asked: `out` takes the mix alone, `out_dir` the whole manifest.
pub struct Request {
    pub doc: Value,
    pub out: String,
    pub out_dir: String,
    pub stems: bool,
}

/// How the bounce is made.
#[derive(Clone)]
pub enum Via {
    /// `<exe> --render - --out <scratch>`, the doc on stdin.
    Child(PathBuf),
    /// A `render` command to the engine on `port`, its scratch folder under `scratch_under`; `rendering` hears +1, then -1.
    Engine {
        port: u16,
        scratch_under: PathBuf,
        rendering: Arc<dyn Fn(i32) + Send + Sync>,
    },
}

/// A folder under `under` (the temp directory when none) that THIS call created, owner-only.
fn fresh_scratch(prefix: &str, under: Option<&Path>) -> Result<PathBuf, String> {
    let base = match under {
        Some(u) => {
            std::fs::create_dir_all(u)
                .map_err(|e| format!("no temporary folder to work in: {e}"))?;
            u.to_path_buf()
        }
        None => std::env::temp_dir(),
    };
    for _ in 0..16 {
        let mut b = [0u8; 8];
        getrandom::fill(&mut b).map_err(|e| e.to_string())?;
        let name: String = b.iter().map(|x| format!("{x:02x}")).collect();
        let dir = base.join(format!("{prefix}{name}"));
        // Fails when the name is taken, by a folder, a file or a link alike: only a folder made here is used.
        let mut make = std::fs::DirBuilder::new();
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut make, 0o700);
        #[cfg(not(unix))]
        let _ = &mut make;
        if make.create(&dir).is_ok() {
            return Ok(dir);
        }
    }
    Err("could not make a temporary folder to work in".into())
}

/// The scratch folder, removed however the job ends.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn failure(id: &Value, why: &str) -> Value {
    json!({"type": "rendered", "id": id, "ok": false, "error": why})
}

/// Send `cmd` (a `render` command) to the engine on `port` and wait for its `rendered` reply; an `ok:false`
/// reply with `error` when none comes. It has a socket of its own: `rendered` goes to the asking socket alone.
pub async fn ask_engine(port: u16, cmd: &Value, limit: Duration) -> Value {
    let id = cmd["id"].clone();
    let dialed = timeout(OPEN, async {
        let tcp = TcpStream::connect(("127.0.0.1", port)).await.ok()?;
        client_async(format!("ws://127.0.0.1:{port}"), tcp)
            .await
            .ok()
            .map(|(ws, _)| ws)
    })
    .await
    .ok()
    .flatten();
    let Some(mut ws) = dialed else {
        return failure(&id, "the engine did not answer");
    };
    let closed = "the engine closed the connection during the render";
    if ws.send(Message::text(cmd.to_string())).await.is_err() {
        return failure(&id, closed);
    }
    let wait = async {
        while let Some(Ok(frame)) = ws.next().await {
            let Message::Text(t) = frame else { continue };
            if let Ok(m) = serde_json::from_str::<Value>(t.as_str())
                && m["type"] == "rendered"
                && m["id"] == id
            {
                return Some(m);
            }
        }
        None
    };
    match timeout(limit, wait).await {
        Ok(Some(reply)) => reply,
        Ok(None) => failure(&id, closed),
        Err(_) => failure(&id, "the render took too long"),
    }
}

/// The manifest is the LAST line the engine printed: `{"mix": "<file>", "stems": [{"name", "file"}]}`.
fn manifest_of(out: &str) -> Option<Value> {
    out.lines()
        .rev()
        .filter_map(|l| serde_json::from_str::<Value>(l.trim_end_matches('\r')).ok())
        .find(|m| m["mix"].as_str().is_some_and(|s| !s.is_empty()))
}

/// Run `exe` with `args`, `stdin` written to it; its exit code and what it printed.
fn run_child(exe: &Path, args: &[String], stdin: String) -> Result<(i32, String), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut cmd = Command::new(exe);
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: a render has no console to show.
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let mut input = child.stdin.take();
    // Written beside the read: a doc bigger than a pipe must not wait on output nobody reads yet.
    let writer = std::thread::spawn(move || {
        if let Some(i) = input.as_mut() {
            let _ = i.write_all(stdin.as_bytes());
        }
    });
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let _ = writer.join();
    Ok((
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    ))
}

/// The file the engine named, as a child of `tmp` and nothing else.
fn inside(tmp: &Path, file: &str) -> Result<PathBuf, String> {
    let p = Path::new(file);
    if file.is_empty() || !p.components().all(|c| matches!(c, Component::Normal(_))) {
        return Err(format!(
            "the engine named a file outside its render folder: {file}"
        ));
    }
    Ok(tmp.join(p))
}

/// Bounce `req.doc` and put the files where it says; `started` is called once the work begins; the `rendered` reply.
pub async fn render(
    req: &Request,
    via: &Via,
    writable: &(dyn Fn(&str) -> bool + Send + Sync),
    started: &(dyn Fn() + Send + Sync),
) -> Result<Value, String> {
    let tmp = match via {
        Via::Child(_) => fresh_scratch("k7-render-", None)?,
        Via::Engine { scratch_under, .. } => fresh_scratch("render-", Some(scratch_under))?,
    };
    let scratch = Scratch(tmp.clone());
    started();
    let want_stems = !req.out_dir.is_empty() && req.stems;
    let manifest = match via {
        Via::Engine {
            port, rendering, ..
        } => {
            if *port == 0 {
                return Err("the engine is not running".into());
            }
            rendering(1);
            let name = tmp
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let cmd = json!({"type": "render", "id": format!("job-{name}"), "doc": req.doc, "outDir": utf8(&tmp), "stems": want_stems});
            let answer = ask_engine(*port, &cmd, RENDER_LIMIT).await;
            rendering(-1);
            if answer["ok"] != true {
                let why = answer["error"].as_str().unwrap_or("no reason given");
                return Err(format!("engine render failed: {why}"));
            }
            answer["manifest"]
                .as_object()
                .filter(|m| {
                    m.get("mix")
                        .and_then(Value::as_str)
                        .is_some_and(|s| !s.is_empty())
                })
                .map(|_| answer["manifest"].clone())
                .ok_or("could not read the engine's render manifest")?
        }
        Via::Child(exe) => {
            let mut args = vec![
                "--render".to_string(),
                "-".into(),
                "--out".into(),
                utf8(&tmp),
            ];
            if want_stems {
                args.push("--stems".into());
            }
            // The doc on stdin (`--render -`): no file of it to leave behind.
            let (exe, doc) = (exe.clone(), req.doc.to_string());
            let (code, out) = tokio::task::spawn_blocking(move || run_child(&exe, &args, doc))
                .await
                .map_err(|_| "the render stopped unexpectedly".to_string())??;
            if code != 0 {
                return Err(format!("engine render failed (exit {code})"));
            }
            manifest_of(&out).ok_or("could not read the engine's render manifest")?
        }
    };
    let copy_out = |file: &str, dest: &Path| -> Result<(), String> {
        // The engine names the stems, so each one is held to the same folders as the job.
        if !writable(&utf8(dest)) {
            return Err(
                "render: refused a stem path outside this server's writable folders".into(),
            );
        }
        let from = inside(&tmp, file)?;
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::copy(from, dest)
            .map(|_| ())
            .map_err(|e| format!("could not write the render: {e}"))
    };
    let mix = manifest["mix"].as_str().unwrap_or("");
    let reply = if !req.out.is_empty() {
        copy_out(mix, Path::new(&req.out))?;
        // Named as a FILE, so the tunnel uploads it before the server reads this reply.
        json!({"type": "rendered", "file": req.out})
    } else {
        let dir = Path::new(&req.out_dir);
        copy_out(mix, &dir.join(mix))?;
        let mut stems = Vec::new();
        for s in manifest["stems"].as_array().into_iter().flatten() {
            let file = s["file"].as_str().unwrap_or("");
            if file.is_empty() {
                continue;
            }
            copy_out(file, &dir.join(file))?;
            stems.push(
                json!({"name": s["name"].as_str().unwrap_or(""), "file": utf8(&dir.join(file))}),
            );
        }
        // Every file named whole, so the tunnel uploads each before the server reads this reply.
        json!({"type": "rendered", "mix": utf8(&dir.join(mix)), "stems": stems})
    };
    drop(scratch);
    Ok(reply)
}
