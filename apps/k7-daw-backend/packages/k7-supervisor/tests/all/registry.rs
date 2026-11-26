//! registry.rs — the registry's wire against a server on 127.0.0.1 that refuses, redirects, hangs and answers:
//! the heartbeat's and the deregister's bytes, the device token only to its own studio, a refusal's reason cut
//! to a line, a hello that is not followed anywhere, and the plain-http refusal before any connection.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use k7_supervisor::identity::Identity;
use k7_supervisor::registry::{self, Credentials};
use serde_json::{Value, json};

use crate::server::{Reply, serve};

const HEARTBEAT: &str = "/api/registry/heartbeat";
const HELLO: &str = "/api/registry/hello";

fn id(secret: &str) -> Identity {
    Identity {
        id: "11111111-2222-4333-8444-555555555555".into(),
        token: "abc123".into(),
        tunnel_secret: secret.into(),
        created_at: String::new(),
    }
}

fn ok(_: &crate::server::Seen) -> Reply {
    Reply::Status(200, "{}".into())
}

#[tokio::test]
async fn a_heartbeat_is_one_post_of_sorted_keys_with_the_id() {
    let server = serve(ok).await;
    let presence = json!({"hostname": "studio", "port": 8765, "addresses": {"port": 8765, "loopback": "127.0.0.1"}});
    let beat = registry::heartbeat(
        &[server.origin()],
        &id("sss"),
        presence,
        &Credentials::default(),
    )
    .await;
    assert!(beat.ok());
    let seen = server.to(HEARTBEAT);
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].method, "POST");
    assert_eq!(seen[0].header("content-type"), Some("application/json"));
    assert_eq!(seen[0].header("authorization"), None);
    assert_eq!(
        seen[0].body,
        r#"{"addresses":{"loopback":"127.0.0.1","port":8765},"hostname":"studio","id":"11111111-2222-4333-8444-555555555555","port":8765,"token":"abc123","tunnelSecret":"sss"}"#
    );
}

#[tokio::test]
async fn a_deregister_names_the_id_and_nothing_else() {
    let server = serve(ok).await;
    assert!(
        registry::deregister(&[server.origin()], &id("sss"))
            .await
            .ok()
    );
    let seen = server.to("/api/registry/deregister");
    assert_eq!(
        seen[0].body,
        r#"{"id":"11111111-2222-4333-8444-555555555555","token":"abc123","tunnelSecret":"sss"}"#
    );
    // An identity with no secret leaves the field out.
    assert!(registry::deregister(&[server.origin()], &id("")).await.ok());
    assert_eq!(
        server.to("/api/registry/deregister")[1].body,
        r#"{"id":"11111111-2222-4333-8444-555555555555","token":"abc123"}"#
    );
}

#[tokio::test]
async fn a_device_token_goes_only_to_its_own_studio() {
    let (a, b) = (serve(ok).await, serve(ok).await);
    let cred = Credentials {
        device_tokens: BTreeMap::from([(a.origin(), "tok-a".to_string())]),
        env_token: String::new(),
    };
    registry::heartbeat(&[a.origin(), b.origin()], &id("s"), json!({}), &cred).await;
    assert_eq!(
        a.to(HEARTBEAT)[0].header("authorization"),
        Some("Bearer tok-a")
    );
    assert_eq!(b.to(HEARTBEAT)[0].header("authorization"), None);
    // The environment's token is everybody's, when the part passes it.
    let env = Credentials {
        env_token: "tok-env".into(),
        ..Credentials::default()
    };
    registry::heartbeat(&[b.origin()], &id("s"), json!({}), &env).await;
    assert_eq!(
        b.to(HEARTBEAT)[1].header("authorization"),
        Some("Bearer tok-env")
    );
}

#[tokio::test]
async fn a_refusal_is_an_outcome_with_its_reason_cut_to_a_line() {
    let long = "x".repeat(400);
    let server = serve(move |s| match s.path.as_str() {
        HEARTBEAT => Reply::Status(400, format!("field\nhostname {long}")),
        _ => Reply::Status(503, String::new()),
    })
    .await;
    let beat = registry::heartbeat(
        &[server.origin()],
        &id("s"),
        json!({}),
        &Credentials::default(),
    )
    .await;
    assert!(!beat.ok());
    let o = &beat.outcomes[0];
    assert_eq!((o.status, o.url.as_str()), (400, server.origin().as_str()));
    assert!(o.detail.starts_with("HTTP 400 field hostname xxx"));
    assert!(!o.detail.contains('\n') && o.detail.ends_with('…'));
    assert_eq!(o.detail.chars().count(), "HTTP 400 ".len() + 300 + 1);
    let gone = registry::deregister(&[server.origin()], &id("s")).await;
    assert_eq!(gone.outcomes[0].detail, "HTTP 503");
}

