//! git_key_retry.rs — a git call over SSH refused for the key, on a host with a signed-in forge account, uploads this
//! device's key to that account and runs once more (`docs/files/git.md`, "Choices"). What a reader gets wrong: the
//! marker "key is on the account" can be stale, so a refusal clears it before the upload.

use std::future::Future;

use k7_git::Error;

use super::git::Ctx;
use super::git_forge::{Forge, accounts, saved};
use super::git_forge_token::{ensure_key, unmark};

/// The signed-in account whose ssh host and port the remote names: none for any other address.
pub(super) fn account_for(ctx: &Ctx, url: &str) -> Option<Forge> {
    let (host, port) = k7_git::remote_host(url)?;
    let port = port.unwrap_or(22);
    accounts(ctx).into_iter().find(|f| {
        let ssh = &f.profile().ssh_host;
        let (h, p) = match ssh.rsplit_once(':') {
            Some((h, p)) => (h, p.parse().unwrap_or(22)),
            None => (ssh.as_str(), 22),
        };
        h.eq_ignore_ascii_case(&host) && p == port && saved(ctx, f).is_some()
    })
}

/// Runs `op`; refused for the key on a signed-in forge's host, uploads the key and runs it once more.
pub(super) async fn retrying<T, Fut>(
    ctx: &Ctx,
    url: &str,
    mut op: impl FnMut() -> Fut,
) -> Result<T, Error>
where
    Fut: Future<Output = Result<T, Error>>,
{
    match op().await {
        Err(Error::Auth(first)) => {
            let Some(f) = account_for(ctx, url) else {
                return Err(Error::Auth(first));
            };
            unmark(ctx, &f);
            ensure_key(ctx, &f).await;
            op().await
        }
        r => r,
    }
}
