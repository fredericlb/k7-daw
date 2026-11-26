//! tunnel_tls.rs — the tunnel dialled over wss, which the agent builds by hand: a hub under a private CA is
//! reached once that CA is a root, and a CA nobody trusts is refused with the reason in the log.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use k7_supervisor::trust::{config_over, pem_certs};
use k7_supervisor::tunnel::{Options, TunnelAgent};
use rustls::pki_types::PrivateKeyDer;
use rustls::pki_types::pem::PemObject;

use crate::hub::until;
use crate::scratch::Scratch;

const CA: &[u8] = include_bytes!("../data/ca.pem");
const LEAF: &[u8] = include_bytes!("../data/leaf.pem");
const KEY: &[u8] = include_bytes!("../data/leaf.key");

/// A wss hub that accepts every handshake and counts them.
async fn wss_hub() -> (u16, Arc<AtomicUsize>) {
    let key = PrivateKeyDer::from_pem_slice(KEY).expect("a key");
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let cfg = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(pem_certs(LEAF), key)
        .unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(cfg));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (port, accepted) = (
        listener.local_addr().unwrap().port(),
        Arc::new(AtomicUsize::new(0)),
    );
    let count = accepted.clone();
    tokio::spawn(async move {
        while let Ok((sock, _)) = listener.accept().await {
            let (acceptor, count) = (acceptor.clone(), count.clone());
            tokio::spawn(async move {
                let Ok(tls) = acceptor.accept(sock).await else {
                    return;
                };
                if let Ok(ws) = tokio_tungstenite::accept_async(tls).await {
                    count.fetch_add(1, Ordering::SeqCst);
                    std::future::pending::<()>().await;
                    drop(ws);
                }
            });
        }
    });
    (port, accepted)
}

fn agent(
    port: u16,
    roots: Vec<rustls::pki_types::CertificateDer<'static>>,
    cache: &Scratch,
) -> (TunnelAgent, Arc<Mutex<Vec<String>>>) {
    let log = Arc::<Mutex<Vec<String>>>::default();
    let mut o = Options::new(
        &format!("https://127.0.0.1:{port}"),
        "e1",
        "tok",
        cache.path().to_path_buf(),
    );
    o.tls = Some(Arc::new(config_over(roots).0));
    let sink = log.clone();
    o.log = Arc::new(move |m| sink.lock().unwrap().push(m.to_string()));
    (TunnelAgent::start(o), log)
}

#[tokio::test]
async fn a_hub_under_a_private_ca_is_reached_once_the_ca_is_a_root() {
    let (port, accepted) = wss_hub().await;
    let cache = Scratch::new("tunnel-tls");
    let (agent, _) = agent(port, pem_certs(CA), &cache);
    until(|| agent.connected(), 5000).await;
    assert_eq!(accepted.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_ca_nobody_trusts_is_refused_and_said() {
    let (port, accepted) = wss_hub().await;
    let cache = Scratch::new("tunnel-tls");
    let (agent, log) = agent(port, Vec::new(), &cache);
    until(|| !log.lock().unwrap().is_empty(), 5000).await;
    assert!(
        log.lock().unwrap()[0].contains("UnknownIssuer"),
        "{:?}",
        log.lock().unwrap()
    );
    assert!(!agent.connected());
    assert_eq!(accepted.load(Ordering::SeqCst), 0);
}
