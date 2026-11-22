//! git_author.rs — who a commit is by when nobody typed it (`docs/files/git.md`, "The commit author"): each field the
//! person left empty is filled from the first source that has it (a signed-in forge, the desktop's git config, the
//! system account). What a reader gets wrong: only what the person typed is ever stored; a suggestion is recomputed on
//! each read, and a forge's answer is cached in a per-device file, never in the keychain that iCloud syncs.

use std::path::Path;
use std::sync::Mutex;

use k7_git::{Error, Identity};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::git::Ctx;
use super::git_forge::{Forge, accounts, saved};
use super::git_forge_token::fresh;

const FILE: &str = ".k7-git-forge-author.json";

/// Where a suggested field came from; the page words it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Source {
    GitHub,
    Forgejo,
    GitConfig,
    System,
}

impl Source {
    pub fn code(self) -> &'static str {
        match self {
            Source::GitHub => "github",
            Source::Forgejo => "forgejo",
            Source::GitConfig => "git-config",
            Source::System => "system",
        }
    }
}

/// A name and an email, either of which may be missing.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Person {
    pub name: String,
    pub email: String,
}

/// What the desktop knows of its user; empty on a phone or a tablet, which have no such source.
#[derive(Clone, Debug, Default)]
pub struct Local {
    pub config: Person,
    pub system_name: String,
}

/// The identity as the commit uses it, and where each filled field came from (none: typed).
pub(super) struct Resolved {
    pub id: Identity,
    pub name_from: Option<Source>,
    pub email_from: Option<Source>,
}

fn clean(s: Option<&str>) -> String {
    s.map(str::trim).unwrap_or("").to_string()
}

/// A forge's `/user` answer as a person: GitHub's commit address stands in for a private email.
pub(super) fn read_user(f: &Forge, body: &Value) -> Person {
    let text = |k: &str| clean(body.get(k).and_then(Value::as_str));
    let login = text("login");
    let name = [text("name"), text("full_name"), login.clone()]
        .into_iter()
        .find(|n| !n.is_empty())
        .unwrap_or_default();
    let mut email = text("email");
    let id = body.get("id").and_then(Value::as_u64);
    if let (Forge::GitHub(_), true, Some(id), false) = (f, email.is_empty(), id, login.is_empty()) {
        email = format!("{id}+{login}@users.noreply.github.com");
    }
    Person { name, email }
}

fn lock() -> &'static Mutex<()> {
    static L: Mutex<()> = Mutex::new(());
    &L
}

fn cache_of(ctx: &Ctx) -> serde_json::Map<String, Value> {
    let path = Path::new(&ctx.account_home).join(FILE);
    std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Keeps what an account's `/user` said, beside the identity file; best effort, a suggestion is never worth an error.
pub(super) fn learn(ctx: &Ctx, f: &Forge, body: &Value) {
    let _held = lock().lock().unwrap_or_else(|p| p.into_inner());
    let mut all = cache_of(ctx);
    all.insert(
        f.profile().id.clone(),
        serde_json::json!(read_user(f, body)),
    );
    let path = Path::new(&ctx.account_home).join(FILE);
    let temp = path.with_extension("json.tmp");
    if std::fs::write(&temp, Value::Object(all).to_string()).is_ok() {
        let _ = std::fs::rename(&temp, &path);
    }
}

fn source_of(f: &Forge) -> Source {
    match f {
        Forge::GitHub(_) => Source::GitHub,
        Forge::Forgejo(_) => Source::Forgejo,
    }
}

/// The signed-in accounts' cached people, GitHub first, then the Forgejo servers in list order.
fn from_forges(ctx: &Ctx) -> Vec<(Source, Person)> {
    let cache = cache_of(ctx);
    accounts(ctx)
        .iter()
        .filter(|f| f.available() && saved(ctx, f).is_some())
        .filter_map(|f| {
            let p = cache.get(&f.profile().id)?;
            Some((source_of(f), serde_json::from_value(p.clone()).ok()?))
        })
        .collect()
}

/// The desktop's own sources: its git config and its account's full name; nothing on a phone or a tablet.
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub(super) fn desktop() -> Local {
    let c = Identity::from_global_config();
    Local {
        config: Person {
            name: c.name,
            email: c.email,
        },
        system_name: system_name(),
    }
}

#[cfg(any(target_os = "ios", target_os = "android"))]
pub(super) fn desktop() -> Local {
    Local::default()
}

/// The first comma-separated field of a GECOS entry, which is the full name.
#[cfg(target_os = "linux")]
fn full_name_of(gecos: &str) -> String {
    clean(gecos.split(',').next())
}

#[cfg(target_os = "macos")]
fn system_name() -> String {
    let out = std::process::Command::new("id").arg("-F").output();
    out.ok()
        .map(|o| clean(std::str::from_utf8(&o.stdout).ok()))
        .unwrap_or_default()
}

#[cfg(target_os = "linux")]
fn system_name() -> String {
    let user = std::env::var("USER").unwrap_or_default();
    let out = std::process::Command::new("getent")
        .args(["passwd", &user])
        .output();
    let line = out.ok().and_then(|o| String::from_utf8(o.stdout).ok());
    line.and_then(|l| l.split(':').nth(4).map(full_name_of))
        .unwrap_or_default()
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "ios",
    target_os = "android"
)))]
fn system_name() -> String {
    String::new()
}

