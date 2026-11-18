//! Shared by the copy's and the life's tests: two workspaces on a real disk, a project with a playlist and modules.

#![allow(dead_code)]

use k7_file_service::tree::Limits;
use k7_file_service::workspaces::WsError;
use k7_file_service::workspaces::copy::{CopyAsk, CopyPlan, apply_copy, plan_copy};

use super::ws::{Acct, TURNTABLES};

/// `songs` holding `winter.k7` (a deck naming the playlist `Fred`), `albums` empty, and the library `k7-core` on disk.
pub fn fx(name: &str) -> Acct {
    let a = Acct::new(name);
    a.project("songs", "winter.k7", "project({ name: \"winter\" })\n");
    a.s.file(
        "studio/workspace/songs/winter.k7/modules/bass.k7x",
        "kind(\"sampler\")\n",
    );
    a.s.file("studio/workspace/songs/winter.k7/kick.wav.cache", "peaks");
    a.s.file("studio/workspace/songs/winter.k7/node_modules/x.js", "//");
    a.s.file("studio/workspace/songs/winter.k7/.hidden", "h");
    a.ws("albums");
    a
}

pub fn deck(a: &Acct) {
    a.surface("songs", "winter.k7", "decks", TURNTABLES);
    a.s.file(
        "studio/workspace/songs/_playlists/Fred.k7x",
        "items([{ song: \"a\" }])\n",
    );
}

/// The four names of a copy, owned, for a table.
pub type CopyAskOwned = (&'static str, &'static str, &'static str, &'static str);

pub fn ask<'a>(from: &'a str, path: &'a str, to: &'a str, name: &'a str) -> CopyAsk<'a> {
    CopyAsk {
        from,
        path,
        to,
        name,
    }
}

pub fn plan(a: &Acct, c: &CopyAsk<'_>) -> Result<CopyPlan, WsError> {
    plan_copy(&a.home(), false, c, &Limits::default())
}

/// The plan, written; the plan back.
pub fn copy(a: &Acct, c: &CopyAsk<'_>) -> CopyPlan {
    let p = plan(a, c).unwrap();
    apply_copy(&p).unwrap();
    p
}

/// The sentence a plan is refused with.
pub fn refusal(a: &Acct, c: &CopyAsk<'_>) -> String {
    match plan(a, c) {
        Err(e) => e.to_string(),
        Ok(p) => panic!("{c:?} was planned: {p:?}"),
    }
}
