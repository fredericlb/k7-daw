//! secret_ios.rs — a git secret in the iOS keychain (the protected-data store), built only for iOS.
//! What a reader gets wrong: the store is opened per call and bound to no process-wide default, so nothing else reads it;
//! the device's SSH key alone is synchronizable (iCloud Keychain), every other entry stays on this device.

use std::collections::HashMap;
use std::io;

use keyring_core::api::CredentialStoreApi;
use keyring_core::{Entry, Error};

use super::secret::{SERVICE, Secrets, entry_name, is_shared, store_spec};

pub struct Keychain {
    pub account: String,
}

fn other(e: Error) -> io::Error {
    io::Error::other(e)
}

fn entry(service: &str, name: &str, shared: bool) -> io::Result<Entry> {
    let spec = store_spec(shared);
    let store = if spec.cloud_sync {
        let config = HashMap::from([("cloud-sync", "true")]);
        apple_native_keyring_store::protected::Store::new_with_configuration(&config)
    } else {
        apple_native_keyring_store::protected::Store::new()
    }
    .map_err(other)?;
    let modifiers = spec
        .access_policy
        .map(|p| HashMap::from([("access-policy", p)]));
    store
        .build(service, name, modifiers.as_ref())
        .map_err(other)
}

fn read(e: &Entry) -> io::Result<Option<String>> {
    match e.get_password() {
        Ok(p) => Ok(Some(p)),
        Err(Error::NoEntry) => Ok(None),
        Err(e) => Err(other(e)),
    }
}

fn drop_entry(e: &Entry) -> io::Result<()> {
    match e.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(e) => Err(other(e)),
    }
}

impl Keychain {
    fn name(&self, host: &str, user: &str) -> String {
        entry_name(&self.account, host, user)
    }

    fn mine(&self, host: &str, user: &str) -> io::Result<Entry> {
        entry(SERVICE, &self.name(host, user), is_shared(host))
    }

    /// The same name in the device-only store, where a key made before it was shared may still sit.
    fn local(&self, host: &str, user: &str) -> io::Result<Entry> {
        entry(SERVICE, &self.name(host, user), false)
    }
}

impl Secrets for Keychain {
    fn get(&self, host: &str, user: &str) -> io::Result<Option<String>> {
        if let Some(p) = read(&self.mine(host, user)?)? {
            return Ok(Some(p));
        }
        if !is_shared(host) {
            return Ok(None);
        }
        // A key kept on this device alone moves into the shared store the first time it is read.
        let local = self.local(host, user)?;
        let Some(p) = read(&local)? else {
            return Ok(None);
        };
        self.mine(host, user)?.set_password(&p).map_err(other)?;
        let _ = drop_entry(&local);
        Ok(Some(p))
    }

    fn set(&self, host: &str, user: &str, password: &str) -> io::Result<()> {
        self.mine(host, user)?.set_password(password).map_err(other)
    }

    fn delete(&self, host: &str, user: &str) -> io::Result<()> {
        drop_entry(&self.mine(host, user)?)?;
        if is_shared(host) {
            drop_entry(&self.local(host, user)?)?;
        }
        Ok(())
    }

    fn is_synced(&self, host: &str, user: &str) -> bool {
        is_shared(host)
            && self
                .mine(host, user)
                .and_then(|e| read(&e))
                .is_ok_and(|p| p.is_some())
    }
}
