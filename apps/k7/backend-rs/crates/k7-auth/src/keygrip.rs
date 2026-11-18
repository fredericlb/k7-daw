//! keygrip.rs — Keygrip's default signature: HMAC-SHA1, base64 with `/+` as `_-` and no `=` padding.
//! It signs `<name>=<value>`, so the cookie's name is part of what is proved.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, Mac};
use sha1::Sha1;
use subtle::ConstantTimeEq;

/// The signature Keygrip's `sign(data)` gives with the single key `secret`.
pub fn sign(data: &str, secret: &str) -> String {
    let mut mac =
        Hmac::<Sha1>::new_from_slice(secret.as_bytes()).expect("HMAC takes any key length");
    mac.update(data.as_bytes());
    URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}

/// Whether `signature` is `data`'s, compared in constant time as Keygrip's `verify` does.
pub fn verify(data: &str, signature: &str, secret: &str) -> bool {
    let expected = sign(data, secret);
    expected.len() == signature.len() && bool::from(expected.as_bytes().ct_eq(signature.as_bytes()))
}
