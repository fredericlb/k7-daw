//! revision.rs — `GET /revision` answered from the native memo (1330 to 1335).
//! What a reader gets wrong: the token is the memo's, read under the gate; a busy gate declines rather than waits.

use axum::response::Response;
use k7_file_service::revisions::Revisions;
use k7_file_service::space::{Declined, Space};
use serde_json::json;

use super::Ctx;
use crate::routes::files::json_answer;

/// What a token weighs at the gate: a stack of names and a few stats (1335).
const WEIGHT: usize = 1 << 20;

/// The active project and the token the memo gives it, or why the route is refused (1334).
fn token_of(ctx: &Ctx) -> Result<String, Declined> {
    let space = Space::open(&ctx.home, &ctx.root);
    let dir = space.active()?;
    Ok(Revisions::global().token(&ctx.home, space.root(), dir.as_deref(), false))
}

/// `GET /revision` answered natively, or `None` where it declined (1335).
pub async fn answer(ctx: &Ctx) -> Option<Response> {
    let Some(held) = ctx.gate.take(&ctx.account, WEIGHT).await else {
        ctx.declined(Declined::Busy.as_str());
        return None;
    };
    let ctx2 = ctx.clone();
    match tokio::task::spawn_blocking(move || {
        let _held = held;
        token_of(&ctx2)
    })
    .await
    {
        Ok(Ok(rev)) => Some(json_answer(200, &json!({ "rev": rev }))),
        Ok(Err(d)) => {
            ctx.declined(d.as_str());
            None
        }
        Err(_) => None,
    }
}
