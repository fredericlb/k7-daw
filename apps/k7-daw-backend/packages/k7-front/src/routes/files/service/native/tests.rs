//! The native pieces' test helpers: a scratch account, a context over it, the waits and the log.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use k7_file_service::libraries::Home;

use super::{Chaos, Ctx, Tag};

pub(super) type Lines = Arc<Mutex<Vec<String>>>;

pub(super) fn log_to(lines: &Lines) -> k7_hub::Log {
    let lines = lines.clone();
    Arc::new(move |m: &str| lines.lock().unwrap().push(m.to_string()))
}

/// The system temp folder in the model's spelling (slashes only), under which the tests make their folders.
pub(super) fn tmp_root() -> std::path::PathBuf {
    let t = std::env::temp_dir();
    std::path::PathBuf::from(t.display().to_string().replace('\\', "/"))
}

/// A path in the model's spelling, slashes only, wherever a `join` left a backslash.
pub(super) fn slashed(p: &std::path::Path) -> String {
    p.display().to_string().replace('\\', "/")
}

pub(super) fn scratch(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("k7-native-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("home/workspace/songs")).unwrap();
    // The model's spelling: no `\\?\` prefix, slashes only (the account keys its roots that way).
    let real = std::fs::canonicalize(d).unwrap();
    std::path::PathBuf::from(
        k7_file_service::confine::spelled(&real).expect("a drive or posix temp dir"),
    )
}

/// A context for `rel` in the workspace at `root`.
pub(super) fn ctx(dir: &std::path::Path, tag: Tag, rel: &str, lines: &Lines) -> Ctx {
    let studio = crate::routes::files::service::native::tests::slashed(&dir.join("home"));
    Ctx {
        tag,
        touches: true,
        home: Home {
            studio: studio.clone(),
            libraries: dir.join("libraries").display().to_string(),
        },
        root: dir.join("home/workspace/songs").display().to_string(),
        workspace: "songs".into(),
        rel: rel.into(),
        account: "acct".into(),
        log: log_to(lines),
        gate: super::answering::global(),
        docs: None,
        remote: None,
        verbs: None,
        chaos: Chaos::None,
    }
}

/// The bound of every wait here: it ends only a run that never got its signal, so it is long on a loaded host.
pub(super) const BACKSTOP: Duration = if cfg!(target_env = "musl") {
    Duration::from_secs(120)
} else {
    Duration::from_secs(60)
};

/// Removes a scratch folder, retrying while a task still in flight writes into it.
pub(super) fn remove_tree(dir: &std::path::Path) {
    for _ in 0..20 {
        if std::fs::remove_dir_all(dir).is_ok() || !dir.exists() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Polls `f` until it holds, up to `BACKSTOP`.
pub(super) async fn until(what: &str, f: impl Fn() -> bool) {
    let by = std::time::Instant::now() + BACKSTOP;
    while !f() {
        assert!(std::time::Instant::now() < by, "never: {what}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

pub(super) fn counted(c: &std::sync::atomic::AtomicU64) -> u64 {
    c.load(std::sync::atomic::Ordering::Relaxed)
}
