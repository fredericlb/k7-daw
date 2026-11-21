//! git_connect.rs — `POST /git/connect`: the host key the person trusted is staged, proved by a fetch through an
//! anonymous remote, and only then does the workspace point at the remote. What a reader gets wrong: a failure puts the
//! trusted hosts back as they were, so a mistyped address leaves the old connection working.

use std::path::Path;

use k7_git::{Error, Origin, Repo};
use serde_json::{Value, json};

use super::git::{Answer, Ctx, field, open, origin_of};
use crate::routes::files::json_answer;

/// What one trusted host was before the connect wrote it.
struct Undo {
    host: String,
    port: u16,
    was: Option<String>,
}

/// The device's trusted hosts as they were: put back by `restore` when the connect fails.
#[derive(Default)]
pub struct Staged(Vec<Undo>);

impl Staged {
    pub fn restore(self, ctx: &Ctx) {
        for u in self.0.into_iter().rev() {
            let _ = match u.was {
                Some(v) => ctx.hosts.remember(&u.host, u.port, &v),
                None => ctx.hosts.forget(&u.host, u.port).map(|_| ()),
            };
        }
    }
}

/// Keeps the host key the person trusted; no secret is read from the body, the device's own key signs in.
pub fn keep_trust(ctx: &Ctx, o: &Origin, b: &Value) -> Result<Staged, Error> {
    let mut staged = Staged::default();
    let Some((host, port)) = k7_git::remote_host(&o.url) else {
        return Ok(staged);
    };
    if let Some(fp) = field(b, "hostKey") {
        let io = |e: std::io::Error| Error::Io(format!("the device's keychain: {e}"));
        let at = port.unwrap_or(22);
        let was = ctx.hosts.get(&host, at).map_err(io)?;
        ctx.hosts.remember(&host, at, fp).map_err(io)?;
        staged.0.push(Undo {
            host,
            port: at,
            was,
        });
    }
    Ok(staged)
}

pub async fn connect(ctx: &Ctx, root: &Path, b: &Value) -> Answer {
    // The demos are put back as shipped at each start: a remote there would push them into someone's repository.
    if root.file_name().and_then(|n| n.to_str()) == Some(k7_file_service::workspaces::EXAMPLES) {
        return Err(Error::BadPath("the demo workspace cannot be connected to git: copy a demo to a workspace of yours first".into()));
    }
    let origin = origin_of(b)?;
    let staged = keep_trust(ctx, &origin, b)?;
    let (net, root_p, o) = (ctx.net.clone(), root.to_path_buf(), origin.clone());
    let made = !root.join(".git").exists();
    let proved = ctx
        .net
        .blocking(move || {
            let repo = if root_p.join(".git").exists() {
                open(&root_p)?
            } else {
                Repo::init(&root_p, &root_p.join(".git"))?
            };
            repo.fetch_anonymous(&net, &o)?;
            repo.set_origin(&net, &o)?;
            Ok(repo.lfs_probe(&net, &o))
        })
        .await;
    let warning = match proved {
        Ok(w) => w,
        Err(e) => {
            staged.restore(ctx);
            if made {
                let _ = std::fs::remove_dir_all(root.join(".git"));
            }
            return Err(e);
        }
    };
    let fetch = ctx.fetchers.of(&root.display().to_string());
    fetch.lock().unwrap_or_else(|p| p.into_inner()).opened();
    if let Some(hub) = super::git_github_api::Hub::from_env() {
        let (c, url) = (ctx.clone(), origin.url.clone());
        tokio::spawn(async move { super::git_github::tag_topic(&c, &hub, &url).await });
    }
    let mut answered = json!({ "ok": true, "remote": origin.url });
    if let Some(w) = warning {
        answered["lfsWarning"] = json!(w);
    }
    Ok(json_answer(200, &answered))
}
