//! orphans.rs — the reads no piece owned: a module's panel, engines and bases, the console systems (F1), the project's plugins, compat and clips (F2).
//! What a reader gets wrong: the open project is the runtime's memory, asked once; a refusal is the TypeScript's sentence in a 400.

use axum::response::Response;
use k7_file_service::module_reads::{self as reads, Failure, Reading};
use serde_json::{Value, json};

use super::entries::{blocking, error};
use super::{Ctx, Tag};
use crate::routes::files::json_answer;
use crate::routes::files::service::body;
use crate::routes::files::service::routes::{Call, H};

/// The routes this file answers.
pub fn owns(h: H) -> bool {
    matches!(
        h,
        H::ModuleUi | H::Consoles | H::ModuleEngines | H::ModuleBase
    )
}

/// The open project the runtime holds, or why the runtime must answer.
async fn active_of(ctx: &Ctx) -> Result<Option<String>, ()> {
    match super::docs::asked_of(&ctx.home, &ctx.root).await {
        Ok(asked) => Ok(asked.active),
        Err(why) => {
            ctx.declined(why);
            Err(())
        }
    }
}

/// What a read answered, as the route words it: a sentence is a 400, a decline is a refusal.
fn finish(ctx: &Ctx, done: Result<Value, Failure>, key: Option<&str>) -> Option<Response> {
    match done {
        Ok(v) => Some(json_answer(
            200,
            &match key {
                Some(k) => json!({ k: v }),
                None => v,
            },
        )),
        Err(Failure::Say(s)) => Some(error(400, &s)),
        Err(Failure::Declined(d)) => {
            ctx.declined(d.as_str());
            None
        }
    }
}

/// One of the four module reads, answered natively or `None` where it declined.
pub async fn answer(h: H, c: &Call, ctx: &Ctx) -> Option<Response> {
    let code = ctx.docs.as_ref()?.code_root.clone();
    let path = body::query(&c.query, "path");
    if h == H::ModuleBase && path.is_empty() {
        return Some(error(400, "path is required"));
    }
    let needs_project = !matches!(h, H::ModuleEngines);
    let active = if needs_project {
        active_of(ctx).await.ok()?
    } else {
        None
    };
    let (home, root) = (ctx.home.clone(), ctx.root.clone());
    let done = blocking(move || {
        let r = Reading {
            home: &home,
            code_root: &code,
            root: &root,
            active: active.as_deref(),
        };
        match h {
            H::ModuleUi => reads::module_ui(&r, &path),
            H::Consoles => reads::consoles(&r),
            H::ModuleEngines => reads::module_engines(&r, &path),
            _ => reads::module_bases(&r, &path).map(|b| json!(b)),
        }
    })
    .await?;
    let key = (h == H::ModuleBase).then_some("bases");
    finish(ctx, done, key)
}

/// The tag a route of this file carries.
pub fn tag_of(h: H) -> Option<Tag> {
    match h {
        H::ModuleUi => Some(Tag::ModuleUi),
        H::Consoles => Some(Tag::Consoles),
        H::ModuleEngines => Some(Tag::ModuleEngines),
        H::ModuleBase => Some(Tag::ModuleBase),
        _ => None,
    }
}
