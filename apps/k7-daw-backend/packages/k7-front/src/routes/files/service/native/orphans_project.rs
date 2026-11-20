//! orphans_project.rs — `/project-plugins`, `/project-compat` and `/resources` answered natively over the shared song session (F2).
//! What a reader gets wrong: the project is read without being opened, so a throw is a 404 for the first two and a decline for the third.

use axum::response::Response;
use k7_file_service::listings::engine::engine_scan;
use k7_file_service::module_reads::Failure;
use k7_file_service::project_reads::{compat_of, plugin_needs, project_dir_of, resources};
use k7_file_service::space::Space;
use serde_json::{Value, json};
use song::lanes::rig::Rig;

use super::answering::BASE_WEIGHT;
use super::docs::{Asked, prepare_with};
use super::entries::error;
use super::stack::{Failed, on_stack};
use super::{Ctx, Tag};
use crate::routes::files::json_answer;
use crate::routes::files::service::body;
use crate::routes::files::service::routes::{Call, H};

/// The routes this file answers.
pub fn owns(h: H) -> bool {
    matches!(h, H::ProjectPlugins | H::ProjectCompat | H::Resources)
}

/// The tag a route of this file carries.
pub fn tag_of(h: H) -> Option<Tag> {
    match h {
        H::ProjectPlugins => Some(Tag::ProjectPlugins),
        H::ProjectCompat => Some(Tag::ProjectCompat),
        H::Resources => Some(Tag::Resources),
        _ => None,
    }
}

/// What the work came to: an answer, a sentence for the route's status, or the reason it declined.
enum Out {
    Done(Value),
    Say(String),
    Decline(&'static str),
}

/// The shared session's cache, the project mounted as the docs mount it.
fn with_session<R>(
    ctx: &Ctx,
    dir: &str,
    f: impl FnOnce(&song::DiskSource, &mut song::Cache, &song::Roots, Rig) -> R,
) -> Result<R, &'static str> {
    let docs = ctx.docs.as_ref().ok_or("no-docs")?;
    let p = prepare_with(&ctx.home, docs, Asked::of_project(dir, None))?;
    let songs = docs.songs();
    let rig = Rig::from_json(&p.ask.rig);
    let made = songs.with(&p.key, p.rule, |s| {
        s.with_cache(p.rule, |cache| f(&p.disk, cache, &p.ask.roots, rig))
    });
    made.map_err(|_| "retired")
}

fn work(h: H, ctx: &Ctx, project: &str, active: Option<&str>) -> Out {
    let space = Space::open(&ctx.home, &ctx.root);
    if h == H::Resources {
        let Some(dir) = active else {
            return Out::Done(json!([]));
        };
        return match with_session(ctx, dir, |disk, cache, _, _| {
            resources(disk, cache, &space, dir)
        }) {
            Ok(rows) => Out::Done(Value::Array(rows)),
            Err(why) => Out::Decline(why),
        };
    }
    let dir = match project_dir_of(&space, project) {
        Ok(d) => d,
        Err(Failure::Say(s)) => return Out::Say(s),
        Err(Failure::Declined(d)) => return Out::Decline(d.as_str()),
    };
    let needs = match with_session(ctx, &dir, |disk, cache, roots, rig| {
        plugin_needs(disk, cache, roots, rig)
    }) {
        Ok(Ok(n)) => n,
        Ok(Err(m)) => return Out::Say(m),
        Err(why) => return Out::Decline(why),
    };
    if h == H::ProjectPlugins {
        return Out::Done(Value::Array(needs.iter().map(|n| n.json()).collect()));
    }
    match engine_scan(&ctx.home) {
        Ok(scan) => Out::Done(compat_of(&needs, &scan.plugins).unwrap_or(Value::Null)),
        Err(d) => Out::Decline(d.as_str()),
    }
}

/// One of the three reads, or `None` where it declined.
pub async fn answer(h: H, c: &Call, ctx: &Ctx) -> Option<Response> {
    ctx.docs.as_ref()?;
    let project = body::query(&c.query, "project");
    if h != H::Resources && project.is_empty() {
        return Some(error(400, "project is required"));
    }
    let held = ctx.gate.take(&ctx.account, BASE_WEIGHT).await;
    if held.is_none() {
        ctx.declined("busy");
        return None;
    }
    let active = if h == H::Resources {
        match super::docs::asked_of(&ctx.home, &ctx.root).await {
            Ok(asked) => asked.active,
            Err(why) => {
                ctx.declined(why);
                return None;
            }
        }
    } else {
        None
    };
    let ctx2 = ctx.clone();
    let made = on_stack(move || work(h, &ctx2, &project, active.as_deref())).await;
    drop(held);
    let key = match h {
        H::ProjectPlugins => "plugins",
        H::ProjectCompat => "compat",
        _ => "resources",
    };
    match made {
        Ok(Out::Done(v)) => Some(json_answer(200, &json!({ key: v }))),
        Ok(Out::Say(s)) if h == H::Resources => {
            (ctx.log)(&format!("[native] resources: {s}"));
            ctx.declined("would-throw");
            None
        }
        Ok(Out::Say(s)) => Some(error(404, &s)),
        Ok(Out::Decline(why)) => {
            ctx.declined(why);
            None
        }
        Err(Failed::Panicked) => {
            (ctx.log)(&format!("[native] {} panicked", ctx.tag.route()));
            ctx.declined("panic");
            None
        }
        Err(Failed::Lost) => {
            ctx.declined("lost");
            None
        }
    }
}
