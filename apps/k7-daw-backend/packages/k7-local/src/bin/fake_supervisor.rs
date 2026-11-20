//! k7-fake-supervisor — a stand-in for `engine/k7-supervisor` in `k7-local`'s and `k7-front`'s tests: it speaks the
//! host protocol on stdio and obeys a few test commands. Copied under the name `k7-runtime`, it plays the engine.
//! Never shipped: nothing but `CARGO_BIN_EXE_k7-fake-supervisor` runs it.

use std::io::{BufRead, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{Value, json};

fn out(frame: Value) {
    let mut o = std::io::stdout().lock();
    let _ = writeln!(o, "{frame}");
    let _ = o.flush();
}

fn notify(body: &str) {
    out(json!({ "t": "notify", "kind": "passive", "title": "fake", "body": body }));
}

fn state(status: &str) {
    let env = |k: &str| std::env::var(k).unwrap_or_default();
    out(json!({
        "t": "state",
        "owns": { "engine": true, "engineQueries": true },
        "status": status,
        "ownsEngine": true,
        "userStopped": false,
        "engineId": "e1",
        "enginePort": 8765,
        "registryUrl": env("STUDIO_REGISTRY_URL"),
        "lastHeartbeatOk": false,
        "pluginCount": null,
        "pluginsSig": "",
        "audioMenuSig": "",
        "editPluginsSig": "",
        "sessionsSig": "",
        "instanceCheck": null,
        "paths": { "root": env("STUDIO_HOME"), "mergedLog": "" },
        "hostPid": env("STUDIO_HOST_PID"),
        "engineRoots": env("STUDIO_ENGINE_ROOTS"),
    }));
}

fn forever() -> ! {
    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}

/// Closes this process's stdin for real, so the host's next write meets a broken pipe.
fn close_stdin() {
    #[cfg(unix)]
    {
        use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
        drop(unsafe { OwnedFd::from_raw_fd(std::io::stdin().as_raw_fd()) });
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
        drop(unsafe { OwnedHandle::from_raw_handle(std::io::stdin().as_raw_handle()) });
    }
}

fn main() {
    let me = std::env::current_exe().unwrap_or_default();
    let stem = me.file_stem().map(|s| s.to_string_lossy().to_lowercase());
    if stem.as_deref() == Some("k7-runtime") {
        forever();
    }
    eprintln!("fake supervisor says hello");
    println!("not a frame");
    state("running");
    let mut hang = false;
    let stdin = std::io::stdin();
    let mut line = String::new();
    loop {
        line.clear();
        match stdin.lock().read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let Ok(c) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let arg = c["arg"].as_str().unwrap_or("").to_string();
        match c["cmd"].as_str().unwrap_or("") {
            "crash" => std::process::exit(3),
            "shutdown" if !hang => std::process::exit(0),
            "hang" => hang = true,
            "notify" => notify(&arg),
            "state" => state(&arg),
            "fail" => out(json!({ "t": "result", "id": c["id"], "ok": false, "message": "no" })),
            "closein" => {
                close_stdin();
                notify("stdin closed");
                forever();
            }
            "engine" => {
                // The engine keeps a copy of this stdout, as a real one may: the host must still see this process die.
                let engine = std::env::var("K7_FAKE_ENGINE").unwrap_or_default();
                match Command::new(engine)
                    .stdin(Stdio::null())
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::null())
                    .spawn()
                {
                    Ok(child) => notify(&format!("engine {}", child.id())),
                    Err(e) => notify(&format!("no engine: {e}")),
                }
            }
            _ => {}
        }
    }
    if hang {
        forever();
    }
}
