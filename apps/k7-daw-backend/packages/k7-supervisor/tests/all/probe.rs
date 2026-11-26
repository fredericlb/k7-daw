//! probe.rs — `k7-supervisor --probe <address>` run as a program against a registry on 127.0.0.1: it prints
//! the registry it asked and what it found, exits 0 for a registry, 1 for anything else and 2 for an address
//! that is no web address, and never touches a state directory.

use std::process::{Command, Output};

use crate::server::{Reply, serve};

async fn probe(server_port: u16, address: &str) -> Output {
    let address = address.to_string();
    // A loopback address means k7's own port, which `$STUDIO_PORTS` moves onto the test server.
    tokio::task::spawn_blocking(move || {
        Command::new(env!("CARGO_BIN_EXE_k7-supervisor"))
            .args(["--probe", &address])
            .env("STUDIO_PORTS", format!("k7={server_port}"))
            .env("STUDIO_HOME", "/nonexistent/k7-probe-must-not-touch")
            .output()
            .unwrap()
    })
    .await
    .unwrap()
}

fn said(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

#[tokio::test]
async fn a_registry_is_named_and_exits_zero() {
    let server = serve(|_| Reply::Status(200, r#"{"service":"k7-registry"}"#.into())).await;
    let out = probe(server.port, "http://127.0.0.1:5223/some/page").await;
    assert_eq!(
        said(&out),
        format!("http://127.0.0.1:{}: an k7 registry", server.port)
    );
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(server.to("/api/registry/hello").len(), 1);
    assert!(!std::path::Path::new("/nonexistent/k7-probe-must-not-touch").exists());
}

#[tokio::test]
async fn anything_else_says_why_and_exits_one() {
    let server = serve(|_| Reply::Status(404, String::new())).await;
    let out = probe(server.port, "http://localhost:5223").await;
    assert_eq!(
        said(&out),
        format!(
            "http://localhost:{}: http://localhost:{} is not an k7 instance",
            server.port, server.port
        )
    );
    assert_eq!(out.status.code(), Some(1));
}

#[tokio::test]
async fn an_address_that_is_none_exits_two() {
    let out = probe(8789, "https://bad host").await;
    assert_eq!(said(&out), "https://bad host is not a web address");
    assert_eq!(out.status.code(), Some(2));
    let out = tokio::task::spawn_blocking(|| {
        Command::new(env!("CARGO_BIN_EXE_k7-supervisor"))
            .args(["--probe"])
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("--probe <address>"));
}
