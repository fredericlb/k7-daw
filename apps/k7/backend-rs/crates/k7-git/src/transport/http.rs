//! http.rs — the hyper client the LFS transfers use, and the check of an address a server names for one.
//! What a reader gets wrong: git itself never travels over http (a remote is ssh only); this is the LFS leg an SSH
//! remote's `git-lfs-authenticate` points at.

use std::io;

use http_body_util::BodyExt;
use hyper::body::Bytes;
use hyper_rustls::HttpsConnectorBuilder;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use url::Url;

use crate::Error;

/// Whether a plain `http://` address is this machine's: the tests' server, never a network.
fn loopback(url: &Url) -> bool {
    match url.host() {
        Some(url::Host::Domain(d)) => d.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

/// An address a server may name for a large-file transfer: `https`, or `http` on loopback, with no password in it.
pub(crate) fn check(url: &Url) -> Result<(), Error> {
    if url.password().is_some() {
        return Err(Error::BadUrl(
            "an address with a password in it is refused".into(),
        ));
    }
    match url.scheme() {
        "https" => Ok(()),
        "http" if loopback(url) => Ok(()),
        "http" => Err(Error::InsecureUrl),
        s => Err(Error::BadUrl(format!("{s}:// is not an http address"))),
    }
}

/// What a request carries: bytes held in memory, or a file read as the connection takes it.
pub(crate) type Body = http_body_util::combinators::BoxBody<Bytes, io::Error>;

/// A body held in memory.
pub(crate) fn full(b: Bytes) -> Body {
    http_body_util::Full::new(b)
        .map_err(|never| match never {})
        .boxed()
}

pub(crate) type Http =
    Client<hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>, Body>;

pub(crate) fn client(tls: &rustls::ClientConfig) -> Http {
    let mut cfg = tls.clone();
    cfg.alpn_protocols.clear();
    let connector = HttpsConnectorBuilder::new()
        .with_tls_config(cfg)
        .https_or_http()
        .enable_http1()
        .build();
    Client::builder(TokioExecutor::new()).build(connector)
}
