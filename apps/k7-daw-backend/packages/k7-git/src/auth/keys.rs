//! keys.rs — the device's one SSH key: an ed25519 key made by the app on first need and kept as a secret.
//! What a reader gets wrong: the private text goes from `generate` to the store and to the SSH client, never to a
//! caller outside the crate; `device_key` hands out the PUBLIC line, and a person never brings a key of their own.

use std::sync::Mutex;

use russh::keys::ssh_key::LineEnding;
use russh::keys::ssh_key::private::Ed25519Keypair;
use russh::keys::{PrivateKey, decode_secret_key};
use serde_json::{Value, json};

use super::Secrets;
use crate::Error;

/// What a keychain holds for the key: a mark, then the key as JSON.
const KEY_MARK: &str = "k7-ssh-key:";

/// The entry the device's key lives in: one per k7 account on the device, whatever the server.
pub(super) const KEY_HOST: &str = "device-ssh-key";
pub(super) const KEY_USER: &str = "k7";

/// Two first calls at once must not each make a key.
static MAKING: Mutex<()> = Mutex::new(());

fn secret_of(pem: &str) -> String {
    format!("{KEY_MARK}{}", json!({ "pem": pem }))
}

/// The key a kept secret holds.
fn pem_of(secret: &str) -> Result<String, Error> {
    let bad = || Error::BadKey("the stored private key cannot be read".into());
    let rest = secret.strip_prefix(KEY_MARK).ok_or_else(bad)?;
    let v: Value = serde_json::from_str(rest).map_err(|_| bad())?;
    v.get("pem")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(bad)
}

/// A new ed25519 key: the private key as OpenSSH text, and the public key as an `authorized_keys` line.
fn generate(comment: &str) -> Result<(String, String), Error> {
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).map_err(|e| Error::BadKey(format!("no randomness: {e}")))?;
    let mut key = PrivateKey::from(Ed25519Keypair::from_seed(&seed));
    key.set_comment(comment);
    let bad = |e: russh::keys::ssh_key::Error| Error::BadKey(e.to_string());
    let private = key.to_openssh(LineEnding::LF).map_err(bad)?;
    let public = key.public_key().to_openssh().map_err(bad)?;
    Ok((private.to_string(), public))
}

fn io(e: std::io::Error) -> Error {
    Error::Io(format!("the device's keychain: {e}"))
}

/// The device's key, made and stored when there is none yet.
pub(crate) fn device_key(secrets: &dyn Secrets) -> Result<PrivateKey, Error> {
    let _one = MAKING.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(kept) = secrets.get(KEY_HOST, KEY_USER).map_err(io)? {
        return decode_secret_key(&pem_of(&kept)?, None).map_err(|e| Error::BadKey(e.to_string()));
    }
    let (pem, _) = generate("k7")?;
    secrets
        .set(KEY_HOST, KEY_USER, &secret_of(&pem))
        .map_err(io)?;
    decode_secret_key(&pem, None).map_err(|e| Error::BadKey(e.to_string()))
}

/// Whether the stored key travels to the person's other Apple devices.
pub fn key_is_synced(secrets: &dyn Secrets) -> bool {
    secrets.is_synced(KEY_HOST, KEY_USER)
}

/// The device's public key as an `authorized_keys` line, made on first use; the private half never leaves.
pub fn public_key(secrets: &dyn Secrets) -> Result<String, Error> {
    device_key(secrets)?
        .public_key()
        .to_openssh()
        .map_err(|e| Error::BadKey(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::MemorySecrets;

    #[test]
    fn a_generated_key_is_an_ed25519_the_client_can_read_and_its_public_line_is_the_same_key() {
        let (private, public) = generate("k7").unwrap();
        let k = decode_secret_key(&private, None).unwrap();
        assert!(public.starts_with("ssh-ed25519 "), "{public}");
        assert_eq!(k.public_key().to_openssh().unwrap(), public);
        assert!(!public.contains("PRIVATE"));
        assert_ne!(generate("k7").unwrap().0, private, "two keys must differ");
    }

    #[test]
    fn a_kept_secret_reads_back_as_the_key_and_a_password_is_no_key() {
        let (private, _) = generate("k7").unwrap();
        assert_eq!(pem_of(&secret_of(&private)).unwrap(), private);
        assert_eq!(pem_of("hunter2").unwrap_err().code(), "bad-key");
    }

    #[test]
    fn the_device_key_is_made_once_and_stable_and_its_public_line_holds_no_private_text() {
        let store = MemorySecrets::default();
        let first = public_key(&store).unwrap();
        assert!(first.starts_with("ssh-ed25519 "), "{first}");
        assert!(!first.contains("PRIVATE"));
        assert_eq!(public_key(&store).unwrap(), first);
        assert_ne!(public_key(&MemorySecrets::default()).unwrap(), first);
    }
}
