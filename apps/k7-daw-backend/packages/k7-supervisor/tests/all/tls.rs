//! tls.rs — the TLS every socket dials with, against a server of our own on 127.0.0.1 under a private CA:
//! trusted when that CA is a root, refused when it is not, and refused again for a name the certificate
//! does not carry; and a platform that gave no root still trusts Mozilla's.

use std::sync::Arc;
use std::time::Duration;

use k7_supervisor::http::{Request, send_with};
use k7_supervisor::trust::{config_over, pem_certs};
use rustls::pki_types::PrivateKeyDer;
use rustls::pki_types::pem::PemObject;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const CA: &[u8] = include_bytes!("../data/ca.pem");
const LEAF: &[u8] = include_bytes!("../data/leaf.pem");
const KEY: &[u8] = include_bytes!("../data/leaf.key");

/// An https server answering every request `{"service":"k7-registry"}`, on an unused port.
async fn https_server() -> u16 {
    let chain = pem_certs(LEAF);
    let key = PrivateKeyDer::from_pem_slice(KEY).expect("a key");
    let cfg = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(chain, key)
    .unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(cfg));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((sock, _)) = listener.accept().await else {
                return;
            };
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                let Ok(mut tls) = acceptor.accept(sock).await else {
                    return;
                };
                let mut buf = [0u8; 2048];
                let _ = tls.read(&mut buf).await;
                let body = r#"{"service":"k7-registry"}"#;
                let text = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = tls.write_all(text.as_bytes()).await;
                let _ = tls.shutdown().await;
            });
        }
    });
    port
}

fn get(url: String) -> Request {
    Request {
        timeout: Duration::from_secs(3),
        ..Request::get(url)
    }
}

#[tokio::test]
async fn a_private_ca_is_trusted_once_it_is_a_root_and_not_before() {
    let port = https_server().await;
    let url = format!("https://127.0.0.1:{port}/api/registry/hello");
    let trusting = Arc::new(config_over(pem_certs(CA)).0);
    let res = send_with(trusting, get(url.clone())).await;
    assert_eq!(
        (res.status, res.body.as_str()),
        (200, r#"{"service":"k7-registry"}"#)
    );
    // Mozilla's roots alone know nobody of ours, and the reason says so.
    let stranger = Arc::new(config_over(Vec::new()).0);
    let res = send_with(stranger, get(url)).await;
    assert_eq!(res.status, 0);
    assert!(res.error.contains("UnknownIssuer"), "{}", res.error);
}

#[tokio::test]
async fn a_certificate_for_another_name_is_refused() {
    let port = https_server().await;
    let trusting = Arc::new(config_over(pem_certs(CA)).0);
    let res = send_with(trusting, get(format!("https://localhost:{port}/"))).await;
    assert_eq!(res.status, 0);
    assert!(
        res.error.contains("NotValidForName") || res.error.contains("not valid for name"),
        "{}",
        res.error
    );
}

#[test]
fn a_platform_that_gave_no_root_trusts_mozillas() {
    let (_, none) = config_over(Vec::new());
    let (_, ours) = config_over(pem_certs(CA));
    assert!(none > 100);
    assert_eq!(ours, 1);
    // A text with no certificate in it is no root, and a broken block is skipped.
    assert!(pem_certs(b"not a pem").is_empty());
    assert_eq!(
        pem_certs(
            &[
                CA,
                b"\n-----BEGIN CERTIFICATE-----\n!!\n-----END CERTIFICATE-----\n"
            ]
            .concat()
        )
        .len(),
        1
    );
}

#[test]
fn the_shared_configuration_reads_the_os_store_once() {
    let first = k7_supervisor::trust::client_config();
    assert!(Arc::ptr_eq(&first, &k7_supervisor::trust::client_config()));
    assert!(k7_supervisor::trust::root_count() > 0);
}
