//! accounts.rs — the native account: what an account holds for the front's pieces, made in `services.rs`.
//! What a reader gets wrong: one set of song sessions per account, shared by every route; never one per request.

use std::collections::BTreeMap;
use std::sync::Arc;

use k7_file_service::songs::{SongSessions, Songs};
use k7_hub::account::{Account as HubAccount, Names};
use k7_hub::env::service_env;

/// The heap the accounts' song sessions are sized for, in bytes: an embedded host's, whose whole app must fit (docs/service.md).
pub fn sessions_heap() -> usize {
    192 << 20
}

/// An account's song sessions and its environment, handed to the pieces (docs/service.md).
pub struct Account {
    songs: Songs,
    /// The environment the account's pieces read: the allow-listed base, its home, its secret and what the host adds.
    pub env: BTreeMap<String, String>,
}

impl Account {
    /// An account over `env`, its sessions sized for a runtime of `heap` bytes.
    pub fn make(env: BTreeMap<String, String>, heap: usize) -> Arc<Account> {
        Arc::new(Account {
            songs: SongSessions::new(heap),
            env,
        })
    }

    /// The account's sessions: one set for the life of the account.
    pub fn songs(&self) -> Songs {
        self.songs.clone()
    }
}

/// `base` taken as the process hands it: the variables a file service may see, from the process and the host's own.
pub fn base_env(host: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut all: BTreeMap<String, String> = std::env::vars().collect();
    all.extend(host.iter().map(|(k, v)| (k.clone(), v.clone())));
    service_env(all.iter().map(|(k, v)| (k.as_str(), v.as_str())))
}

/// The account of `user`: its home made, the allow-listed base, its secret and what the host adds for it.
pub async fn identity(
    names: &Names,
    base: &BTreeMap<String, String>,
    user: &str,
    secret: &str,
) -> Result<HubAccount, String> {
    let (slug, home) = names.name_of(user);
    std::fs::create_dir_all(&home).map_err(|e| format!("cannot create {}: {e}", home.display()))?;
    let mut env = base.clone();
    env.insert("STUDIO_HOME".into(), home.display().to_string());
    env.insert("STUDIO_HUB_SECRET".into(), secret.to_string());
    env.extend(names.overlay(&slug, user).await);
    Ok(HubAccount {
        user: user.to_string(),
        slug,
        home,
        secret: secret.to_string(),
        env,
    })
}
