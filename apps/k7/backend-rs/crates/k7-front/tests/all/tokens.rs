//! The library tokens through the front, with a device's local user signing the browser in, and the file they
//! share with the TypeScript backend.

use crate::support;

use serde_json::Value;
use support::{front, send};

#[tokio::test]
async fn tokens_are_minted_listed_and_revoked_in_the_shared_file() {
    let f = front("tokens", &[("STUDIO_LOCAL_USER", "1")]).await;
    let me = send(f.addr, "GET", "/auth/local", &[], "").await.cookies();
    let c = ("Cookie", me.as_str());
    assert_eq!(
        send(f.addr, "POST", "/api/token", &[], "").await.status,
        401
    );
    let made = send(
        f.addr,
        "POST",
        "/api/token",
        &[c, ("Content-Type", "application/json")],
        r#"{"label":"  cli  "}"#,
    )
    .await;
    assert_eq!(made.status, 200, "{}", made.body);
    let made: Value = serde_json::from_str(&made.body).unwrap();
    assert_eq!(made["label"], "cli");
    assert_eq!(made["lastUsed"], Value::Null);
    let file: Value =
        serde_json::from_str(&std::fs::read_to_string(f.auth.join("library-tokens.json")).unwrap())
            .unwrap();
    assert_eq!(file[0]["id"], made["id"]);
    assert_eq!(file[0]["sub"], "dev-user");
    let secret = "test-secret:pl8";
    let token = made["token"].as_str().unwrap();
    let claims = k7_auth::api_token::claims(secret, token).unwrap();
    assert_eq!(claims.sub, "dev-user");
    let bearer = format!("Bearer {token}");
    let handed = send(
        f.addr,
        "GET",
        "/api/token",
        &[c, ("Authorization", &bearer)],
        "",
    )
    .await;
    assert_eq!(
        handed.status, 502,
        "a bearer is checked by the backend, which holds the ephemeral tokens"
    );
    let list = send(f.addr, "GET", "/api/token", &[c], "").await;
    assert_eq!(list.header("cache-control"), Some("no-store"));
    let list: Value = serde_json::from_str(&list.body).unwrap();
    assert_eq!(list["tokens"][0]["id"], made["id"]);
    assert!(list["tokens"][0].get("token").is_none());
    let path = format!("/api/token/{}", made["id"].as_str().unwrap());
    let cross = send(
        f.addr,
        "DELETE",
        &path,
        &[c, ("Origin", "https://evil.example")],
        "",
    )
    .await;
    assert_eq!(cross.status, 403);
    assert_eq!(send(f.addr, "DELETE", &path, &[c], "").await.status, 200);
    assert_eq!(send(f.addr, "DELETE", &path, &[c], "").await.status, 404);
    let pair = send(f.addr, "GET", "/api/token/pair", &[c], "").await;
    assert_eq!(
        pair.status, 502,
        "the backend answers what the front does not own"
    );
}
