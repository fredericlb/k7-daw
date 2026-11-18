//! https.rs — the one outgoing HTTPS client the front's routes share (Claude's API, the git listings).
//! What a reader gets wrong: TLS trusts Mozilla's roots and `K7_EXTRA_CA_CERTS`, never the OS store (`trust.rs`).

use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;

/// TLS with ring and the front's roots; HTTP/1.1, no proxy, no redirect followed.
pub(crate) fn client<B>() -> Client<HttpsConnector<HttpConnector>, B>
where
    B: hyper::body::Body + Send,
    B::Data: Send,
{
    let connector = hyper_rustls::HttpsConnectorBuilder::new()
        .with_tls_config((*crate::trust::client_config()).clone())
        .https_or_http()
        .enable_http1()
        .build();
    Client::builder(TokioExecutor::new()).build(connector)
}
