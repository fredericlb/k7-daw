//! auth_secret_map.rs — which keyring errors mean "no store is available": only those, per OS, let the file take over.
//! What a reader gets wrong: the stand-ins print what the real stores' errors Debug-print; the OS calls cannot be faked.

#![cfg(not(any(target_os = "android", target_os = "ios")))]

use std::fmt;

use k7_git::auth::{Os, means_absent};
use keyring::Error;

/// Prints as `security_framework::Error` does.
struct Mac(i32);
impl fmt::Debug for Mac {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Error {{ code: {}, message: Some(\"x\") }}", self.0)
    }
}
impl fmt::Display for Mac {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "mac {}", self.0)
    }
}
impl std::error::Error for Mac {}

/// Prints as the Windows store's `PlatformError`.
#[derive(Debug)]
struct PlatformError(u32);
impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "win {}", self.0)
    }
}
impl std::error::Error for PlatformError {}

fn mac_failure(code: i32) -> Error {
    Error::PlatformFailure(Box::new(Mac(code)))
}

#[test]
fn a_mac_with_no_default_keychain_is_absent() {
    assert!(means_absent(&mac_failure(-25307), Os::Mac));
    assert!(means_absent(
        &Error::NoStorageAccess(Box::new(Mac(-25294))),
        Os::Mac
    ));
    assert!(means_absent(
        &Error::NoStorageAccess(Box::new(Mac(-25291))),
        Os::Mac
    ));
}

#[test]
fn a_locked_or_refusing_mac_keychain_is_an_error() {
    // errSecInteractionNotAllowed, errSecAuthFailed, errSecUserCanceled, errSecReadOnly.
    for c in [-25308, -25293, -128, -25292] {
        assert!(!means_absent(&mac_failure(c), Os::Mac), "{c}");
        assert!(
            !means_absent(&Error::NoStorageAccess(Box::new(Mac(c))), Os::Mac),
            "{c}"
        );
    }
}

#[test]
fn a_windows_with_no_logon_session_is_absent_and_other_failures_are_not() {
    assert!(means_absent(
        &Error::NoStorageAccess(Box::new(PlatformError(1312))),
        Os::Windows
    ));
    assert!(!means_absent(
        &Error::NoStorageAccess(Box::new(PlatformError(5))),
        Os::Windows
    ));
    assert!(!means_absent(
        &Error::PlatformFailure(Box::new(PlatformError(5))),
        Os::Windows
    ));
}

#[test]
fn a_linux_with_no_secret_service_is_absent_and_a_locked_one_is_not() {
    assert!(means_absent(
        &Error::PlatformFailure(Box::new(PlatformError(0))),
        Os::Linux
    ));
    assert!(!means_absent(
        &Error::NoStorageAccess(Box::new(PlatformError(0))),
        Os::Linux
    ));
}

#[test]
fn no_default_store_is_absent_everywhere_and_the_rest_never() {
    for os in [Os::Mac, Os::Windows, Os::Linux] {
        assert!(means_absent(&Error::NoDefaultStore, os));
        assert!(!means_absent(&Error::NoEntry, os));
        assert!(!means_absent(&Error::BadEncoding(vec![]), os));
        assert!(!means_absent(&Error::Invalid("a".into(), "b".into()), os));
        assert!(!means_absent(&Error::TooLong("a".into(), 1), os));
        assert!(!means_absent(&Error::NotSupportedByStore("a".into()), os));
    }
}
