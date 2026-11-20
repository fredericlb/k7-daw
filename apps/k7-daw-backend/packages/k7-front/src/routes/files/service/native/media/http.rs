//! http.rs — the one outbound client of the media pieces: `hyper` over rustls with the front's roots (`crate::trust`).
//! What a reader gets wrong: it does not decompress and follows no redirect; each caller says what it accepts and where it goes.

use std::io;
use std::sync::OnceLock;
use std::time::Duration;

use http_body_util::combinators::BoxBody;
use hyper::body::Bytes;
use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;

pub type Body = BoxBody<Bytes, io::Error>;
pub type Https = Client<HttpsConnector<HttpConnector>, Body>;

/// The shared client: http and https, http/1.1, idle connections let go quickly.
pub fn client() -> &'static Https {
    static CLIENT: OnceLock<Https> = OnceLock::new();
    CLIENT.get_or_init(|| {
        let connector = hyper_rustls::HttpsConnectorBuilder::new()
            .with_tls_config((*crate::trust::client_config()).clone())
            .https_or_http()
            .enable_http1()
            .build();
        Client::builder(TokioExecutor::new())
            .pool_idle_timeout(Duration::from_secs(4))
            .build(connector)
    })
}
