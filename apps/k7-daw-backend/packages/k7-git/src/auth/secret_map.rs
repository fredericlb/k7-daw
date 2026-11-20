//! secret_map.rs — which `keyring` errors mean "no store is available here", so the private file may take over.
//! What a reader gets wrong: a locked or refusing store is NOT absent (docs/git.md, "Credentials").

use std::io;

/// The desktop OS a store error came from; a parameter so every row is testable on one machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Mac,
    Windows,
    Linux,
}

impl Os {
    pub fn here() -> Os {
        if cfg!(target_os = "macos") {
            Os::Mac
        } else if cfg!(windows) {
            Os::Windows
        } else {
            Os::Linux
        }
    }
}

// errSecNoDefaultKeychain, errSecNotAvailable, errSecNoSuchKeychain.
const MAC_ABSENT: [i32; 3] = [-25307, -25291, -25294];
const WINDOWS_NO_LOGON_SESSION: u32 = 1312;

/// A macOS error code, read from `security_framework::Error`'s Debug (`Error { code: -25307, .. }`).
fn mac_code(e: &(dyn std::error::Error + Send + Sync)) -> Option<i32> {
    let d = format!("{e:?}");
    let n = d.split("code: ").nth(1)?;
    n.split(|c: char| c != '-' && !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()
}

/// A Windows error code, read from the store's `PlatformError(1312)` Debug.
fn windows_code(e: &(dyn std::error::Error + Send + Sync)) -> Option<u32> {
    let d = format!("{e:?}");
    d.strip_prefix("PlatformError(")?
        .strip_suffix(')')?
        .parse()
        .ok()
}

/// True when `e` says no store answers at all; false for a locked, refusing or failing one.
pub fn means_absent(e: &keyring::Error, os: Os) -> bool {
    use keyring::Error::{NoDefaultStore, NoStorageAccess, PlatformFailure};
    match (e, os) {
        (NoDefaultStore, _) => true,
        // zbus has no bus or service: the store maps every non-lock failure to PlatformFailure.
        (PlatformFailure(_), Os::Linux) => true,
        (PlatformFailure(p) | NoStorageAccess(p), Os::Mac) => {
            mac_code(p.as_ref()).is_some_and(|c| MAC_ABSENT.contains(&c))
        }
        (NoStorageAccess(p), Os::Windows) => {
            windows_code(p.as_ref()) == Some(WINDOWS_NO_LOGON_SESSION)
        }
        _ => false,
    }
}

/// `NotConnected` for an absent store, `Other` for anything else.
pub fn to_io(e: keyring::Error) -> io::Error {
    if means_absent(&e, Os::here()) {
        io::Error::new(io::ErrorKind::NotConnected, e)
    } else {
        io::Error::other(e)
    }
}
