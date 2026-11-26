//! certs — the roots Windows trusts for TLS: the ROOT stores of the user and of the machine, less the
//! thumbprints in their `Disallowed` stores and any root whose usages exclude server authentication. The
//! selection is plain code; only the walk of the stores is Windows'. What a reader gets wrong: a root that
//! names NO usages at all is good for everything, while one that names usages without server auth is refused.

use std::collections::BTreeSet;

/// The usage identifier of server authentication (`szOID_PKIX_KP_SERVER_AUTH`).
pub const SERVER_AUTH: &str = "1.3.6.1.5.5.7.3.1";

/// What a root's extended key usage allows.
#[derive(Debug, Clone, PartialEq)]
pub enum Usage {
    /// No usage extension or property: every use.
    Unrestricted,
    /// These usages only; none at all is no use.
    Only(Vec<String>),
}

pub fn serves_tls(usage: &Usage) -> bool {
    match usage {
        Usage::Unrestricted => true,
        Usage::Only(ids) => ids.iter().any(|id| id == SERVER_AUTH),
    }
}

/// One certificate of a ROOT store: its SHA-1 thumbprint, its DER and its usages.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub thumbprint: Vec<u8>,
    pub der: Vec<u8>,
    pub usage: Usage,
}

/// The DER of every candidate not denied and fit for TLS, a thumbprint kept once.
pub fn select(candidates: Vec<Candidate>, denied: &BTreeSet<Vec<u8>>) -> Vec<Vec<u8>> {
    let mut seen: BTreeSet<Vec<u8>> = BTreeSet::new();
    let mut out = Vec::new();
    for c in candidates {
        if denied.contains(&c.thumbprint) || !serves_tls(&c.usage) {
            continue;
        }
        // A thumbprint Windows would not give cannot tell two roots apart, so it never counts as seen.
        if !c.thumbprint.is_empty() && !seen.insert(c.thumbprint.clone()) {
            continue;
        }
        out.push(c.der);
    }
    out
}

/// The roots of both ROOT stores, as DER, less what `Disallowed` and the usages refuse.
#[cfg(windows)]
pub fn roots() -> Vec<Vec<u8>> {
    let locations = [
        windows::Win32::Security::Cryptography::CERT_SYSTEM_STORE_CURRENT_USER,
        windows::Win32::Security::Cryptography::CERT_SYSTEM_STORE_LOCAL_MACHINE,
    ];
    let mut denied = BTreeSet::new();
    for at in locations {
        for c in store::read(at, windows::core::w!("Disallowed")) {
            denied.insert(c.thumbprint);
        }
    }
    let candidates = locations
        .into_iter()
        .flat_map(|at| store::read(at, windows::core::w!("ROOT")))
        .collect();
    select(candidates, &denied)
}

#[cfg(windows)]
mod store {
    use windows::Win32::Foundation::{CRYPT_E_NOT_FOUND, GetLastError};
    use windows::Win32::Security::Cryptography::{
        CERT_CONTEXT, CERT_OPEN_STORE_FLAGS, CERT_QUERY_ENCODING_TYPE, CERT_SHA1_HASH_PROP_ID,
        CERT_STORE_PROV_SYSTEM_W, CERT_STORE_READONLY_FLAG, CTL_USAGE, CertCloseStore,
        CertEnumCertificatesInStore, CertGetCertificateContextProperty, CertGetEnhancedKeyUsage,
        CertOpenStore, HCERTSTORE, X509_ASN_ENCODING,
    };
    use windows::core::PCWSTR;

    use super::{Candidate, Usage};

    fn thumbprint(ctx: *const CERT_CONTEXT) -> Vec<u8> {
        let mut hash = [0u8; 20];
        let mut len = hash.len() as u32;
        // SAFETY: `ctx` is a live context of the enumeration and `hash` is `len` bytes.
        let got = unsafe {
            CertGetCertificateContextProperty(
                ctx,
                CERT_SHA1_HASH_PROP_ID,
                Some(hash.as_mut_ptr().cast()),
                &mut len,
            )
        };
        if got.is_ok() {
            hash[..len as usize].to_vec()
        } else {
            Vec::new()
        }
    }