#[tokio::test]
async fn a_closed_port_answers_at_once_and_a_hang_within_its_bound() {
    let dead = format!("http://127.0.0.1:{}", crate::supervisor::free_port());
    let beat = registry::heartbeat(&[dead], &id("s"), json!({}), &Credentials::default()).await;
    assert_eq!(beat.outcomes[0].status, 0);
    assert!(beat.outcomes[0].detail.starts_with("no answer"));
    assert!(
        !registry::heartbeat(&[], &id("s"), json!({}), &Credentials::default())
            .await
            .ok()
    );
    let hung = serve(|_| Reply::Hang).await;
    let start = Instant::now();
    let beat = registry::heartbeat_within(
        &[hung.origin()],
        &id("s"),
        json!({}),
        &Credentials::default(),
        Duration::from_millis(300),
    )
    .await;
    assert!(start.elapsed() < Duration::from_secs(3));
    assert_eq!(beat.outcomes[0].detail, "no answer (timed out)");
    let closed = serve(|_| Reply::Close).await;
    assert!(
        !registry::heartbeat(
            &[closed.origin()],
            &id("s"),
            json!({}),
            &Credentials::default()
        )
        .await
        .ok()
    );
}

#[tokio::test]
async fn one_dead_registry_does_not_hide_a_live_one() {
    let live = serve(ok).await;
    let dead = format!("http://127.0.0.1:{}", crate::supervisor::free_port());
    let beat = registry::heartbeat(
        &[dead.clone(), live.origin()],
        &id("s"),
        json!({}),
        &Credentials::default(),
    )
    .await;
    assert_eq!(
        beat.outcomes.iter().map(|o| o.ok).collect::<Vec<_>>(),
        [false, true]
    );
    assert!(!beat.ok());
    assert_eq!(live.to(HEARTBEAT).len(), 1);
}

#[tokio::test]
async fn plain_http_past_loopback_never_leaves_the_machine() {
    let beat = registry::heartbeat(
        &["http://192.0.2.1:8789".into()],
        &id("s"),
        json!({}),
        &Credentials::default(),
    )
    .await;
    assert!(
        beat.outcomes[0]
            .detail
            .contains("plain http and not loopback")
    );
    assert_eq!(beat.outcomes[0].status, 0);
    let p = registry::probe("http://192.0.2.1:8789").await;
    assert!(!p.ok && p.why.contains("would cross in clear"));
}

#[tokio::test]
async fn a_hello_proves_a_registry_and_a_redirect_is_not_followed() {
    let good = serve(|_| Reply::Status(200, r#"{"service":"k7-registry"}"#.into())).await;
    assert_eq!(
        registry::probe(&good.origin()).await,
        registry::Probe {
            ok: true,
            why: String::new()
        }
    );
    assert_eq!(good.to(HELLO).len(), 1);
    let other = serve(|_| Reply::Status(200, r#"{"service":"something-else"}"#.into())).await;
    let p = registry::probe(&other.origin()).await;
    assert_eq!(p.why, format!("{} is not an k7 instance", other.origin()));
    let page = serve(|_| Reply::Status(200, "<html>".into())).await;
    assert!(!registry::probe(&page.origin()).await.ok);
    let target = serve(ok).await;
    let to = format!("{}{HELLO}", target.origin());
    let redirecting = serve(move |_| Reply::Redirect(to.clone())).await;
    let p = registry::probe(&redirecting.origin()).await;
    assert_eq!(
        p.why,
        format!("{} did not answer (it redirects)", redirecting.origin())
    );
    assert!(target.seen.lock().unwrap().is_empty());
    let hung = serve(|_| Reply::Hang).await;
    let p = registry::probe_within(&hung.origin(), Duration::from_millis(300)).await;
    assert_eq!(
        p.why,
        format!("{} did not answer (timed out)", hung.origin())
    );
}

#[test]
fn the_listed_registries_are_families_roots_without_repeats_or_plaintext() {
    let list: Vec<String> = [
        "https://app.k7.rocks/band/a",
        "https://app.k7.rocks",
        "http://studio.lan:8080",
        "not a url",
        "ftp://x.example",
        "https://other.example:8443",
    ]
    .map(String::from)
    .into();
    assert_eq!(
        registry::resolve(&list),
        ["https://app.k7.rocks", "https://other.example:8443"]
    );
}

#[test]
fn an_extra_field_rides_every_presence() {
    registry::set_presence_extra("protocol", Some(Value::from(7)));
    assert_eq!(registry::presence_extras()["protocol"], 7);
    registry::set_presence_extra("protocol", None);
    assert!(!registry::presence_extras().contains_key("protocol"));
}
