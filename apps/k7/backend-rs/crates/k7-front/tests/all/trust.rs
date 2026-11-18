//! A server under a private CA is refused with Mozilla's roots alone, and reached once the host names that CA's PEM
//! (`K7_EXTRA_CA_CERTS`, else `NODE_EXTRA_CA_CERTS`): the roots every outbound call of the front is built with.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;

use http_body_util::Empty;
use hyper::body::Bytes;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/private-ca")
        .join(name)
}

/// A one-thread HTTPS server on the loopback, answering every connection `ok`, under the private CA's certificate.
fn serve() -> u16 {
    let certs = CertificateDer::pem_file_iter(fixture("server.pem"))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let key = PrivateKeyDer::from_pem_file(fixture("server.key")).unwrap();
    let cfg = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(certs, key)
    .unwrap();
    let cfg = Arc::new(cfg);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for tcp in listener.incoming().flatten() {
            let cfg = cfg.clone();
            std::thread::spawn(move || {
                let Ok(conn) = rustls::ServerConnection::new(cfg) else {
                    return;
                };
                let mut tls = rustls::StreamOwned::new(conn, tcp);
                let mut buf = [0u8; 4096];
                let _ = tls.read(&mut buf);
                let _ = tls.write_all(
                    b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok",
                );
                let _ = tls.flush();
            });
        }
    });
    port
}

async fn get(port: u16) -> Result<u16, String> {
    let connector = hyper_rustls::HttpsConnectorBuilder::new()
        .with_tls_config((*k7_front::trust::client_config()).clone())
        .https_only()
        .enable_http1()
        .build();
    let client: Client<_, Empty<Bytes>> = Client::builder(TokioExecutor::new()).build(connector);
    let uri: hyper::Uri = format!("https://127.0.0.1:{port}/").parse().unwrap();
    let r = tokio::time::timeout(std::time::Duration::from_secs(10), client.get(uri))
        .await
        .map_err(|_| "timed out".to_string())?;
    r.map(|r| r.status().as_u16()).map_err(|e| format!("{e:?}"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_private_ca_is_trusted_once_the_host_names_it() {
    let port = serve();
    let refused = get(port).await;
    assert!(
        refused.is_err(),
        "Mozilla's roots alone must refuse a private CA: {refused:?}"
    );

    let ca = fixture("ca.pem").display().to_string();
    assert_eq!(
        k7_front::trust::configure(None, Some(&ca)),
        Ok(1),
        "NODE_EXTRA_CA_CERTS is read when K7's is unset"
    );
    assert_eq!(get(port).await, Ok(200));

    let missing = fixture("none.pem").display().to_string();
    assert!(
        k7_front::trust::configure(Some(&missing), Some(&ca)).is_err(),
        "K7_EXTRA_CA_CERTS wins, and a missing file is said"
    );
    assert!(
        k7_front::trust::read(&fixture("server.key")).is_err(),
        "a PEM with no certificate is said"
    );
}
