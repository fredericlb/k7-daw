//! routes.rs — the five routes of the jobs piece: an audition, a plugin's parameters and window, a base's snapshot and a thaw (2340 to 2345).
//! What a reader gets wrong: a decline hands the request back WHOLE, rebuilt from the bytes already read, and no answer waits on a lock.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use axum::body::{Body, Bytes};
use axum::extract::Request;
use axum::response::Response;
use http_body_util::BodyExt;
use hyper::header::CONTENT_TYPE;
use k7_file_service::confine::Disk;
use k7_file_service::entries::EntryError;
use k7_file_service::entries::apply::make_folder;
use k7_file_service::jobs::base_path::{BasePath, base_path};
use k7_file_service::jobs::plugin::resolve_plugin;
use k7_file_service::libraries::Home;
use k7_file_service::paths::join;
use k7_file_service::space::Space;
use serde_json::{Value, json};
use song::DiskSource;
use song::modules::Scan;

use super::Jobs;
use super::preview::{World, request_of, resolve};
use super::queries::{self, Reply};
use crate::routes::files::json_answer;
use crate::routes::files::service::body::{self, Field};
use crate::routes::files::service::native::engine::routes::{Engine, declined};
use crate::routes::files::service::native::entries::{blocking, told};
use crate::routes::files::service::native::projects::error;
use crate::routes::files::service::native::{Tag, locks};
use crate::routes::files::service::{Who, workspace};

const SNAPSHOT: [(&str, Field); 3] = [
    ("path", Field::Text),
    ("name", Field::Text),
    ("ref", Field::Text),
];
const THAW: [(&str, Field); 2] = [("module", Field::Text), ("on", Field::Bool)];

/// What a route needs of the account, so a test can hold it without a whole front.
pub struct Deps<'a> {
    pub engine: &'a Arc<Engine>,
    pub jobs: &'a Jobs,
    pub home: Home,
    pub env: &'a BTreeMap<String, String>,
    pub code_root: PathBuf,
}

/// The five routes: answered here, or the request handed back to be refused.
#[allow(
    clippy::result_large_err,
    reason = "a request it does not answer is handed back whole"
)]
pub async fn hook(
    who: &Who<'_>,
    engine: &Arc<Engine>,
    path: &str,
    req: Request,
) -> Result<Response, Request> {
    let tag = match (req.method(), path) {
        (&hyper::Method::GET, "/plugin-params") => Tag::PluginParams,
        (&hyper::Method::GET, "/plugin-native-ui") => Tag::PluginNativeUi,
        (&hyper::Method::POST, "/module-base/snapshot") => Tag::ModuleBaseSnapshot,
        (&hyper::Method::POST, "/preview") => Tag::Preview,
        (&hyper::Method::POST, "/auto-freeze/thaw") => Tag::AutoFreezeThaw,
        _ => return Err(req),
    };
    let Some(jobs) = engine.jobs.get() else {
        return Err(req);
    };
    engine.host.note_admin(who.admin);
    let deps = Deps {
        engine,
        jobs,
        home: Home::of(&who.held.account.env, who.admin),
        env: &who.held.account.env,
        code_root: who
            .code_root
            .map(std::path::Path::to_path_buf)
            .unwrap_or_default(),
    };
    if matches!(tag, Tag::PluginParams | Tag::PluginNativeUi) {
        return Ok(plugin(&deps, tag, req.uri().query().unwrap_or("")).await);
    }
    let (parts, payload) = req.into_parts();
    let parsed = match read_body(&parts, payload).await {
        Ok(p) => p,
        Err(r) => return Ok(r),
    };
    let root = match workspace(who, &parts.headers) {
        Ok((_, root)) => root.display().to_string(),
        Err(r) => return Ok(*r),
    };
    match act(&deps, tag, &root, &parsed.1).await {
        Ok(r) => Ok(r),
        Err(why) => {
            declined(who, tag, why);
            Err(Request::from_parts(parts, Body::from(parsed.0)))
        }
    }
}

/// The body as `express.json` reads it: its bytes, kept to hand the request back, and the value.
#[allow(clippy::result_large_err, reason = "the answer to send is the error")]
async fn read_body(
    parts: &hyper::http::request::Parts,
    payload: Body,
) -> Result<(Bytes, Value), Response> {
    let bytes = match http_body_util::Limited::new(payload, body::JSON_LIMIT)
        .collect()
        .await
    {
        Ok(b) => b.to_bytes(),
        Err(_) => return Err(error(413, "request entity too large")),
    };
    let content_type = parts
        .headers
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok());
    match body::json(content_type, &bytes) {
        Ok(v) => Ok((bytes, v)),
        Err(m) => Err(error(400, &m)),
    }
}

