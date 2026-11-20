//! auth — what a connection proves who it is with: the secrets in the device's keychain or a
//! private file, the device's one SSH key, and the servers whose host key is trusted.
//! What a reader gets wrong: nothing here holds a secret in memory past a call, and none reaches a log or a URL.

mod hosts;
mod keys;
mod secret;
#[cfg(target_os = "ios")]
mod secret_ios;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod secret_map;

pub use hosts::{HOSTS_FILE, KnownHosts};
pub(crate) use keys::device_key;
pub use keys::{key_is_synced, public_key};
pub use secret::{
    FileSecrets, MemorySecrets, SERVICE, Secrets, StoreSpec, WithFile, account_tag, device_store,
    entry_name, is_shared, store_spec,
};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub use secret_map::{Os, means_absent};
