//! secret.rs — where a git secret (a token, an SSH key) is kept: the device's keychain, or a private file where it has none.
//! What a reader gets wrong: a store belongs to ONE k7 account (the keychain is the device's, the entry is not), and a
//! locked keychain is an error: the file takes over only when no keychain answers.

use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use sha2::{Digest, Sha256};

/// The keychain's service name: a git secret is "k7 git", apart from any other of k7's.
pub const SERVICE: &str = "k7 git";

/// One secret per (host, user); the key is a hash, so a file's name tells nothing.
pub trait Secrets: Send + Sync {
    fn get(&self, host: &str, user: &str) -> io::Result<Option<String>>;
    fn set(&self, host: &str, user: &str, password: &str) -> io::Result<()>;
    fn delete(&self, host: &str, user: &str) -> io::Result<()>;
    /// Whether the kept secret travels to the person's other devices (iCloud Keychain); false wherever a secret is per device.
    fn is_synced(&self, _host: &str, _user: &str) -> bool {
        false
    }
}

/// How an Apple device's keychain stores one entry: synchronizable (iCloud Keychain) or this device's alone (docs/git.md).
#[derive(Debug, PartialEq, Eq)]
pub struct StoreSpec {
    pub cloud_sync: bool,
    /// None: no `ThisDeviceOnly` class, which would stop the item from syncing.
    pub access_policy: Option<&'static str>,
}

/// What an Apple device shares with the person's other ones: the SSH key and the forge tokens.
pub fn is_shared(host: &str) -> bool {
    host == super::keys::KEY_HOST || super::synced::is_token(host)
}

/// Whether this Mac app carries its iCloud entitlements, said once by the host before the front starts (docs/git.md).
static MAC_ICLOUD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The Mac app's word that it is signed with its provisioning profile: the tokens and the SSH key then sync; a no-op elsewhere.
pub fn use_icloud_keychain() {
    MAC_ICLOUD.store(true, std::sync::atomic::Ordering::Relaxed);
}

pub fn store_spec(shared: bool) -> StoreSpec {
    if shared {
        StoreSpec {
            cloud_sync: true,
            access_policy: None,
        }
    } else {
        StoreSpec {
            cloud_sync: false,
            access_policy: Some("after-first-unlock-this-device-only"),
        }
    }
}

/// The name of one account's entry for (`host`, `user`): the account's tag first, so no two accounts share an entry.
pub fn entry_name(account: &str, host: &str, user: &str) -> String {
    format!("{}/{user}@{host}", account_tag(account))
}

