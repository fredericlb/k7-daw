//! The copy gate (2223, 2224): a document waits for its files on the bound machine, stated outside every lock and only inside
//! the account's roots, and the latest document goes when the job ends. `send_doc` and a finished copy take the session's gate.
//! What a reader gets wrong: nothing stated here is read under the session's lock, which only decides on the answers.

use std::collections::HashMap;
use std::sync::Arc;

use k7_file_service::engine::sync::{Ctx, FileKey, SyncEffect, collect, within};
use serde_json::Value;

use super::queue::Session;
use super::stats::Stats;
use super::{Inner, held};

/// The mtime as the copy keys it: rounded milliseconds.
fn key_of(p: &str) -> Option<FileKey> {
    let m = std::fs::metadata(p).ok()?;
    if !m.is_file() {
        return None;
    }
    let ms = m
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64() * 1000.0);
    Some(FileKey {
        size: m.len(),
        mtime_ms: ms.round() as i64,
    })
}

/// The machine's view of the files, built in one place so `windows` is the host's own everywhere.
pub(super) fn ctx_of<'a>(
    url: Option<&'a str>,
    stat: &'a dyn Fn(&str) -> Option<FileKey>,
    roots: &'a [String],
) -> Ctx<'a> {
    Ctx {
        url,
        stat,
        roots: Some(roots),
        windows: cfg!(windows),
    }
}

impl Session {
    /// The files `doc` names, stated outside every lock and only inside the roots (2224, F4).
    pub(super) async fn stat_named(
        self: &Arc<Self>,
        inner: &Arc<Inner>,
        doc: &Value,
        extra: &[String],
    ) -> HashMap<String, Option<FileKey>> {
        let mut named: Vec<String> = extra.to_vec();
        collect(doc, &mut named, cfg!(windows));
        let roots = inner.account.roots.clone();
        tokio::task::spawn_blocking(move || {
            named
                .into_iter()
                .filter(|p| within(p, &roots, cfg!(windows)))
                .map(|p| {
                    let k = key_of(&p);
                    (p, k)
                })
                .collect()
        })
        .await
        .unwrap_or_default()
    }

    /// `namedSongFiles`: the files the project's library songs name, which the copy sends beside the document's.
    pub(super) async fn extra_files(self: &Arc<Self>) -> Vec<String> {
        let (Some(inner), Some(s)) = (self.inner(), self.served()) else {
            return vec![];
        };
        tokio::task::spawn_blocking(move || inner.host.named_files(&s.root, &s.project))
            .await
            .unwrap_or_default()
    }

    /// `sendDoc`: send `doc` once its files are on the bound machine.
    pub async fn send_doc(self: &Arc<Self>, doc: Value) {
        let _deciding = self.deciding();
        let _gate = self.gate.lock().await;
        self.gated_send(doc).await;
    }

    async fn gated_send(self: &Arc<Self>, doc: Value) {
        let Some(inner) = self.inner() else { return };
        let url = inner.jobs_url();
        let (mut extra, mut stats) = (vec![], HashMap::new());
        if url.is_some() {
            extra = self.extra_files().await;
            stats = self.stat_named(&inner, &doc, &extra).await;
        }
        let fx = {
            let mut copies = held(&inner.copies);
            let stat = |p: &str| stats.get(p).copied().flatten();
            let ctx = ctx_of(url.as_deref(), &stat, &inner.account.roots);
            copies.ask(&self.name, doc, extra, &ctx)
        };
        self.sync_effects(&inner, fx).await;
    }

    pub(super) async fn sync_effects(self: &Arc<Self>, inner: &Arc<Inner>, fx: Vec<SyncEffect>) {
        for e in fx {
            match e {
                SyncEffect::Send(doc) => self.dispatch(doc).await,
                SyncEffect::Log(l) => inner.tells.say(&l),
                SyncEffect::Copy { url, files } => self.run_copy(url, files),
            }
        }
    }

    /// One `sync` job over the bound machine's jobs path; its progress is the poll's, its end sends the latest document.
    pub(super) fn run_copy(self: &Arc<Self>, url: String, files: Vec<String>) {
        let Some(inner) = self.inner() else { return };
        Stats::bump(&inner.stats.copies);
        let me = self.clone();
        let i = inner.clone();
        inner.spawn(async move {
            let name = me.name.clone();
            let progress = i.clone();
            let reply = super::jobs::sync(&url, files, move |frame| {
                held(&progress.copies).progress(&name, frame);
            })
            .await;
            let _deciding = me.deciding();
            let _gate = me.gate.lock().await;
            let doc = held(&i.copies).run_doc(&me.name).unwrap_or(Value::Null);
            let extra = me.extra_files().await;
            let stats = me.stat_named(&i, &doc, &extra).await;
            let fx = {
                let mut copies = held(&i.copies);
                let stat = |p: &str| stats.get(p).copied().flatten();
                let jobs = i.jobs_url();
                let ctx = ctx_of(jobs.as_deref(), &stat, &i.account.roots);
                copies.finished(&me.name, reply, extra, &ctx)
            };
            me.sync_effects(&i, fx).await;
        });
    }
}
