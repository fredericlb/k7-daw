//! picture_run.rs — `POST /picture` and `POST /picture/scene`: the open project's picture and its scene, read by the `verbs` crate.
//! What a reader gets wrong: both read the compile cache the verbs keep, so a scene asked right after a line costs only what moved.

use axum::response::Response;
use k7_file_service::edits::picture::{Use, picture_doc, picture_scene};
use k7_file_service::edits::verb::{Line, Replay};
use k7_file_service::space::Space;
use serde_json::Value;

use super::Ctx;
use super::entries::blocking;
use super::journal::error;
use super::verb_run::{give_cache, take_cache};
use crate::routes::files::json_answer;
use crate::routes::files::service::routes::Call;

/// The longest `/media` prefix a scene takes: a base URL and three query fields.
const MAX_MEDIA: usize = 4096;

/// Runs `read` over the open project with the verbs' cache for it.
async fn with_project(
    ctx: &Ctx,
    read: impl FnOnce(&Line<'_>, &mut song::Cache) -> Result<Value, String> + Send + 'static,
) -> Response {
    let Some(code_root) = ctx.docs.as_ref().map(|d| d.code_root.clone()) else {
        return error(503, "file service: no code root to read k7's modules from");
    };
    let ctx2 = ctx.clone();
    let answer = blocking(move || {
        let project = Space::open(&ctx2.home, &ctx2.root).active().ok().flatten();
        let key = format!("{}\n{}", ctx2.account, project.as_deref().unwrap_or(""));
        let mut cache = take_cache(&key);
        let at = Line {
            home: &ctx2.home,
            root: &ctx2.root,
            code_root: &code_root,
            project: project.as_deref(),
            line: "",
            replay: Replay::default(),
            chain: false,
        };
        let out = read(&at, &mut cache);
        give_cache(key, cache);
        out
    })
    .await;
    match answer {
        Some(Ok(v)) => json_answer(200, &v),
        Some(Err(e)) => error(500, &e),
        None => error(500, "internal error"),
    }
}

/// `POST /picture`: `{doc, warnings}` with every media path workspace-relative, or `{error}`.
pub async fn doc(_c: &Call, ctx: &Ctx) -> Response {
    with_project(ctx, picture_doc).await
}

/// `POST /picture/scene {media, armed?, use?}`: `{scene, mixes}`, each file `media` followed by its workspace path, or `{error}`.
pub async fn scene(c: &Call, ctx: &Ctx) -> Response {
    let Some(media) = c
        .body
        .get("media")
        .and_then(Value::as_str)
        .map(str::to_owned)
    else {
        return error(400, "a scene needs its 'media' prefix");
    };
    if media.len() > MAX_MEDIA {
        return error(413, "the media prefix is too long");
    }
    let armed = c
        .body
        .get("armed")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let use_ = match c.body.get("use").and_then(Value::as_str) {
        None | Some("preview") => Use::Preview,
        Some("export") => Use::Export,
        Some(_) => return error(400, "use must be preview or export"),
    };
    with_project(ctx, move |at, cache| {
        picture_scene(at, cache, armed.as_deref(), use_, &media)
    })
    .await
}