/// The stored identity with each empty field filled from the first source that has it.
pub(super) fn resolve(ctx: &Ctx) -> Result<Resolved, Error> {
    let mut id = ctx.identity.load()?.unwrap_or_default();
    let (need_name, need_email) = (id.name.trim().is_empty(), id.email.trim().is_empty());
    if !need_name && !need_email {
        return Ok(Resolved {
            id,
            name_from: None,
            email_from: None,
        });
    }
    let local = (ctx.local)();
    let mut sources = from_forges(ctx);
    sources.push((Source::GitConfig, local.config));
    sources.push((
        Source::System,
        Person {
            name: local.system_name,
            email: String::new(),
        },
    ));
    let first = |want: bool, pick: fn(&Person) -> &String| {
        sources
            .iter()
            .filter(|_| want)
            .find(|(_, p)| !pick(p).is_empty())
            .map(|(s, p)| (*s, pick(p).clone()))
    };
    let (mut nf, mut ef) = (None, None);
    if let Some((s, v)) = first(need_name, |p| &p.name) {
        (id.name, nf) = (v, Some(s));
    }
    if let Some((s, v)) = first(need_email, |p| &p.email) {
        (id.email, ef) = (v, Some(s));
    }
    Ok(Resolved {
        id,
        name_from: nf,
        email_from: ef,
    })
}

/// Reads `/user` of a signed-in account and keeps it; a refused or offline call leaves what was cached.
pub(super) async fn refresh(ctx: &Ctx, f: &Forge) {
    let Some(s) = saved(ctx, f) else { return };
    let Ok(s) = fresh(ctx, f, s).await else {
        return;
    };
    if let Ok(r) = f.profile().api.get(&s.token, "/user").await
        && r.ok()
    {
        learn(ctx, f, &r.body);
    }
}

/// `refresh` for every signed-in account, on a task of its own so an accounts read never waits on a forge.
pub(super) fn refresh_later(ctx: &Ctx, all: &[Forge]) {
    for f in all.iter().filter(|f| f.available()) {
        let (c, f) = (ctx.clone(), f.clone());
        tokio::spawn(async move { refresh(&c, &f).await });
    }
}

/// Before a commit: an account signed in through iCloud, never read here, is read now when the author is still unknown.
pub(super) async fn ensure(ctx: &Ctx) {
    if resolve(ctx).is_ok_and(|r| r.id.is_complete()) {
        return;
    }
    let cached = cache_of(ctx);
    for f in accounts(ctx) {
        if f.available() && saved(ctx, &f).is_some() && !cached.contains_key(&f.profile().id) {
            refresh(ctx, &f).await;
        }
    }
}