/// The three POSTs once their body and workspace are known: the fields held, then the act; a decline names its reason.
pub async fn act(
    d: &Deps<'_>,
    tag: Tag,
    root: &str,
    parsed: &Value,
) -> Result<Response, &'static str> {
    let checked = match tag {
        Tag::Preview => check_preview(parsed),
        Tag::ModuleBaseSnapshot => body::check(parsed, &SNAPSHOT),
        _ => body::check(parsed, &THAW),
    };
    if let Err(m) = checked {
        return Ok(error(400, &m));
    }
    match tag {
        Tag::Preview => preview(d, root, parsed).await,
        Tag::ModuleBaseSnapshot => snapshot(d, root, parsed).await,
        _ => thaw(d, root, parsed).await,
    }
}

/// The reply of a query as the route sends it.
fn sent((status, body): Reply) -> Response {
    json_answer(status, &body)
}

/// `resolvePlugin` over the scan copy under the studio home; the scan is read only for a `ref`, which is all it serves.
fn pair(
    home: &Home,
    r#ref: &str,
    format: &str,
    id: &str,
) -> Result<(String, String), (u16, String)> {
    let exts = join(&[&home.studio, "exts.json"]);
    let scan = if r#ref.is_empty() {
        Scan::default()
    } else {
        DiskSource::with_files(&[] as &[&str], &[exts.as_str()])
            .ok()
            .and_then(|source| Scan::read(&source, Some(&exts)).ok())
            .unwrap_or_default()
    };
    resolve_plugin(&scan, r#ref, format, id)
}

/// The plugin as the request names it, or the answer to send; the disk is read off the runtime.
#[allow(clippy::result_large_err, reason = "the answer to send is the error")]
async fn named(
    home: &Home,
    r#ref: String,
    format: String,
    id: String,
) -> Result<(String, String), Response> {
    let home = home.clone();
    match blocking(move || pair(&home, &r#ref, &format, &id)).await {
        Some(Ok(found)) => Ok(found),
        Some(Err((status, message))) => Err(error(status, &message)),
        None => Err(error(500, "internal error")),
    }
}

/// `GET /plugin-params` and `GET /plugin-native-ui`: account routes, so no workspace is asked.
pub async fn plugin(d: &Deps<'_>, tag: Tag, query: &str) -> Response {
    let ask = |k: &str| body::query(query, k);
    let (format, id) = match named(&d.home, ask("ref"), ask("format"), ask("id")).await {
        Ok(found) => found,
        Err(refused) => return refused,
    };
    d.jobs.counts.queries.fetch_add(1, Ordering::Relaxed);
    sent(if tag == Tag::PluginParams {
        queries::params(&d.engine.link, &format, &id).await
    } else {
        queries::window(&d.engine.link, &format, &id).await
    })
}

/// zod's `discriminatedUnion("kind", …)`: the branch the `kind` names, or its own sentence for none.
fn check_preview(parsed: &Value) -> Result<(), String> {
    let schema: &[(&str, Field)] = match parsed.get("kind").and_then(Value::as_str) {
        Some("source") => &[("source", Field::Text)],
        Some("clip") => &[("clip", Field::Text)],
        Some("slice") => &[
            ("source", Field::Text),
            ("from", Field::Num),
            ("to", Field::Num),
        ],
        _ if parsed.is_object() => {
            return Err("Invalid discriminator value. Expected 'source' | 'clip' | 'slice'".into());
        }
        _ => &[],
    };
    body::check(parsed, schema)
}

/// A number as JavaScript prints it: a whole one without its fraction.
fn num(f: f64) -> Value {
    if f.fract() == 0.0 && f.abs() < 1e15 {
        json!(f as i64)
    } else {
        json!(f)
    }
}

/// The open project of the workspace, or the reason this front declines to say.
async fn active_of(home: &Home, root: &str) -> Result<Option<String>, &'static str> {
    let (home, root) = (home.clone(), root.to_owned());
    match blocking(move || Space::open(&home, &root).active()).await {
        Some(Ok(active)) => Ok(active),
        Some(Err(d)) => Err(d.as_str()),
        None => Ok(None),
    }
}

