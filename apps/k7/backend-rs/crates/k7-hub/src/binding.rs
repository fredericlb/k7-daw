//! binding.rs — pointing an ACCOUNT at the engine it CLAIMED, through that engine's tunnel (`docs/engine/claim.md`):
//! only a claim binds, an engine serves one account at a time, and its plugin list is written as `<home>/exts.json`.
//! The binding is the account's, whoever serves it (`Bindings`). What a reader gets wrong: a header naming an engine is no permission.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;

use crate::Log;
use crate::seam::Seam;

/// How long the engine gets to list its plugins.
const PLUGINS_TIMEOUT: Duration = Duration::from_secs(30);
/// How long a scan running at the bind is listened to for the list it ends on.
const SCAN_LISTEN: Duration = Duration::from_secs(15 * 60);

fn root(remote: &Path, name: &str, writable: bool) -> Value {
    json!({ "remote": remote.display().to_string(), "name": name, "writable": writable })
}

/// The libraries, read-only: the hub's ONE folder of each, every account's alike.
fn libraries_root(home: &Path) -> Value {
    let dir = home.parent().unwrap_or(home).join("libraries");
    json!({ "remote": dir.display().to_string(), "name": "libs", "writable": false, "perChild": true })
}

/// What an engine's machine may see of an account: its workspaces, caches, libraries, models and the media.
pub fn roots_for(home: &Path, slug: &str, mediatek: Option<&Path>) -> Vec<Value> {
    let mut roots = vec![
        root(&home.join("workspace"), &format!("ws-{slug}"), true),
        libraries_root(home),
        root(&home.join("models"), &format!("models-{slug}"), false),
        root(&home.join(".renders"), &format!("renders-{slug}"), true),
        root(&home.join(".history"), &format!("history-{slug}"), true),
    ];
    if let Some(m) = mediatek {
        roots.push(root(m, "mediatek", false));
    }
    roots
}

/// What a media CUT shares: the library writable, for the stems, and the model's homes read-only.
/// The model's own folder is shared too when no root holds it (`$STUDIO_MODELS` moves it off `models/`).
pub fn cut_roots(home: &Path, slug: &str, mediatek: &Path, model: &str) -> Vec<Value> {
    let mut roots = vec![
        root(mediatek, "mediatek", true),
        root(&home.join("models"), &format!("models-{slug}"), false),
        root(&home.join("workspace"), &format!("ws-{slug}"), false),
        libraries_root(home),
    ];
    let held = |r: &Value| Path::new(model).starts_with(r["remote"].as_str().unwrap_or_default());
    let at = Path::new(model);
    if at.is_absolute()
        && let Some(dir) = at.parent()
        && !roots.iter().any(held)
    {
        roots.push(root(dir, &format!("model-{slug}"), false));
    }
    roots
}

/// What a cut shares on a server, with no account: the library writable, the libraries and the model's folder read-only.
pub fn library_cut_roots(data_root: &Path, mediatek: &Path, model: &str) -> Vec<Value> {
    let libs = data_root.join("libraries");
    let mut roots = vec![
        root(mediatek, "mediatek", true),
        json!({ "remote": libs.display().to_string(), "name": "libs", "writable": false, "perChild": true }),
    ];
    let held = |r: &Value| Path::new(model).starts_with(r["remote"].as_str().unwrap_or_default());
    let at = Path::new(model);
    if at.is_absolute()
        && let Some(dir) = at.parent()
        && !roots.iter().any(held)
    {
        roots.push(root(dir, "model-cut", false));
    }
    roots
}

/// What a server's slot shares with the machine: the libraries and the media, both read-only.
pub fn library_slot_roots(data_root: &Path, mediatek: &Path) -> Vec<Value> {
    let libs = data_root.join("libraries");
    vec![
        json!({ "remote": libs.display().to_string(), "name": "libs", "writable": false, "perChild": true }),
        root(mediatek, "mediatek", false),
    ]
}

async fn drop_paths(seam: &Seam, slug: &str, paths: Vec<String>, log: &Log) {
    for p in paths {
        if let Err(e) = seam.unbind(&p).await {
            log(&format!("[{slug}] could not unbind {p}: {e}"));
        }
    }
}

/// The engine an account drives and the loopback paths the tunnel hub bound for it.
#[derive(Default, Debug)]
pub struct Binding {
    pub engine_id: Option<String>,
    pub local_path: Option<String>,
    pub jobs_path: Option<String>,
    /// The name the service was told the engine by (its machine's hostname, else its id).
    pub engine_name: Option<String>,
}

