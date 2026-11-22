//! synced.rs — an Apple keychain as two stores: the iCloud-synchronized one for the entries that travel, the device's
//! own for the rest (`docs/git.md`, "Credentials"). What a reader gets wrong: an entry that travels and is found only in
//! the device's store was kept by an earlier k7; the first read moves it to the synced store and deletes the old one,
//! except the SSH key where `keeps_old` is set: the device's own key is then kept, local, as the old key.

use std::io;

use super::Secrets;
use super::keys::{KEY_HOST, OLD_KEY_HOST};

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
    /// The Mac's rule: the device's own SSH key, replaced by the shared one, stays in `local` as the old key.
    pub keeps_old: bool,
}

impl<S: Secrets, L: Secrets> Synced<S, L> {
    /// The device's own entry once the synced one rules: kept as the old key when it is another key, then dropped.
    fn retire(&self, host: &str, user: &str, shared: &str) {
        if !self.keeps_old || host != KEY_HOST {
            return;
        }
        let Ok(Some(own)) = self.local.get(host, user) else {
            return;
        };
        if own == shared || self.local.set(OLD_KEY_HOST, user, &own).is_ok() {
            let _ = self.local.delete(host, user);
        }
    }
}

impl<S: Secrets, L: Secrets> Secrets for Synced<S, L> {
    fn get(&self, host: &str, user: &str) -> io::Result<Option<String>> {
        if !(self.travels)(host) {
            return self.local.get(host, user);
        }
        if let Some(p) = self.synced.get(host, user)? {
            self.retire(host, user, &p);
            return Ok(Some(p));
        }
        let Some(p) = self.local.get(host, user)? else {
            return Ok(None);
        };
        // Another device's entry may have arrived since the first read: the synced store wins whenever it holds one.
        if let Some(theirs) = self.synced.get(host, user)? {
            self.retire(host, user, &theirs);
            return Ok(Some(theirs));
        }
        self.synced.set(host, user, &p)?;
        if self.keeps_old && host == KEY_HOST {
            // The promoted key is kept too: should another device's write win in iCloud, this Mac still has its own.
            self.local.set(OLD_KEY_HOST, user, &p)?;
        }
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
            keeps_old: false,
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

    /// The Mac's two stores: the SSH key travels, and the Mac's own key is kept as the old one.
    fn mac() -> (
        MemorySecrets,
        MemorySecrets,
        Synced<MemorySecrets, MemorySecrets>,
    ) {
        let (synced, local) = (MemorySecrets::default(), MemorySecrets::default());
        let s = Synced {
            synced: synced.clone(),
            local: local.clone(),
            travels: crate::auth::is_shared,
            keeps_old: true,
        };
        (synced, local, s)
    }

    #[test]
    fn a_mac_adopts_the_shared_key_an_iphone_made_and_keeps_its_own_as_the_old_key() {
        let (synced, local, s) = mac();
        let phone = crate::auth::public_key(&synced).unwrap();
        let own = crate::auth::public_key(&local).unwrap();
        assert_ne!(phone, own);
        assert_eq!(crate::auth::public_key(&s).unwrap(), phone);
        assert!(crate::auth::key_is_synced(&s));
        assert_eq!(local.get_for(KEY_HOST, "k7"), None);
        assert_eq!(
            crate::auth::old_public_key(&s).unwrap().as_deref(),
            Some(own.as_str())
        );
    }

    #[test]
    fn a_mac_promotes_its_key_when_the_store_is_empty_and_the_copy_it_keeps_is_no_old_key() {
        let (synced, local, s) = mac();
        let own = crate::auth::public_key(&local).unwrap();
        assert_eq!(crate::auth::public_key(&s).unwrap(), own);
        assert_eq!(crate::auth::public_key(&synced).unwrap(), own);
        assert!(local.get_for(OLD_KEY_HOST, "k7").is_some(), "kept");
        assert_eq!(crate::auth::old_public_key(&s).unwrap(), None);
        // Another device's write wins in iCloud: the kept copy is then the old key.
        let other = MemorySecrets::default();
        let theirs = crate::auth::public_key(&other).unwrap();
        synced
            .set(KEY_HOST, "k7", &other.get_for(KEY_HOST, "k7").unwrap())
            .unwrap();
        assert_eq!(crate::auth::public_key(&s).unwrap(), theirs);
        assert_eq!(
            crate::auth::old_public_key(&s).unwrap().as_deref(),
            Some(own.as_str())
        );
    }

    #[test]
    fn forgetting_the_old_key_deletes_it_alone() {
        let (synced, local, s) = mac();
        let shared = crate::auth::public_key(&synced).unwrap();
        crate::auth::public_key(&local).unwrap();
        crate::auth::public_key(&s).unwrap();
        assert!(crate::auth::forget_old_key(&s).unwrap());
        assert_eq!(crate::auth::old_public_key(&s).unwrap(), None);
        assert_eq!(crate::auth::public_key(&s).unwrap(), shared);
        assert!(!crate::auth::forget_old_key(&s).unwrap());
    }

    #[test]
    fn an_iphone_keeps_no_old_key() {
        let (synced, local, _) = pair();
        let s = Synced {
            synced: synced.clone(),
            local: local.clone(),
            travels: crate::auth::is_shared,
            keeps_old: false,
        };
        crate::auth::public_key(&synced).unwrap();
        crate::auth::public_key(&local).unwrap();
        crate::auth::public_key(&s).unwrap();
        assert_eq!(local.get_for(OLD_KEY_HOST, "k7"), None);
        assert!(local.get_for(KEY_HOST, "k7").is_some(), "left as it was");
    }
}
