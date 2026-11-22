//! secret_apple.rs — a git secret in an Apple device's protected-data keychain: iOS always, a Mac app with its
//! provisioning profile (`docs/git.md`, "Credentials"). What a reader gets wrong: the store is opened per call and
//! bound to no process-wide default, and its access group is the app's default one, which the iOS and Mac apps share.

use std::collections::HashMap;
use std::io;

use keyring_core::api::CredentialStoreApi;
use keyring_core::{Entry, Error};

use super::secret::{SERVICE, Secrets, entry_name, store_spec};

/// One of the two protected stores: the iCloud-synchronized one (`cloud`), or this device's alone.
pub struct Protected {
    pub account: String,
    pub cloud: bool,
}

fn other(e: Error) -> io::Error {
    io::Error::other(e)
}

impl Protected {
    fn entry(&self, host: &str, user: &str) -> io::Result<Entry> {
        let spec = store_spec(self.cloud);
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
        let name = entry_name(&self.account, host, user);
        store
            .build(SERVICE, &name, modifiers.as_ref())
            .map_err(other)
    }
}

impl Secrets for Protected {
    fn get(&self, host: &str, user: &str) -> io::Result<Option<String>> {
        match self.entry(host, user)?.get_password() {
            Ok(p) => Ok(Some(p)),
            Err(Error::NoEntry) => Ok(None),
            Err(e) => Err(other(e)),
        }
    }

    fn set(&self, host: &str, user: &str, password: &str) -> io::Result<()> {
        self.entry(host, user)?
            .set_password(password)
            .map_err(other)
    }

    fn delete(&self, host: &str, user: &str) -> io::Result<()> {
        match self.entry(host, user)?.delete_credential() {
            Ok(()) | Err(Error::NoEntry) => Ok(()),
            Err(e) => Err(other(e)),
        }
    }
}
