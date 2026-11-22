//! synced.rs — an Apple keychain as two stores: the iCloud-synchronized one for the entries that travel, the device's
//! own for the rest (`docs/git.md`, "Credentials"). What a reader gets wrong: an entry that travels and is found only in
//! the device's store was kept by an earlier k7; the first read moves it to the synced store and deletes the old one.

use std::io;

use super::Secrets;

/// The forge tokens' keychain hosts: GitHub's, and every Forgejo server's `forge:<id>`.
pub const GITHUB_TOKEN_HOST: &str = "github-oauth";
pub const FORGE_TOKEN_PREFIX: &str = "forge:";

/// The keychain host of one Forgejo server's token.
pub fn forge_token_host(id: &str) -> String {
    format!("{FORGE_TOKEN_PREFIX}{id}")
}

/// Whether `host` names a forge's token: the sign-ins that travel between a person's Apple devices.
pub fn is_token(host: &str) -> bool {
    host == GITHUB_TOKEN_HOST || host.starts_with(FORGE_TOKEN_PREFIX)
}

/// The synced store for the hosts `travels` names, the device's own for every other; `local` also holds what an earlier k7 kept.
pub struct Synced<S, L> {
    pub synced: S,
    pub local: L,
    pub travels: fn(&str) -> bool,
}

impl<S: Secrets, L: Secrets> Secrets for Synced<S, L> {
    fn get(&self, host: &str, user: &str) -> io::Result<Option<String>> {
        if !(self.travels)(host) {
            return self.local.get(host, user);
        }
        if let Some(p) = self.synced.get(host, user)? {
            return Ok(Some(p));
        }
        let Some(p) = self.local.get(host, user)? else {
            return Ok(None);
        };
        self.synced.set(host, user, &p)?;
        let _ = self.local.delete(host, user);
        Ok(Some(p))
    }

    fn set(&self, host: &str, user: &str, password: &str) -> io::Result<()> {
        if (self.travels)(host) {
            self.synced.set(host, user, password)
        } else {
            self.local.set(host, user, password)
        }
    }

    fn delete(&self, host: &str, user: &str) -> io::Result<()> {
        if (self.travels)(host) {
            self.synced.delete(host, user)?;
        }
        self.local.delete(host, user)
    }

    fn is_synced(&self, host: &str, user: &str) -> bool {
        (self.travels)(host) && self.synced.get(host, user).is_ok_and(|p| p.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::MemorySecrets;

    fn pair() -> (
        MemorySecrets,
        MemorySecrets,
        Synced<MemorySecrets, MemorySecrets>,
    ) {
        let (synced, local) = (MemorySecrets::default(), MemorySecrets::default());
        let s = Synced {
            synced: synced.clone(),
            local: local.clone(),
            travels: is_token,
        };
        (synced, local, s)
    }

    #[test]
    fn the_token_hosts_travel_and_nothing_else_does() {
        assert!(is_token("github-oauth"));
        assert!(is_token(&forge_token_host("git.example:3000")));
        assert!(!is_token("device-ssh-key"));
        assert!(!is_token("nas.example"));
    }

    #[test]
    fn a_token_kept_by_an_earlier_k7_moves_to_the_synced_store_once() {
        let (synced, local, s) = pair();
        local.set("github-oauth", "k7", "old").unwrap();
        assert!(!s.is_synced("github-oauth", "k7"));
        assert_eq!(s.get("github-oauth", "k7").unwrap().as_deref(), Some("old"));
        assert_eq!(synced.get_for("github-oauth", "k7").as_deref(), Some("old"));
        assert_eq!(
            local.get_for("github-oauth", "k7"),
            None,
            "the old entry is deleted"
        );
        assert!(s.is_synced("github-oauth", "k7"));
        assert_eq!(s.get("github-oauth", "k7").unwrap().as_deref(), Some("old"));
    }

    #[test]
    fn the_synced_entry_wins_over_a_leftover_one_and_a_delete_drops_both() {
        let (synced, local, s) = pair();
        synced.set("forge:h", "k7", "new").unwrap();
        local.set("forge:h", "k7", "old").unwrap();
        assert_eq!(s.get("forge:h", "k7").unwrap().as_deref(), Some("new"));
        s.delete("forge:h", "k7").unwrap();
        assert_eq!(synced.get_for("forge:h", "k7"), None);
        assert_eq!(local.get_for("forge:h", "k7"), None);
    }

    #[test]
    fn what_does_not_travel_stays_in_the_device_s_store() {
        let (synced, local, s) = pair();
        s.set("device-ssh-key", "k7", "key").unwrap();
        s.set("forge:h", "k7", "tok").unwrap();
        assert_eq!(
            local.get_for("device-ssh-key", "k7").as_deref(),
            Some("key")
        );
        assert_eq!(synced.get_for("device-ssh-key", "k7"), None);
        assert_eq!(synced.get_for("forge:h", "k7").as_deref(), Some("tok"));
        assert!(!s.is_synced("device-ssh-key", "k7"));
    }
}