    fn usage_of(ctx: *const CERT_CONTEXT) -> Usage {
        let mut size = 0u32;
        // SAFETY: a sizing call with no buffer, on a live context.
        if unsafe { CertGetEnhancedKeyUsage(ctx, 0, None, &mut size) }.is_err() {
            // No extension and no property is every use; any other failure is none.
            return not_found_is_unrestricted();
        }
        let mut buf = vec![0u64; (size as usize).div_ceil(8)];
        let usage = buf.as_mut_ptr().cast::<CTL_USAGE>();
        // SAFETY: `buf` is `size` bytes, 8-aligned, which the call fills with a `CTL_USAGE` and its strings.
        if unsafe { CertGetEnhancedKeyUsage(ctx, 0, Some(usage), &mut size) }.is_err() {
            return Usage::Only(Vec::new());
        }
        // SAFETY: the call succeeded, so `usage` heads a filled `CTL_USAGE`.
        let (count, ids) = unsafe { ((*usage).cUsageIdentifier, (*usage).rgpszUsageIdentifier) };
        if count == 0 {
            return not_found_is_unrestricted();
        }
        let list = (0..count as usize)
            // SAFETY: `ids` holds `count` NUL-terminated ANSI identifiers, inside `buf`.
            .map(|i| unsafe { std::ffi::CStr::from_ptr((*ids.add(i)).0.cast()) })
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        Usage::Only(list)
    }

    /// The last error says whether the certificate had no usage at all (every use) or an empty list (none).
    fn not_found_is_unrestricted() -> Usage {
        // SAFETY: reads this thread's last error.
        if unsafe { GetLastError() }.0 == CRYPT_E_NOT_FOUND.0 as u32 {
            Usage::Unrestricted
        } else {
            Usage::Only(Vec::new())
        }
    }

    /// Every X.509 certificate of the system store `name` at `location`; none when it cannot be opened.
    pub fn read(location: u32, name: PCWSTR) -> Vec<Candidate> {
        let flags = CERT_OPEN_STORE_FLAGS(location | CERT_STORE_READONLY_FLAG.0);
        // SAFETY: `name` is a NUL-terminated wide string that outlives the call.
        let opened = unsafe {
            CertOpenStore(
                CERT_STORE_PROV_SYSTEM_W,
                CERT_QUERY_ENCODING_TYPE(0),
                None,
                flags,
                Some(name.0.cast()),
            )
        };
        let Ok(store) = opened else {
            return Vec::new();
        };
        let out = enumerate(store);
        // SAFETY: `store` was opened above and is closed once.
        let _ = unsafe { CertCloseStore(Some(store), 0) };
        out
    }

    fn enumerate(store: HCERTSTORE) -> Vec<Candidate> {
        let mut out = Vec::new();
        let mut at: *const CERT_CONTEXT = std::ptr::null();
        loop {
            // SAFETY: `at` is null or the previous context, which this call releases.
            at = unsafe { CertEnumCertificatesInStore(store, Some(at)) };
            // SAFETY: a non-null context stays valid until the next call.
            let Some(ctx) = (unsafe { at.as_ref() }) else {
                return out;
            };
            if ctx.dwCertEncodingType.0 & X509_ASN_ENCODING.0 == 0 {
                continue;
            }
            // SAFETY: `pbCertEncoded` points at `cbCertEncoded` bytes owned by the context.
            let der = unsafe {
                std::slice::from_raw_parts(ctx.pbCertEncoded, ctx.cbCertEncoded as usize)
            };
            out.push(Candidate {
                thumbprint: thumbprint(at),
                der: der.to_vec(),
                usage: usage_of(at),
            });
        }
    }
}
