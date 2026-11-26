//! trust — the certificates the supervisor trusts, and the one TLS configuration every socket of it dials with.
//! Not a preference: a studio added from the app is routinely behind a local proxy with a private CA that
//! exists in the OS store and nowhere else, and without it every heartbeat fails its handshake silently.
//! What a reader gets wrong: iOS and Android never read the OS store here (the bundle the app ships, the
//! conscrypt folders); the Windows "Disallowed" store and server-auth filter are M8's.

use std::sync::{Arc, Mutex};

use rustls::RootCertStore;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::pem::PemObject;

/// The roots as parsed, and the configuration built over them, until `forget`.
static CACHE: Mutex<Option<(Arc<rustls::ClientConfig>, usize)>> = Mutex::new(None);

/// Every certificate of a PEM text, a broken block skipped.
pub fn pem_certs(text: &[u8]) -> Vec<CertificateDer<'static>> {
    CertificateDer::pem_slice_iter(text)
        .filter_map(Result::ok)
        .collect()
}

#[cfg(target_os = "ios")]
fn platform_roots() -> Vec<CertificateDer<'static>> {
    let bundle = std::env::var("STUDIO_SYSTEM_CA_CERTS").unwrap_or_default();
    std::fs::read(bundle)
        .map(|t| pem_certs(&t))
        .unwrap_or_default()
}

#[cfg(target_os = "android")]
fn platform_roots() -> Vec<CertificateDer<'static>> {
    // Android 14+ updates its CAs through the conscrypt module, so that set wins when it exists.
    for dir in [
        "/apex/com.android.conscrypt/cacerts",
        "/system/etc/security/cacerts",
    ] {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            if let Ok(text) = std::fs::read(entry.path()) {
                out.extend(pem_certs(&text));
            }
        }
        if !out.is_empty() {
            return out;
        }
    }
    Vec::new()
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
fn platform_roots() -> Vec<CertificateDer<'static>> {
    // The keychain with its trust settings on macOS, the ROOT stores on Windows, the distribution's bundle on Linux.
    let loaded = rustls_native_certs::load_native_certs();
    for e in &loaded.errors {
        crate::log::host(&format!("trust roots: {e}"));
    }
    loaded.certs
}

/// The roots, and the PEM file `$STUDIO_EXTRA_CA_CERTS` names appended to them.
fn read_roots() -> Vec<CertificateDer<'static>> {
    let mut certs = platform_roots();
    if let Some(file) = std::env::var("STUDIO_EXTRA_CA_CERTS")
        .ok()
        .filter(|f| !f.is_empty())
        && let Ok(text) = std::fs::read(file)
    {
        certs.extend(pem_certs(&text));
    }
    certs
}

/// TLS with ring over `roots` (and Mozilla's when the platform gave none), the peer always verified.
pub fn config_over(roots: Vec<CertificateDer<'static>>) -> (rustls::ClientConfig, usize) {
    let mut store = RootCertStore::empty();
    let (added, _) = store.add_parsable_certificates(roots);
    let count = if added == 0 {
        store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        store.roots.len()
    } else {
        added
    };
    let cfg = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .expect("ring supports the default protocol versions")
    .with_root_certificates(store)
    .with_no_client_auth();
    (cfg, count)
}

fn cached() -> (Arc<rustls::ClientConfig>, usize) {
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let (cfg, n) = guard.get_or_insert_with(|| {
        let (cfg, n) = config_over(read_roots());
        (Arc::new(cfg), n)
    });
    (cfg.clone(), *n)
}

/// What every TLS socket of the program dials with, read once.
pub fn client_config() -> Arc<rustls::ClientConfig> {
    cached().0
}

/// How many roots the configuration trusts, for the one log line that says TLS is set up.
pub fn root_count() -> usize {
    cached().1
}

/// Read the roots again at the next use: an embedded supervisor restarted in one process.
pub fn forget() {
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = None;
}