/// One account's binding: who, where its home is, and the binding itself, changed by one caller at a time.
#[derive(Debug)]
pub struct Bound {
    pub user: String,
    pub slug: String,
    pub home: PathBuf,
    pub state: tokio::sync::Mutex<Binding>,
    /// When a request last asked for it, in milliseconds since the epoch: a fresh one is never let go.
    touched: AtomicU64,
}

/// What a runtime is told of a binding: `hubEngine`'s arguments, `{ "url": null }` for none.
pub fn told_json(seam: &Seam, b: &Binding) -> Value {
    match (&b.engine_id, &b.local_path) {
        (Some(id), Some(path)) => json!({
            "url": seam.ws_url(path),
            "jobsUrl": b.jobs_path.as_deref().map(|j| seam.ws_url(j)),
            "engineId": id,
            "engineName": b.engine_name.as_deref().unwrap_or(id),
        }),
        _ => json!({ "url": null }),
    }
}

/// Every account's binding, held by the hub whether an account's runtime or nothing serves the account now.
#[derive(Default)]
pub struct Bindings {
    by_user: Mutex<HashMap<String, Arc<Bound>>>,
}

impl Bindings {
    fn map(&self) -> std::sync::MutexGuard<'_, HashMap<String, Arc<Bound>>> {
        self.by_user.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The account's binding for a request, made empty on first asking.
    pub fn of(&self, user: &str, slug: &str, home: &Path) -> Arc<Bound> {
        let mut map = self.map();
        let bound = map.entry(user.to_string()).or_insert_with(|| {
            Arc::new(Bound {
                user: user.to_string(),
                slug: slug.to_string(),
                home: home.to_path_buf(),
                state: Default::default(),
                touched: AtomicU64::new(0),
            })
        });
        bound
            .touched
            .store(crate::account::now_ms(), Ordering::Relaxed);
        bound.clone()
    }

    /// The account's binding while it has one, never made for asking.
    pub fn get(&self, user: &str) -> Option<Arc<Bound>> {
        self.map().get(user).cloned()
    }
}

/// Drop both loopback paths a binding holds; a failed drop is said, never thrown.
async fn unbind_all(seam: &Seam, slug: &str, b: &mut Binding, log: &Log) {
    let paths = [b.local_path.take(), b.jobs_path.take()]
        .into_iter()
        .flatten()
        .collect();
    drop_paths(seam, slug, paths, log).await;
    b.engine_name = None;
}

/// Bind the account to engine `id` if it holds the claim; a no-op when bound to it.
pub async fn bind_engine(
    seam: &Arc<Seam>,
    acct: &Arc<Bound>,
    id: &str,
    mediatek: Option<&Path>,
    log: &Log,
) -> Result<(), String> {
    let mut b = acct.state.lock().await;
    if b.engine_id.as_deref() == Some(id) {
        return Ok(());
    }
    // Anyone may register an engine, so only one this account has CLAIMED ever sees its files.
    let Some(view) = seam.engine(id, &acct.user).await? else {
        return Ok(());
    };
    if view.get("holds") != Some(&Value::Bool(true)) {
        return Ok(());
    }
    let roots = Value::Array(roots_for(&acct.home, &acct.slug, mediatek));
    let path = seam.bind(Some(id), &roots, "engine", Some(true)).await?;
    let jobs = match seam.bind(Some(id), &roots, "jobs", None).await {
        Ok(j) => j,
        Err(e) => {
            drop_paths(seam, &acct.slug, vec![path], log).await;
            return Err(e);
        }
    };
    let name = view
        .get("hostname")
        .and_then(Value::as_str)
        .filter(|h| !h.is_empty())
        .unwrap_or(id);
    unbind_all(seam, &acct.slug, &mut b, log).await;
    *b = Binding {
        engine_id: Some(id.to_string()),
        local_path: Some(path),
        jobs_path: Some(jobs),
        engine_name: Some(name.to_string()),
    };
    log(&format!("[{}] drives engine {id}", acct.slug));
    tokio::spawn(refresh_plugins(
        seam.clone(),
        acct.clone(),
        id.to_string(),
        log.clone(),
    ));
    Ok(())
}

/// Let go of the bound engine once the account no longer holds its claim: the next account took it.
pub async fn release_if_unclaimed(seam: &Seam, acct: &Bound, log: &Log) -> Result<(), String> {
    let mut b = acct.state.lock().await;
    let Some(id) = b.engine_id.clone() else {
        return Ok(());
    };
    let holds = seam
        .engine(&id, &acct.user)
        .await?
        .and_then(|v| v.get("holds").cloned())
        == Some(Value::Bool(true));
    if holds {
        return Ok(());
    }
    unbind_all(seam, &acct.slug, &mut b, log).await;
    b.engine_id = None;
    log(&format!(
        "[{}] let go of engine {id}: its claim moved on",
        acct.slug
    ));
    Ok(())
}

