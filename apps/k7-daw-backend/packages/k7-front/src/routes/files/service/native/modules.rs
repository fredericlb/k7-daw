//! modules.rs — the library move written natively: judged, the folder renamed, the refs repointed, all under the journal's gate then the lock.
//! What a reader gets wrong: the gate comes first (an open step must not absorb the write); a repoint that fails leaves the folder moved.

use axum::response::Response;
use k7_file_service::edits::module_make::{Made, clone_module, create_module, create_node_of};
use k7_file_service::edits::modules::{LibraryMove, plan_library_move};
use k7_file_service::edits::presets::node_presets;
use k7_file_service::edits::project_create::create_project;
use k7_file_service::entries::EntryError;
use k7_file_service::entries::apply::{Disk, RETRY};
use k7_file_service::journal::Journals;
use k7_file_service::libraries::Home;
use k7_file_service::listings::engine::engine_catalogue;
use k7_file_service::paths::resolve_all;
use k7_file_service::rel::to_rel;
use k7_file_service::revisions::Revisions;
use k7_file_service::space::Space;
use serde_json::json;
use std::sync::Arc;

use super::entries::{blocking, error, finished, refusal, rename_retrying, told};
use super::{Ctx, Tag, locks};
use crate::routes::files::json_answer;
use crate::routes::files::service::body::{self, Field};
use crate::routes::files::service::routes::{self, Call, H};
use k7_file_service::edits;

/// The routes this file answers.
pub fn owns(h: H) -> bool {
    matches!(
        h,
        H::LibraryMove | H::Module | H::ModuleClone | H::Node | H::Presets | H::ProjectCreate
    )
}

/// The route answered natively, or `None` where it declined.
pub async fn answer(h: H, c: &Call, ctx: &Ctx) -> Option<Response> {
    match h {
        H::LibraryMove => library_move(c, ctx).await,
        H::Module => module_create(c, ctx).await,
        H::ModuleClone => module_clone(c, ctx).await,
        H::Node => node_create(c, ctx).await,
        H::Presets => presets(c, ctx).await,
        H::ProjectCreate => project_create(c, ctx).await,
        _ => None,
    }
}

/// A refusal as `/library/move` words it: every throw is a 400.
fn refused(e: &EntryError, warn: &(dyn Fn(&str) + Sync)) -> Response {
    match e {
        EntryError::Io { .. } => refusal(Tag::LibraryMove, e, warn),
        _ => error(400, &e.to_string()),
    }
}

async fn library_move(c: &Call, ctx: &Ctx) -> Option<Response> {
    if let Some(r) = routes::invalid(c, &routes::PATH_TO) {
        return Some(r);
    }
    let warn = |l: &str| c.say("warn", l);
    // The gate alone, then the account's lock, as a take's filing does.
    let gate = Journals::global()
        .exclusive(&resolve_all(&[&ctx.root]))
        .await;
    let lock = locks::write(&ctx.home.studio).await;
    let (path, to_dir) = (body::text(&c.body, "path"), body::text(&c.body, "toDir"));
    let (home, root) = (ctx.home.clone(), ctx.root.clone());
    let planned =
        blocking(move || plan_library_move(&Space::open(&home, &root), &path, &to_dir)).await?;
    let plan = match planned {
        Ok(p) => p,
        Err(EntryError::Declined(d)) => {
            ctx.declined(d.as_str());
            return None;
        }
        Err(e) => return Some(refused(&e, &warn)),
    };
    let (c, ctx) = (c.clone(), ctx.clone());
    Some(
        finished(async move {
            let _held = (gate, lock);
            written(&c, &ctx, plan).await
        })
        .await,
    )
}

async fn written(c: &Call, ctx: &Ctx, plan: LibraryMove) -> Response {
    let warn = |l: &str| c.say("warn", l);
    if let Some(m) = &c.media {
        m.prewarms.cancel(&c.root, &m.waves).await;
    }
    told(&c.root, "before").await;
    let real = Arc::new(Disk);
    if let Err(e) = rename_retrying(real, &plan.from_abs, &plan.dest, RETRY.1).await {
        // `before` was told, so `after` is told too.
        told(&c.root, "after").await;
        return refused(&e, &warn);
    }
    let roots = vec![plan.root.clone(), ctx.root.clone()];
    let (from, to) = (plan.from.clone(), plan.to.clone());
    let done = blocking(move || edits::modules::repoint(&roots, &from, &to, false))
        .await
        .unwrap_or_else(|| Err(String::from("the repoint was lost")));
    told(&c.root, "after").await;
    let files = match done {
        Ok(f) => f,
        Err(e) => return error(400, &e),
    };
    if !files.is_empty() {
        Revisions::global().bump(Space::open(&ctx.home, &ctx.root).root());
    }
    let (home, root) = (ctx.home.clone(), ctx.root.clone());
    let body = blocking(move || {
        let space = Space::open(&home, &root);
        json!({
            "path": to_rel(&space, &plan.dest),
            "from": plan.from,
            "to": plan.to,
            "repointed": files.iter().map(|f| to_rel(&space, f)).collect::<Vec<_>>(),
        })
    })
    .await;
    match body {
        Some(v) => json_answer(200, &v),
        None => error(500, "internal error"),
    }
}

