//! internal.rs — the médiathèque's cut: its chain resolved in the account's own context on a device, in the libraries'
//! on a server (`docs/files/hub.md`). What a reader gets wrong: the answer is shaped as node's route answered it.

use serde_json::{Value, json};

use super::native::{self, Tag};
use super::native::{bench, chains};
use super::{Cut, Held};

/// `/hub/cold-chain`'s answer (`resolveColdRun`): `{model, outputs}`, or the refusal's status.
pub async fn cold_run(held: &Held, chain: &str, want: &str) -> Result<(u16, Value), String> {
    let admin = held.admin.load(std::sync::atomic::Ordering::Relaxed);
    let (a, acct, root) = (&held.account, &held.acct, &held.code_root);
    resolve(
        &a.env,
        &a.slug,
        admin,
        (acct, root.as_path()),
        &held.log,
        chain,
        want,
    )
    .await
}

/// A server's cut: the libraries' context, so no account's chain setting and no drafts.
pub async fn cut_run(cut: &Cut, chain: &str, want: &str) -> Result<(u16, Value), String> {
    let a = &cut.account;
    let at = (&cut.acct, cut.code_root.as_path());
    resolve(&a.env, &a.slug, false, at, &cut.log, chain, want).await
}

async fn resolve(
    env: &std::collections::BTreeMap<String, String>,
    slug: &str,
    admin: bool,
    at: (&std::sync::Arc<super::accounts::Account>, &std::path::Path),
    log: &k7_hub::Log,
    chain: &str,
    want: &str,
) -> Result<(u16, Value), String> {
    let c = &chains::ctx_of(Tag::ColdRun, env, admin, slug, log, Some(at));
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
    let admin = held.admin.load(std::sync::atomic::Ordering::Relaxed);
    let a = &held.account;
    let run = bench::Run::of(
        &held.acct,
        &a.env,
        &a.slug,
        admin,
        &held.code_root,
        &held.log,
    );
    compile(run, r#ref).await
}

/// A server's slot: a chain of the libraries compiled in their context, no account's chain and no drafts.
pub async fn cut_slot(cut: &Cut, r#ref: &str) -> Result<(u16, Value), String> {
    let a = &cut.account;
    let run = bench::Run::of(&cut.acct, &a.env, &a.slug, false, &cut.code_root, &cut.log);
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
