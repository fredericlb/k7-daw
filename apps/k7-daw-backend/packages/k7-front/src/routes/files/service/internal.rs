//! internal.rs — the media's cut: its chain resolved in the account's own context (`docs/files/hub.md`). What a reader gets wrong: the answer is shaped as node's route answered it.

use serde_json::{Value, json};

use super::Held;
use super::native::{self, Tag};
use super::native::{bench, chains};

/// `/hub/cold-chain`'s answer (`resolveColdRun`): `{model, outputs}`, or the refusal's status.
pub async fn cold_run(held: &Held, chain: &str, want: &str) -> Result<(u16, Value), String> {
    let (a, acct, root) = (&held.account, &held.acct, &held.code_root);
    resolve(
        &a.env,
        &a.slug,
        (acct, root.as_path()),
        &held.log,
        chain,
        want,
    )
    .await
}

async fn resolve(
    env: &std::collections::BTreeMap<String, String>,
    slug: &str,
    at: (&std::sync::Arc<super::accounts::Account>, &std::path::Path),
    log: &k7_hub::Log,
    chain: &str,
    want: &str,
) -> Result<(u16, Value), String> {
    let c = &chains::ctx_of(Tag::ColdRun, env, slug, log, Some(at));
    // An answering piece that declines refuses with a sentence, as a route does (4-7).
    let (done, why) = native::refuse::scoped(async {
        let done = chains::cold_run_answer(c, want, chain).await;
        (done, native::refuse::last())
    })
    .await;
    match done {
        Some(done) => Ok(done),
        None => {
            let (status, error) = native::refuse::sentence(why);
            Ok((status, json!({ "error": error, "declined": why })))
        }
    }
}

/// `POST /account/slot` as the hub's slot route asks it: answered natively, or a 503 where the compile could not run (Z2-8).
pub async fn slot(held: &Held, r#ref: &str) -> Result<(u16, Value), String> {
    let a = &held.account;
    let run = bench::Run::of(&held.acct, &a.env, &a.slug, &held.code_root, &held.log);
    compile(run, r#ref).await
}

async fn compile(run: bench::Run, r#ref: &str) -> Result<(u16, Value), String> {
    if let Err(m) = super::body::check(
        &json!({ "ref": r#ref }),
        &[("ref", super::body::Field::Text)],
    ) {
        return Ok((400, json!({ "error": m })));
    }
    match bench::slot_for_hub(run, r#ref).await {
        Some(done) => Ok(done),
        None => Ok((
            503,
            json!({ "error": "the slot could not be compiled; retry once the machine is idle" }),
        )),
    }
}