/// What names a k7 account in a keychain entry: a short hash, so the entry's name tells nothing and stays short on Windows.
pub fn account_tag(account: &str) -> String {
    sha256_hex(format!("k7-account:{account}").as_bytes())[..12].to_string()
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn key(host: &str, user: &str) -> String {
    sha256_hex(format!("{user}@{host}").as_bytes())
}

/// The device's keychain: Keychain, Credential Manager or the Secret Service, for one k7 account's entries.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub struct Keychain {
    pub account: String,
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl Keychain {
    fn entry(&self, host: &str, user: &str) -> io::Result<keyring::Entry> {
        keyring::Entry::new(SERVICE, &entry_name(&self.account, host, user)).map_err(unreachable)
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
use super::secret_map::to_io as unreachable;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl Secrets for Keychain {
    fn get(&self, host: &str, user: &str) -> io::Result<Option<String>> {
        match self.entry(host, user)?.get_password() {
            Ok(p) => Ok(Some(p)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(unreachable(e)),
        }
    }

    fn set(&self, host: &str, user: &str, password: &str) -> io::Result<()> {
        self.entry(host, user)?
            .set_password(password)
            .map_err(unreachable)
    }

    fn delete(&self, host: &str, user: &str) -> io::Result<()> {
        match self.entry(host, user)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(unreachable(e)),
        }
    }
}

/// A file per secret in a folder only the app reads (an account's own folder): Android's store, and the desktop's backup.
pub struct FileSecrets(pub PathBuf);

impl FileSecrets {
    fn file(&self, host: &str, user: &str) -> PathBuf {
        self.0.join(key(host, user))
    }
}

impl Secrets for FileSecrets {
    fn get(&self, host: &str, user: &str) -> io::Result<Option<String>> {
        match std::fs::read_to_string(self.file(host, user)) {
            Ok(p) => Ok(Some(p)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn set(&self, host: &str, user: &str, password: &str) -> io::Result<()> {
        std::fs::create_dir_all(&self.0)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o700))?;
        }
        k7_sync::fsx::write_atomic(&self.file(host, user), password)
    }

    fn delete(&self, host: &str, user: &str) -> io::Result<()> {
        match std::fs::remove_file(self.file(host, user)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }
}

/// A keychain with the private file behind it where none answers (no default keychain, no Secret Service).
pub struct WithFile<S> {
    pub keychain: S,
    pub file: FileSecrets,
}

fn absent(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::NotConnected
}

impl<S: Secrets> Secrets for WithFile<S> {
    fn get(&self, host: &str, user: &str) -> io::Result<Option<String>> {
        match self.keychain.get(host, user) {
            Ok(Some(p)) => Ok(Some(p)),
            // A password kept in the file while the keychain was away is still this device's.
            Ok(None) => self.file.get(host, user),
            Err(e) if absent(&e) => self.file.get(host, user),
            Err(e) => Err(e),
        }
    }

    fn set(&self, host: &str, user: &str, password: &str) -> io::Result<()> {
        match self.keychain.set(host, user, password) {
            Err(e) if absent(&e) => self.file.set(host, user, password),
            r => r,
        }
    }

    fn delete(&self, host: &str, user: &str) -> io::Result<()> {
        let file = self.file.delete(host, user);
        match self.keychain.delete(host, user) {
            Err(e) if absent(&e) => file,
            r => r.and(file),
        }
    }
}

/// The device's store for one k7 account: desktop keychain plus file, iOS keychain, or Android file (`folder` is the account's own).
pub fn device_store(folder: PathBuf, account: &str) -> std::sync::Arc<dyn Secrets> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let here = WithFile {
            keychain: Keychain {
                account: account.to_string(),
            },
            file: FileSecrets(folder),
        };
        #[cfg(target_os = "macos")]
        if MAC_ICLOUD.load(std::sync::atomic::Ordering::Relaxed) {
            // The tokens and the SSH key travel; the Mac's own key stays here as the old key (docs/git.md).
            return std::sync::Arc::new(super::synced::Synced {
                synced: super::secret_apple::Protected {
                    account: account.to_string(),
                    cloud: true,
                },
                local: here,
                travels: is_shared,
                keeps_old: true,
            });
        }
        std::sync::Arc::new(here)
    }
    #[cfg(target_os = "ios")]
    {
        let _ = folder;
        let protected = |cloud| super::secret_apple::Protected {
            account: account.to_string(),
            cloud,
        };
        std::sync::Arc::new(super::synced::Synced {
            synced: protected(true),
            local: protected(false),
            travels: is_shared,
            keeps_old: false,
        })
    }
    #[cfg(target_os = "android")]
    {
        let _ = account;
        std::sync::Arc::new(FileSecrets(folder))
    }
}

/// Memory only: the tests' store, and a stand-in where nothing may touch the real keychain.
/// `for_account` is another k7 account's view of the same device: the entries are shared, the names are not.
#[derive(Default, Clone)]
pub struct MemorySecrets {
    account: String,
    kept: Arc<Mutex<HashMap<String, String>>>,
}

impl MemorySecrets {
    /// The same device, seen by another k7 account.
    pub fn for_account(&self, account: &str) -> MemorySecrets {
        MemorySecrets {
            account: account.to_string(),
            ..self.clone()
        }
    }

    fn name(&self, host: &str, user: &str) -> String {
        entry_name(&self.account, host, user)
    }

    /// Tests only: what is kept for (`host`, `user`).
    pub fn get_for(&self, host: &str, user: &str) -> Option<String> {
        self.get(host, user).ok().flatten()
    }
}

impl Secrets for MemorySecrets {
    fn get(&self, host: &str, user: &str) -> io::Result<Option<String>> {
        let m = self.kept.lock().unwrap_or_else(|e| e.into_inner());
        Ok(m.get(&self.name(host, user)).cloned())
    }

    fn set(&self, host: &str, user: &str, password: &str) -> io::Result<()> {
        let mut m = self.kept.lock().unwrap_or_else(|e| e.into_inner());
        m.insert(self.name(host, user), password.to_string());
        Ok(())
    }

    fn delete(&self, host: &str, user: &str) -> io::Result<()> {
        let mut m = self.kept.lock().unwrap_or_else(|e| e.into_inner());
        m.remove(&self.name(host, user));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shared_entries_sync_and_have_no_this_device_only_class() {
        let spec = store_spec(is_shared(super::super::keys::KEY_HOST));
        assert!(spec.cloud_sync);
        assert_eq!(spec.access_policy, None);
        assert!(is_shared("github-oauth") && is_shared("forge:git.example"));
        let local = store_spec(is_shared("nas.example"));
        assert!(!local.cloud_sync);
        assert!(local.access_policy.unwrap().ends_with("this-device-only"));
    }

    #[test]
    fn the_shared_entry_keeps_its_service_and_account_names() {
        assert_eq!(SERVICE, "k7 git");
        assert_eq!(super::super::keys::KEY_HOST, "device-ssh-key");
        assert_eq!(super::super::keys::KEY_USER, "k7");
        assert!(entry_name("ann", "device-ssh-key", "k7").ends_with("/k7@device-ssh-key"));
    }

    #[test]
    fn an_entry_is_named_by_its_account_so_two_accounts_never_share_one() {
        let (host, user) = ("nas.example", "ann");
        assert_ne!(entry_name("ann", host, user), entry_name("bob", host, user));
        assert_eq!(entry_name("ann", host, user), entry_name("ann", host, user));
    }
}
