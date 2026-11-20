//! trust.rs — the roots every outbound TLS call of the front trusts: Mozilla's, plus the PEM bundle the host names in
//! `K7_EXTRA_CA_CERTS`, so a server under the person's own CA is reached.
//! What a reader gets wrong: a client built before `configure` trusts Mozilla's alone — the front configures first.

use std::path::Path;
use std::sync::{Arc, RwLock};

use rustls::pki_types::CertificateDer;
use rustls::pki_types::pem::PemObject;

static EXTRA: RwLock<Vec<CertificateDer<'static>>> = RwLock::new(Vec::new());

/// The settings' bundle, read once: how many certificates it added, or why none was.
pub fn configure(k7: Option<&str>) -> Result<usize, String> {
    let Some(file) = k7.map(str::trim).filter(|f| !f.is_empty()) else {
        return Ok(0);
    };
    let certs = read(Path::new(file))?;
    let n = certs.len();
    *EXTRA.write().unwrap_or_else(|p| p.into_inner()) = certs;
    Ok(n)
}

/// Every certificate of a PEM file; a file with none, or a broken one, is an error naming it.
pub fn read(file: &Path) -> Result<Vec<CertificateDer<'static>>, String> {
    let certs = CertificateDer::pem_file_iter(file)
        .map_err(|e| format!("{}: {e}", file.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("{}: {e}", file.display()))?;
    if certs.is_empty() {
        return Err(format!("{}: no certificate in it", file.display()));
    }
    Ok(certs)
}

/// Mozilla's roots and the configured ones; a certificate rustls refuses as a root is left out.
pub fn roots() -> rustls::RootCertStore {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let extra = EXTRA.read().unwrap_or_else(|p| p.into_inner()).clone();
    roots.add_parsable_certificates(extra);
    roots
}

/// TLS with ring and `roots()`, as every outbound call of the front.
pub fn client_config() -> Arc<rustls::ClientConfig> {
    let cfg = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .expect("ring supports the default protocol versions")
    .with_root_certificates(roots())
    .with_no_client_auth();
    Arc::new(cfg)
}
