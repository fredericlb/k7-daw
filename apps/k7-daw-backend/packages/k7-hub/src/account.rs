//! account.rs — what the hub knows of an account whether or not anything runs for it: its slug and home, the
//! environment its runtime is given, a secret, and the log line's shape. What a reader gets wrong: nothing starts here.

use std::collections::BTreeMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

/// Where a hub's lines go.
pub type Log = Arc<dyn Fn(&str) + Send + Sync>;

/// A boxed future, for the callbacks the hub gives.
pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// The milliseconds since the epoch.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// The folder of the device's one account under the files root: the key `dev@example.com` has always had, kept so no device's files move.
pub const LOCAL_ACCOUNT_KEY: &str = "dev_example_com-eb2b6c0d06";

/// An account's slug and home from its user.
pub type NameOf = Arc<dyn Fn(&str) -> (String, PathBuf) + Send + Sync>;
/// What the host adds to one account's environment, by slug and user.
pub type EnvOf = Arc<dyn Fn(String, String) -> BoxFuture<BTreeMap<String, String>> + Send + Sync>;

/// An account as its runtime is given it: who, where, its secret and environment.
#[derive(Clone, Debug)]
pub struct Account {
    pub user: String,
    pub slug: String,
    pub home: PathBuf,
    /// What the hub knows the account's runtime by: its `STUDIO_HUB_SECRET`.
    pub secret: String,
    pub env: BTreeMap<String, String>,
}

/// 32 random bytes in hex: an account's secret.
pub fn random_secret() -> Result<String, String> {
    let mut raw = [0u8; 32];
    getrandom::fill(&mut raw).map_err(|e| e.to_string())?;
    Ok(crate::hex(&raw))
}

/// How accounts are named and what the host adds to each one's environment.
pub struct Names {
    pub name_of: NameOf,
    pub base_env: BTreeMap<String, String>,
    pub env: EnvOf,
}

impl Names {
    pub fn new(name_of: NameOf) -> Names {
        Names {
            name_of,
            base_env: BTreeMap::new(),
            env: Arc::new(|_, _| Box::pin(async { BTreeMap::new() })),
        }
    }

    /// The account's slug and home, whether or not it has a runtime.
    pub fn name_of(&self, user: &str) -> (String, PathBuf) {
        (self.name_of)(user)
    }

    /// What the host adds to an account's environment beyond the allowed base: its hub, sync and token variables.
    pub async fn overlay(&self, slug: &str, user: &str) -> BTreeMap<String, String> {
        (self.env)(slug.to_string(), user.to_string()).await
    }

    /// The environment a runtime of `user` holding `secret` is given, its home made.
    pub async fn environment(&self, user: &str, secret: &str) -> Result<Account, String> {
        let (slug, home) = (self.name_of)(user);
        std::fs::create_dir_all(&home)
            .map_err(|e| format!("cannot create {}: {e}", home.display()))?;
        let mut env = self.base_env.clone();
        env.insert("STUDIO_HOME".into(), home.display().to_string());
        env.insert("STUDIO_HUB_SECRET".into(), secret.to_string());
        env.extend(self.overlay(&slug, user).await);
        Ok(Account {
            user: user.to_string(),
            slug,
            home,
            secret: secret.to_string(),
            env,
        })
    }
}

#[cfg(test)]
mod key_tests {
    use super::LOCAL_ACCOUNT_KEY;

    /// The key as every release has derived it: the email's non-alphanumerics as `_`, then the first 10 hex of its SHA-256.
    #[test]
    fn the_accounts_folder_is_the_one_every_device_holds() {
        use sha2::{Digest, Sha256};
        let email = "dev@example.com";
        let slug: String = email
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        let hash = Sha256::digest(email.as_bytes());
        let hex: String = hash.iter().take(5).map(|b| format!("{b:02x}")).collect();
        assert_eq!(LOCAL_ACCOUNT_KEY, format!("{slug}-{hex}"));
        assert_eq!(LOCAL_ACCOUNT_KEY, "dev_example_com-eb2b6c0d06");
    }
}
