//! auth_secret.rs — the private file takes over from the keychain only where no keychain answers: a store that is away
//! is replaced by the file, a store that is locked or refuses is an error, never a silent file.
//! What a reader gets wrong: the stubs stand for what `Keychain` reports, since the CI machine has a real keychain or none.

use std::io;

use k7_git::auth::Secrets;
use k7_git::auth::{FileSecrets, MemorySecrets, WithFile};

mod common;
use common::Dir;

/// A keychain that fails every call with `kind`.
struct Failing(io::ErrorKind);

impl Secrets for Failing {
    fn get(&self, _: &str, _: &str) -> io::Result<Option<String>> {
        Err(io::Error::new(self.0, "stub"))
    }
    fn set(&self, _: &str, _: &str, _: &str) -> io::Result<()> {
        Err(io::Error::new(self.0, "stub"))
    }
    fn delete(&self, _: &str, _: &str) -> io::Result<()> {
        Err(io::Error::new(self.0, "stub"))
    }
}

#[test]
fn a_keychain_that_is_away_is_replaced_by_the_private_file() {
    let dir = Dir::new("auth-secret-away");
    let s = WithFile {
        keychain: Failing(io::ErrorKind::NotConnected),
        file: FileSecrets(dir.path().join("secrets")),
    };
    s.set("https://x/", "ann", "pw").unwrap();
    assert_eq!(s.get("https://x/", "ann").unwrap().as_deref(), Some("pw"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(dir.path().join("secrets"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700);
    }
    s.delete("https://x/", "ann").unwrap();
    assert_eq!(s.get("https://x/", "ann").unwrap(), None);
}

#[test]
fn a_keychain_that_refuses_is_an_error_and_writes_no_file() {
    let dir = Dir::new("auth-secret-locked");
    let s = WithFile {
        keychain: Failing(io::ErrorKind::Other),
        file: FileSecrets(dir.path().join("secrets")),
    };
    assert!(s.set("https://x/", "ann", "pw").is_err());
    assert!(s.get("https://x/", "ann").is_err());
    assert!(
        !dir.path().join("secrets").exists(),
        "a password reached the file"
    );
}

#[test]
fn a_password_kept_in_the_file_while_the_keychain_was_away_is_found_when_it_is_back() {
    let dir = Dir::new("auth-secret-back");
    let file = FileSecrets(dir.path().join("secrets"));
    file.set("https://x/", "ann", "pw").unwrap();
    let s = WithFile {
        keychain: MemorySecrets::default(),
        file,
    };
    assert_eq!(s.get("https://x/", "ann").unwrap().as_deref(), Some("pw"));
}