/// `POST /preview`: the window resolved off the runtime, its frame queued in the workspace's session after any document.
async fn preview(d: &Deps<'_>, root: &str, parsed: &Value) -> Result<Response, &'static str> {
    let active = active_of(&d.home, root).await?;
    let req = request_of(parsed);
    let (host, home, code, models, root2) = (
        d.engine.host.clone(),
        d.home.clone(),
        d.code_root.clone(),
        d.env.get("STUDIO_MODELS").cloned(),
        root.to_owned(),
    );
    let found = blocking(move || {
        let world = World {
            host,
            home: &home,
            code_root: &code,
            models: models.as_deref(),
            root: &root2,
            active: active.as_deref(),
        };
        resolve(&world, &req)
    })
    .await;
    let Some(found) = found else {
        return Ok(error(500, "internal error"));
    };
    d.jobs.counts.previews.fetch_add(1, Ordering::Relaxed);
    Ok(match found {
        Ok(w) => {
            let (start, length) = (num(w.start), num(w.length));
            let frame =
                json!({ "type": "preview", "file": w.file, "start": start, "length": length });
            d.engine.link.session(Some(root)).frame(frame);
            json_answer(
                200,
                &json!({ "ok": true, "start": start, "length": length }),
            )
        }
        Err(refused) => json_answer(refused.status(), &json!({ "error": refused.message })),
    })
}

/// `baseSnapshotPathOf` and its folder: judged, then made under the account's write lock and told to the engine.
async fn base_folder(
    d: &Deps<'_>,
    root: &str,
    path: &str,
    name: &str,
) -> Result<Result<BasePath, EntryError>, &'static str> {
    let (home, root2, path, name) = (
        d.home.clone(),
        root.to_owned(),
        path.to_owned(),
        name.to_owned(),
    );
    let lock = locks::write(&d.home.studio).await;
    let root3 = root.to_owned();
    // A folder begun is finished: a client gone meanwhile must not leave it half made.
    let work = tokio::spawn(async move {
        let _lock = lock;
        let made = blocking(move || {
            let space = Space::open(&home, &root2);
            let planned = base_path(&space, &path, &name, &Disk)?;
            let fresh = std::fs::metadata(&planned.dir).is_err();
            make_folder(&planned.dir)?;
            Ok::<_, EntryError>((planned, fresh))
        })
        .await;
        match made {
            Some(Ok((planned, fresh))) => {
                if fresh {
                    told(&root3, "after").await;
                }
                Ok(Ok(planned))
            }
            Some(Err(EntryError::Declined(d))) => Err(d.as_str()),
            Some(Err(refused)) => Ok(Err(refused)),
            None => Ok(Err(EntryError::Refused("internal error".into()))),
        }
    });
    work.await
        .unwrap_or(Ok(Err(EntryError::Refused("internal error".into()))))
}

/// `POST /module-base/snapshot`: the plugin, the folder, then the engine's word that the state was written.
async fn snapshot(d: &Deps<'_>, root: &str, parsed: &Value) -> Result<Response, &'static str> {
    let text = |k: &str| body::text(parsed, k);
    let (format, id) = match named(&d.home, text("ref"), String::new(), String::new()).await {
        Ok(found) => found,
        Err(refused) => return Ok(refused),
    };
    let planned = match base_folder(d, root, &text("path"), &text("name")).await? {
        Ok(p) => p,
        Err(refused) => return Ok(error(400, &refused.to_string())),
    };
    d.jobs.counts.queries.fetch_add(1, Ordering::Relaxed);
    let link = &d.engine.link;
    Ok(sent(
        queries::base(link, &format, &id, &planned.abs, &planned.path).await,
    ))
}

/// `POST /auto-freeze/thaw`: the module left live (or let go) in the open project, which is then recompiled and sent.
async fn thaw(d: &Deps<'_>, root: &str, parsed: &Value) -> Result<Response, &'static str> {
    if let Some(project) = active_of(&d.home, root).await? {
        d.jobs.thaw(
            &project,
            &body::text(parsed, "module"),
            parsed["on"] == true,
        );
        let root = root.to_owned();
        d.engine
            .link
            .session(Some(&root))
            .enqueue(move |s| async move { s.reload(&root, &project).await });
    }
    Ok(json_answer(200, &json!({ "ok": true })))
}
