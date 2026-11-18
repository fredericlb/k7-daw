//! The proxy against a bare TCP backend: what reaches it, and an upgraded socket spliced both ways.

use std::net::SocketAddr;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Reads one request head off `s`.
async fn read_head(s: &mut TcpStream) -> String {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    while !buf.ends_with(b"\r\n\r\n") {
        if s.read(&mut byte).await.unwrap() == 0 {
            break;
        }
        buf.push(byte[0]);
    }
    String::from_utf8(buf).unwrap()
}

/// A backend that answers every request with its own head as the body, or 101 then an echo when asked to upgrade.
async fn backend() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (mut s, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let head = read_head(&mut s).await;
                if head.to_ascii_lowercase().contains("upgrade: echo") {
                    s.write_all(b"HTTP/1.1 101 Switching Protocols\r\nConnection: upgrade\r\nUpgrade: echo\r\n\r\n").await.unwrap();
                    let mut buf = [0u8; 64];
                    while let Ok(n) = s.read(&mut buf).await {
                        if n == 0 || s.write_all(&buf[..n]).await.is_err() {
                            break;
                        }
                    }
                } else {
                    let reply = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{head}",
                        head.len()
                    );
                    s.write_all(reply.as_bytes()).await.unwrap();
                }
            });
        }
    });
    addr
}

async fn front(upstream: SocketAddr) -> SocketAddr {
    let config = k7_front::Config {
        listen: "127.0.0.1:0".parse().unwrap(),
        upstream: upstream.to_string(),
        secret: "front-secret".into(),
        trust_hops: 0,
        env: Default::default(),
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(k7_front::serve(config, std::future::pending(), move |a| {
        tx.send(a).unwrap()
    }));
    rx.await.unwrap()
}

#[tokio::test]
async fn stamps_the_peer_and_drops_what_a_client_claims() {
    let addr = front(backend().await).await;
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(b"GET /api/x?y=1 HTTP/1.1\r\nHost: app.example\r\nX-K7-Peer: 10.9.9.9\r\nX-K7-Front: guess\r\nX-Forwarded-For: 10.9.9.9\r\nConnection: close\r\n\r\n").await.unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).await.unwrap();
    let seen = out.to_ascii_lowercase();
    assert!(seen.starts_with("http/1.1 200"), "{out}");
    assert!(seen.contains("get /api/x?y=1 http/1.1"), "{out}");
    assert!(seen.contains("host: app.example"), "{out}");
    assert!(seen.contains("x-k7-peer: 127.0.0.1"), "{out}");
    assert!(seen.contains("x-k7-front: front-secret"), "{out}");
    assert!(seen.contains("x-forwarded-for: 127.0.0.1"), "{out}");
    assert!(!seen.contains("10.9.9.9"), "{out}");
}

#[tokio::test]
async fn owns_its_health_route() {
    let addr = front(backend().await).await;
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(b"GET /api/front/health HTTP/1.1\r\nHost: h\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).await.unwrap();
    assert!(out.contains("\"ok\":true"), "{out}");
}

#[tokio::test]
async fn splices_an_upgraded_socket() {
    let addr = front(backend().await).await;
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(b"GET /ws HTTP/1.1\r\nHost: h\r\nConnection: Upgrade\r\nUpgrade: echo\r\n\r\n")
        .await
        .unwrap();
    let head = read_head(&mut s).await;
    assert!(head.starts_with("HTTP/1.1 101"), "{head}");
    s.write_all(b"ping").await.unwrap();
    let mut buf = [0u8; 4];
    s.read_exact(&mut buf).await.unwrap();
    assert_eq!(&buf, b"ping");
}

#[tokio::test]
async fn says_502_when_the_backend_is_gone() {
    let gone = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap();
    let addr = front(gone).await;
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(b"GET / HTTP/1.1\r\nHost: h\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).await.unwrap();
    assert!(out.starts_with("HTTP/1.1 502"), "{out}");
}
