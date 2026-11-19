//! spawn.rs — what the hub is built from and launched with: the accounts' and the libraries' environments (the
//! account's own values, the sync target), the libraries' runtime, a server's cut
//! context and the caches' sweep. Choices: `docs/files.md`.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;

use k7_hub::Log;
use k7_hub::account::Names;
use serde_json::Value;

use super::{CUT, LIBRARIES, State};
use crate::AppState;
use crate::settings::Settings;

/// `path.resolve`: absolute, with `.` and `..` folded.
fn resolve(p: &Path) -> PathBuf {
    let abs = std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
    let mut out = PathBuf::new();
    for c in abs.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            c => out.push(c),
        }
    }
    out
}

fn random_hex(n: usize) -> String {
    let mut raw = vec![0u8; n];
    getrandom::fill(&mut raw).expect("no OS randomness");
    k7_hub::hex(&raw)
}

/// A setting as the backend reads it: the host's `Config.env` first, this process's environment else.
fn setting(env: &BTreeMap<String, String>, key: &str) -> Option<String> {
    env.get(key)
        .cloned()
        .or_else(|| std::env::var(key).ok())
        .filter(|v| !v.is_empty())
}

/// The hub's state from the front's settings; nothing runs until `launch`.
pub(super) fn state(config: &crate::Config, settings: &Settings, port: u16) -> State {
    let env = &config.env;
    let data_root = resolve(Path::new(
        &setting(env, "STUDIO_FILES_ROOT").unwrap_or_default(),
    ));
    let log: Log = Arc::new(|m: &str| println!("files | {m}"));
    // The process's own allow-listed variables, overlaid by what the host handed the front.
    let mut base: BTreeMap<String, String> = std::env::vars()
        .filter(|(k, _)| k7_hub::env::allowed(k))
        .collect();
    base.extend(
        env.iter()
            .filter(|(k, _)| k7_hub::env::allowed(k))
            .map(|(k, v)| (k.clone(), v.clone())),
    );
    let app: Arc<OnceLock<Weak<AppState>>> = Arc::new(OnceLock::new());
    let machine = settings.local_user;
    let library_secret = random_hex(24);
    let seam = k7_hub::seam::Seam::new(&format!("127.0.0.1:{port}"), &config.secret, log.clone());
    // Spelled as node's `path.resolve` spells it: the engine's roots and the services' paths are compared as text.
    let mediatek_root = resolve(&settings.mediatek_dir);

    let root = data_root.clone();
    let mut files = Names::new(Arc::new(move |user: &str| {
        let slug = k7_hub::account::account_key(user);
        (slug.clone(), root.join(slug))
    }));
    let (r, m) = (data_root.clone(), mediatek_root.clone());
    // Every account is the front's where it has a code root, and it captures.
    let beside = super::super::service::code_root(env).is_some();
    files.env = Arc::new(move |_, _| {
        let (r, m) = (r.clone(), m.clone());
        Box::pin(async move {
            let mut env = BTreeMap::new();
            env.insert(
                "STUDIO_LIBRARIES_HOME".into(),
                r.join("libraries").display().to_string(),
            );
            env.insert(
                "STUDIO_HUB_URL".into(),
                format!("http://127.0.0.1:{port}/api/files-hub/internal"),
            );
            // The account's capture is the front's: no workspace joins a relay.
            if beside {
                env.insert(super::super::service::FRONT_CAPTURE.into(), "1".into());
            }
            env.insert("STUDIO_MEDIATEK".into(), m.display().to_string());
            env
        })
    });

    let root = data_root.clone();
    let mut libraries = Names::new(Arc::new(move |_: &str| {
        (LIBRARIES.to_string(), root.clone())
    }));
    libraries.base_env = base;
    if let Some(v) = setting(env, "STUDIO_LIBRARY_SEED") {
        libraries.base_env.insert("STUDIO_LIBRARY_SEED".into(), v);
    }
    let r = data_root.clone();
    libraries.env = Arc::new(move |_, _| {
        let mut env = BTreeMap::new();
        env.insert("STUDIO_FILES_ROOT".into(), r.display().to_string());
        if machine {
            env.insert("STUDIO_LOCAL_USER".into(), "1".into());
        }
        Box::pin(async move { env })
    });

    // A server's cut resolves its chain here: the libraries' folders under a home of its own, no account's.
    let (root, home) = (data_root.clone(), data_root.join(CUT));
    let mut cut = Names::new(Arc::new(move |_: &str| (CUT.to_string(), home.clone())));
    cut.env = Arc::new(move |_, _| {
        let libs = root.join("libraries").display().to_string();
        Box::pin(async move { BTreeMap::from([("STUDIO_LIBRARIES_HOME".to_string(), libs)]) })
    });

    State {
        data_root,
        mediatek_root,
        ticket_key: if settings.raw_session_secret.is_empty() {
            random_hex(32).into_bytes()
        } else {
            format!("{}:files", settings.raw_session_secret).into_bytes()
        },
        library_secret,
        device: machine,
        names: Arc::new(files),
        bindings: Arc::default(),
        libraries: Arc::new(libraries),
        cut: Arc::new(cut),
        library_native: OnceLock::new(),
        seam,
        log,
        app,
        settled: Arc::new(AtomicBool::new(!machine)),
        started: OnceLock::new(),
        tasks: Mutex::new(Vec::new()),
    }
}