type Work = Box<dyn FnOnce(&Home, &str, &Space<'_>) -> Result<Made, EntryError> + Send>;

/// A write of the module routes: judged and landed on a blocking thread under the gate then the lock, told after.
async fn made_under_gate(
    c: &Call,
    ctx: &Ctx,
    schema: &[(&str, Field)],
    work: Work,
) -> Option<Response> {
    if let Some(r) = routes::invalid(c, schema) {
        return Some(r);
    }
    let code = ctx.docs.as_ref()?.code_root.clone();
    let gate = Journals::global()
        .exclusive(&resolve_all(&[&ctx.root]))
        .await;
    let lock = locks::write(&ctx.home.studio).await;
    let (c, ctx) = (c.clone(), ctx.clone());
    let task = tokio::spawn(async move {
        let _held = (gate, lock);
        let warn = |l: &str| c.say("warn", l);
        let (home, ws) = (ctx.home.clone(), ctx.root.clone());
        let made = blocking(move || work(&home, &code, &Space::open(&home, &ws))).await?;
        match made {
            Err(EntryError::Declined(d)) => {
                ctx.declined(d.as_str());
                None
            }
            Err(e) => Some(refused(&e, &warn)),
            Ok(made) => {
                if !made.landed.touched.is_empty() {
                    Revisions::global().bump(Space::open(&ctx.home, &ctx.root).root());
                }
                told(&c.root, "after").await;
                Some(json_answer(200, &made.answer))
            }
        }
    });
    task.await.ok().flatten()
}

/// `POST /module`: an empty module made in a listed library.
async fn module_create(c: &Call, ctx: &Ctx) -> Option<Response> {
    const SCHEMA: [(&str, Field); 2] = [("root", Field::Text), ("rel", Field::Text)];
    let (root, rel) = (body::text(&c.body, "root"), body::text(&c.body, "rel"));
    let work: Work =
        Box::new(move |home, code, space| create_module(home, code, space, &root, &rel));
    made_under_gate(c, ctx, &SCHEMA, work).await
}

/// `POST /module/clone`: the module of `path` copied under `newName` beside the open project.
async fn module_clone(c: &Call, ctx: &Ctx) -> Option<Response> {
    const SCHEMA: [(&str, Field); 2] = [("path", Field::Text), ("newName", Field::Text)];
    let (path, name) = (body::text(&c.body, "path"), body::text(&c.body, "newName"));
    let work: Work =
        Box::new(move |home, code, space| clone_module(home, code, space, &path, &name));
    made_under_gate(c, ctx, &SCHEMA, work).await
}

/// `POST /node`: one use of the module of `path` in the open project, named or the first free name.
async fn node_create(c: &Call, ctx: &Ctx) -> Option<Response> {
    const SCHEMA: [(&str, Field); 2] = [("path", Field::Text), ("node", Field::OptText)];
    let path = body::text(&c.body, "path");
    let node = c
        .body
        .get("node")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let work: Work = Box::new(move |home, code, space| {
        // The catalogue is the engine's scan; a scan this reader says less of is declined.
        let catalogue = engine_catalogue(home).map_err(EntryError::Declined)?;
        let mut mint = || {
            let mut b = [0u8; 16];
            let _ = getrandom::fill(&mut b);
            k7_file_service::media::models::uuid_v4(b)
        };
        create_node_of(
            home,
            code,
            space,
            &path,
            node.as_deref(),
            &catalogue,
            &mut mint,
        )
    });
    made_under_gate(c, ctx, &SCHEMA, work).await
}

/// `GET /presets`: a node's presets from every scope, read where nothing else writes it at once.
async fn presets(c: &Call, ctx: &Ctx) -> Option<Response> {
    let node = body::query(&c.query, "node");
    if node.is_empty() {
        return Some(error(400, "node required"));
    }
    let code = ctx.docs.as_ref()?.code_root.clone();
    let asked = match super::docs::asked_of(&ctx.home, &ctx.root).await {
        Ok(a) => a,
        Err(why) => {
            ctx.declined(why);
            return None;
        }
    };
    let Some(project) = asked.active else {
        return Some(error(400, "no active project"));
    };
    let home = ctx.home.clone();
    let listed = blocking(move || node_presets(&home, &code, &project, &node)).await?;
    Some(match listed {
        Ok(v) => json_answer(200, &v),
        Err(e) => error(400, &e),
    })
}

/// `POST /project/create`: a project made from its archetype at the workspace's root.
async fn project_create(c: &Call, ctx: &Ctx) -> Option<Response> {
    const SCHEMA: [(&str, Field); 2] = [
        ("shape", Field::Enum(routes::SHAPES)),
        ("name", Field::Text),
    ];
    let (shape, name) = (body::text(&c.body, "shape"), body::text(&c.body, "name"));
    let work: Work = Box::new(move |home, code, space| {
        let mut mint = || {
            let mut b = [0u8; 16];
            let _ = getrandom::fill(&mut b);
            k7_file_service::media::models::uuid_v4(b)
        };
        create_project(home, code, space, &shape, &name, &mut mint)
    });
    made_under_gate(c, ctx, &SCHEMA, work).await
}
