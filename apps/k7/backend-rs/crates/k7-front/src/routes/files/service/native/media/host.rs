//! host.rs — the account's media site: where a route warms the waveforms of the project it opened.
//! What a reader gets wrong: the prewarm runs on in a task; a cancel begun after it retires it.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use k7_file_service::paths::basename;
use k7_hub::Log;
use serde_json::Value;

use super::Media;
use super::locate::Session;
use crate::routes::files::service::Acct;
use crate::routes::files::service::native::docs::Asked;
use crate::routes::files::service::native::{Ctx, Req, Tag};

/// What a call is answered from: the account's media and its place.
#[derive(Clone)]
pub struct Site {
    pub media: Arc<Media>,
    pub acct: Arc<Acct>,
    pub code_root: PathBuf,
    pub env: BTreeMap<String, String>,
    pub slug: String,
    pub log: Log,
}

impl Site {
    /// The prewarm of `ws`'s active project, begun now and run on in a task; a cancel begun after it retires it.
    pub(in crate::routes::files::service::native) fn prewarm(
        &self,
        ws: &str,
        admin: bool,
        asked: Asked,
    ) {
        let Ok(session) = self.session(ws, admin, asked) else {
            return;
        };
        let media = self.media.clone();
        let run = media.prewarms.begin(ws);
        let ws = ws.to_string();
        tokio::spawn(async move {
            let (waves, counts) = (media.waves.clone(), media.counts.clone());
            media
                .prewarms
                .run(&ws, run, waves, Arc::new(session), counts)
                .await;
        });
    }

    /// The locator of workspace `ws`, with the ask given (review fix 6).
    pub(super) fn session(&self, ws: &str, admin: bool, asked: Asked) -> Result<Session, String> {
        let name = basename(ws);
        let rq = Req {
            env: &self.env,
            admin,
            slug: &self.slug,
            workspace: &name,
            root: ws,
            query: "",
            body: &Value::Null,
            log: &self.log,
            docs: Some((&self.acct, &self.code_root)),
            thawed: None,
        };
        let mut ctx = Ctx::of(Tag::ProjectDoc, &rq);
        ctx.touches = false;
        Ok(Session::given(ctx, self.media.lites.clone(), admin, asked))
    }
}

/// The site a host call is answered from: none when `media` is off.
pub fn site_of(
    media: Option<&Arc<Media>>,
    acct: Arc<Acct>,
    code_root: &std::path::Path,
    env: &BTreeMap<String, String>,
    slug: &str,
    log: &Log,
) -> Option<Site> {
    Some(Site {
        media: media?.clone(),
        acct,
        code_root: code_root.to_path_buf(),
        env: env.clone(),
        slug: slug.to_string(),
        log: log.clone(),
    })
}

/// The account's media, for a route that warms what it opened.
pub(in crate::routes::files::service::native) fn site_for(
    who: &crate::routes::files::service::Who<'_>,
) -> Option<Site> {
    let account = &who.held.account;
    site_of(
        who.held.media.as_ref(),
        who.held.acct.clone(),
        who.code_root?,
        &account.env,
        &account.slug,
        who.log,
    )
}