/// The scan file's own shape, left alone when it already says the same: its mtime is in every project's revision.
pub fn write_scan_copy(plugins: &Value, file: &Path) -> std::io::Result<bool> {
    let text = crate::json::stringify(&json!({ "entries": [{ "plugins": plugins }] }));
    if std::fs::read_to_string(file).is_ok_and(|was| was == text) {
        return Ok(false);
    }
    let tmp = file.with_extension(format!("json.{}.tmp", std::process::id()));
    std::fs::write(&tmp, &text)?;
    std::fs::rename(&tmp, file).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })?;
    Ok(true)
}

/// The engine's plugin list, written as `<home>/exts.json`, and again when a scan running at the bind ends.
async fn refresh_plugins(seam: Arc<Seam>, acct: Arc<Bound>, id: String, log: Log) {
    let Some(mut ch) = seam
        .open(Some(&id), &json!([]), "engine", PLUGINS_TIMEOUT)
        .await
    else {
        return;
    };
    let at = acct.home.join("exts.json");
    let mut heard = false;
    let mut deadline = tokio::time::Instant::now() + PLUGINS_TIMEOUT;
    if ch
        .send(Message::text(r#"{"type":"list-plugins"}"#))
        .await
        .is_ok()
    {
        loop {
            let next = tokio::time::timeout_at(deadline, ch.next()).await;
            let Ok(Some(Ok(msg))) = next else {
                break;
            };
            let text = match msg {
                Message::Text(t) => t.to_string(),
                Message::Binary(b) => String::from_utf8_lossy(&b).into_owned(),
                Message::Close(_) => break,
                _ => continue,
            };
            let Ok(m) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            if m.get("type").and_then(Value::as_str) != Some("plugins") {
                continue;
            }
            let plugins = m
                .get("plugins")
                .filter(|p| p.is_array())
                .cloned()
                .unwrap_or(json!([]));
            if write_scan_copy(&plugins, &at).is_err() {
                continue;
            }
            heard = true;
            // A scan still running replaces this list when it ends, and broadcasts the new one.
            if m.get("scanning") == Some(&Value::Bool(true)) {
                deadline = tokio::time::Instant::now() + SCAN_LISTEN;
            } else {
                break;
            }
        }
    }
    let _ = ch.close(None).await;
    if !heard {
        log(&format!(
            "[{}] engine {id} never listed its plugins",
            acct.slug
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_the_scan_once_and_keeps_a_copy_that_says_the_same() {
        let dir = std::env::temp_dir().join(format!("k7-hub-exts-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("exts.json");
        let _ = std::fs::remove_file(&file);
        let plugins = json!([{ "format": "VST3", "name": "Amp", "n": 1.0 }]);
        assert!(write_scan_copy(&plugins, &file).unwrap());
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            r#"{"entries":[{"plugins":[{"format":"VST3","name":"Amp","n":1}]}]}"#
        );
        assert!(!write_scan_copy(&plugins, &file).unwrap());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn shares_five_roots_and_the_mediatek_read_only() {
        let home = Path::new("/d/ann-1");
        let r = roots_for(home, "ann-1", Some(Path::new("/m")));
        let names: Vec<_> = r
            .iter()
            .map(|v| v["name"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            names,
            [
                "ws-ann-1",
                "libs",
                "models-ann-1",
                "renders-ann-1",
                "history-ann-1",
                "mediatek"
            ]
        );
        assert_eq!(
            r[1]["remote"],
            Path::new("/d").join("libraries").display().to_string()
        );
        assert_eq!(r[5]["writable"], false);
        assert_eq!(
            cut_roots(home, "ann-1", Path::new("/m"), "/d/models/m.onnx")[0]["writable"],
            true
        );
    }

    #[test]
    fn a_cut_shares_the_folder_of_a_model_no_root_holds() {
        let base = std::env::temp_dir().join(format!("k7-hub-cut-{}", std::process::id()));
        let (home, mediatek) = (base.join("ann"), base.join("m"));
        let inside = home.join("models").join("m.onnx");
        let held = cut_roots(&home, "ann-1", &mediatek, &inside.display().to_string());
        assert_eq!(held.len(), 4);
        let outside = base.join("srv").join("models");
        let model = outside.join("m.onnx").display().to_string();
        let moved = cut_roots(&home, "ann-1", &mediatek, &model);
        assert_eq!(moved.len(), 5);
        assert_eq!(moved[4]["remote"], outside.display().to_string());
        assert_eq!(moved[4]["writable"], false);
    }
}
