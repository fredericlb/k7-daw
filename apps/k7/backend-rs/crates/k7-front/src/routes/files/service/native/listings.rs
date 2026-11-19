//! listings.rs — the five listings (playlists, references, the libraries' chains and assemblies, `/module-libraries`)
//! read natively (1404 to 1408, 1431, 1440 to 1444). The four ops go through `reads`' one `compute_with`;
//! `/module-libraries` has its own hook; the versions it lists are the ones held here.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use axum::response::Response;
use http_body_util::Full;
use hyper::HeaderMap;
use hyper::body::Bytes;
use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use k7_file_service::listings::{Listing, list_gated};
use k7_file_service::space::Space;
use k7_file_service::tree::Limits;
use serde_json::Value;

use super::reads::{self, Json, Native, Outcome};
use super::{Ctx, Req, Tag, ctx_for};
use crate::routes::files::service::{Who, workspace};

/// The listing a tag names.
fn listing_of(tag: Tag) -> Option<Listing> {
    Some(match tag {
        Tag::Playlists => Listing::Playlists,
        Tag::References => Listing::References,
        Tag::LibraryChains => Listing::LibraryChains,
        Tag::LibraryAssemblies => Listing::LibraryAssemblies,
        Tag::ModuleLibraries => Listing::ModuleLibraries,
        _ => return None,
    })
}

/// Whether the listing weighs a tree while it runs (1442); a playlist listing grows by its texts, the references stay small.
pub fn weighs_a_tree(tag: Tag) -> bool {
    matches!(
        tag,
        Tag::ModuleLibraries | Tag::LibraryChains | Tag::LibraryAssemblies
    )
}

/// The native listing; `admit` is asked the size of each playlist's text.
pub fn compute_with(ctx: &Ctx, admit: &dyn Fn(u64) -> bool) -> Outcome {
    let Some(which) = listing_of(ctx.tag) else {
        return Outcome::Declined(k7_file_service::space::Declined::Busy);
    };
    let space = Space::open(&ctx.home, &ctx.root);
    let remote = ctx.remote.as_deref().cloned().unwrap_or_default();
    match list_gated(&space, which, &remote, &Limits::default(), admit) {
        Ok(l) => Outcome::Answer(Native {
            status: 200,
            body: Json::Text(l.json),
            unlistable: u32::try_from(l.unlisted.len()).unwrap_or(u32::MAX),
            warn: None,
        }),
        Err(d) => Outcome::Declined(d),
    }
}

/// The hub's address and secret in the account's environment; no address, no hub.
pub fn hub_of(env: &BTreeMap<String, String>) -> Option<(String, String)> {
    let url = env.get("STUDIO_HUB_URL").filter(|u| !u.is_empty())?;
    let secret = env.get("STUDIO_HUB_SECRET").cloned().unwrap_or_default();
    Some((url.clone(), secret))
}

type Https = Client<HttpsConnector<HttpConnector>, Full<Bytes>>;

pub(crate) fn http() -> &'static Https {
    static CLIENT: OnceLock<Https> = OnceLock::new();
    CLIENT.get_or_init(crate::https::client)
}

/// The context of a `GET /module-libraries` whose workspace the header names (1441); else `None`.
pub fn live_ctx(who: &Who<'_>, headers: &HeaderMap, query: &str) -> Option<Ctx> {
    let (name, root) = workspace(who, headers).ok()?;
    let root = root.display().to_string();
    let account = who.account();
    ctx_for(
        Some(Tag::ModuleLibraries),
        &Req {
            env: &account.env,
            slug: &account.slug,
            workspace: &name,
            root: &root,
            query,
            body: &Value::Null,
            log: who.log,
            docs: None,
            thawed: None,
        },
    )
}

/// `/module-libraries` answered natively (1440); a decline is refused.
pub async fn live(_who: &Who<'_>, ctx: Ctx) -> Response {
    answer(ctx).await
}

/// The levels' listing over the versions held here, or the refusal of its decline.
pub async fn answer(ctx: Ctx) -> Response {
    let Some(n) = reads::run(&ctx).await else {
        return super::refuse::refused();
    };
    if n.unlistable > 0 {
        (ctx.log)(&format!(
            "[files] native: {} folder(s) of the levels could not be listed",
            n.unlistable
        ));
    }
    reads::response(n.status, n.body)
}