/// The libraries made and watched until their first `ensure` settles; the local user warmed.
pub(super) async fn launch(app: &Arc<AppState>) {
    let Some(files) = app.files.clone() else {
        return;
    };
    let hub = &files.hub;
    let _ = hub.app.set(Arc::downgrade(app));
    let _ = hub.started.set(std::time::Instant::now());
    let mut tasks = Vec::new();
    match super::super::service::libraries::start(&files.service, &hub.libraries, LIBRARIES).await {
        Some(Ok(native)) => {
            (hub.log)("the libraries run in the front");
            let _ = hub.library_native.set(native);
        }
        Some(Err(e)) => (hub.log)(&format!("the libraries were not made: {e}")),
        None => {}
    }
    // Made or refused, the libraries have nothing left to wait for once their runtime is not coming.
    if hub.library_native.get().is_none() {
        (hub.log)(&format!(
            "the libraries are refused: {}",
            files.service.libraries_refusal()
        ));
        hub.settled.store(true, Ordering::Relaxed);
    }
    let f = files.clone();
    tasks.push(tokio::spawn(async move {
        let h = &f.hub;
        if let Some(native) = h.library_native.get() {
            while !native.settled() {
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            h.settled.store(true, Ordering::Relaxed);
        }
    }));
    // One person here: their workspaces are ready before the first page asks for them.
    if hub.is_device()
        && let Some(email) = crate::session::local_user()
            .get("email")
            .and_then(Value::as_str)
            .map(str::to_string)
    {
        let f = files.clone();
        tasks.push(tokio::spawn(async move { f.hub.warm(&email).await }));
    }
    let f = files.clone();
    tasks.push(tokio::spawn(async move {
        let mut tick = tokio::time::interval(k7_hub::stale::CACHE_SWEEP_EVERY);
        loop {
            tick.tick().await;
            let (root, log) = (f.hub.data_root.clone(), f.hub.log.clone());
            let _ = tokio::task::spawn_blocking(move || {
                let now = k7_hub::account::now_ms();
                let ttl = k7_hub::stale::CACHE_TTL;
                let n = k7_hub::stale::purge_stale_renders(&root, now, ttl)
                    + k7_hub::stale::purge_stale_history(&root, now, ttl);
                if n > 0 {
                    log(&format!("purged {n} unused cache entr(ies)"));
                }
            })
            .await;
        }
    }));
    hub.tasks
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .extend(tasks);
}

#[cfg(test)]
mod tests {
    use super::resolve;
    use std::path::Path;

    #[test]
    fn resolve_folds_dots_and_makes_absolute() {
        let r = resolve(Path::new("a/./b/../c"));
        assert!(r.is_absolute());
        assert!(r.ends_with(Path::new("a").join("c")));
    }

    #[cfg(windows)]
    #[test]
    fn resolve_spells_a_windows_path_with_backslashes_as_node_does() {
        assert_eq!(
            resolve(Path::new("C:/Users/x/.studio/mediatek")).to_str(),
            Some(r"C:\Users\x\.studio\mediatek")
        );
    }
}
