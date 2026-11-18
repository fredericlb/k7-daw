//! verb_bundle.rs — `GET /verb-bundle` and `GET /verb-bundle/library` (3d): what the browser's verbs read, and a library by its hash.
//! What a reader gets wrong: the token is read BEFORE the walk, so a bundle is never newer than its `rev` and a stale one is a 409 later.

use axum::response::Response;
use hyper::header::{CACHE_CONTROL, HeaderValue};
use k7_file_service::changes::Refusal;
use k7_file_service::changes::target::target;
use k7_file_service::confine::Disk;
use k7_file_service::revisions::Revisions;
use k7_file_service::rig::rig;
use k7_file_service::song_ask::{Input, song_roots};
use k7_file_service::space::Space;
use k7_file_service::verb_bundle::{Inputs, build, library_files, library_roots};
use serde_json::json;

use super::answering::TREE_WEIGHT;
use super::entries::blocking;
use super::journal::{busy, error};
use super::{Ctx, Tag};
use crate::routes::files::json_answer;
use crate::routes::files::service::body;
use crate::routes::files::service::routes::Call;

/// The bundle of the open project, or one library of it by hash.
pub async fn answer(c: &Call, ctx: &Ctx) -> Response {
    let Some(docs) = ctx.docs.clone() else {
        return error(
            503,
            "file service: the verb bundle needs the native song compiler",
        );
    };
    // An empty `at` is the workspace alone; a bundle without the parameter is a mistake.
    let has_at = url::form_urlencoded::parse(c.query.as_bytes()).any(|(k, _)| k == "at");
    if !has_at {
        return error(400, "at required");
    }
    let at = body::query(&c.query, "at");
    let (home, root, at2) = (ctx.home.clone(), ctx.root.clone(), at.clone());
    let target = match blocking(move || target(&Space::open(&home, &root), &at2)).await {
        Some(Ok(t)) => t,
        Some(Err(Refusal::Elsewhere { .. })) => {
            return error(409, &format!("'{at}' is not the open project"));
        }
        Some(Err(r)) => return error(r.status(), &r.to_string()),
        None => return error(500, "internal error"),
    };
    let active = target.journal_dir(&ctx.root);
    let Some(_held) = ctx.gate.take(&ctx.account, TREE_WEIGHT).await else {
        return busy();
    };
    let ctx2 = ctx.clone();
    let (library, name, hash) = (
        ctx.tag == Tag::VerbLibrary,
        body::query(&c.query, "name"),
        body::query(&c.query, "hash"),
    );
    let made = blocking(move || {
        let input = Input {
            home: &ctx2.home,
            project: &active,
            code_root: &docs.code_root,
            env: &docs.env,
        };
        let roots =
            song_roots(&input, &Disk).map_err(|d| (503, format!("declined: {}", d.as_str())))?;
        if library {
            let dir = library_roots(&roots)
                .into_iter()
                .find(|(n, _)| *n == name)
                .map(|(_, d)| d)
                .ok_or((404, format!("no library '{name}' for this project")))?;
            let lib = library_files(&dir).map_err(|r| (r.status, r.error))?;
            if lib.hash().0 != hash {
                return Err((404, format!("library '{name}' has no such version")));
            }
            return Ok(
                json!({ "name": name, "hash": hash, "files": lib.files, "lengths": lib.lengths }),
            );
        }
        let space = Space::open(&ctx2.home, &ctx2.root);
        let dir = target.project_dir();
        let rev = Revisions::global().token(&ctx2.home, space.root(), dir, false);
        let rig = rig(&ctx2.home).map_err(|d| (503, format!("declined: {}", d.as_str())))?;
        build(&Inputs {
            home: &ctx2.home,
            root: space.root(),
            project: &active,
            at: &at,
            rev: &rev,
            roots,
            rig,
            code_root: &docs.code_root,
        })
        .map_err(|r| (r.status, r.error))
    })
    .await;
    match made {
        Some(Ok(v)) => {
            let mut r = json_answer(200, &v);
            if library {
                r.headers_mut().insert(
                    CACHE_CONTROL,
                    HeaderValue::from_static("private, max-age=31536000, immutable"),
                );
            }
            r
        }
        Some(Err((status, msg))) => error(status, &msg),
        None => error(500, "internal error"),
    }
}
