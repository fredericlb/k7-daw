//! The session a device has, through the front: `/auth/local` signs the local user in, `/auth/me` reads it, a
//! logout everywhere is written where the TypeScript backend reads it, and no sign-in route is left to a provider.

use crate::support;

use support::{front, send};

#[tokio::test]
async fn a_device_signs_its_local_user_in_and_rewrites_any_other() {
    let f = front("local", &[("STUDIO_LOCAL_USER", "1")]).await;
    let local = send(f.addr, "GET", "/auth/local?return_to=%2Fx", &[], "").await;
    assert_eq!((local.status, local.header("location")), (302, Some("/x")));
    let me = send(
        f.addr,
        "GET",
        "/auth/me",
        &[("Cookie", &local.cookies())],
        "",
    )
    .await;
    assert!(me.body.contains("\"local\":true"), "{}", me.body);
    assert_eq!(me.header("cache-control"), Some("no-store"));
    let away = send(
        f.addr,
        "GET",
        "/auth/local?return_to=https%3A%2F%2Fevil.example",
        &[],
        "",
    )
    .await;
    assert_eq!(away.header("location"), Some("/"));
}

#[tokio::test]
async fn a_session_that_is_not_the_local_users_is_rewritten_on_a_device() {
    let f = front("rewrite", &[("STUDIO_LOCAL_USER", "1")]).await;
    let other = k7_auth::cookie_header(
        serde_json::json!({ "sub": "ann", "name": "Ann", "email": "ann@x" })
            .as_object()
            .unwrap()
            .clone(),
        "studio.sid",
        "test-secret",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as f64,
    );
    let me = send(f.addr, "GET", "/auth/me", &[("Cookie", &other)], "").await;
    assert!(
        me.body.contains("Ann"),
        "/auth/me sits before the rewrite: {}",
        me.body
    );
    let list = send(f.addr, "GET", "/api/token", &[("Cookie", &other)], "").await;
    assert_eq!(list.status, 200);
    assert!(
        list.headers.iter().any(|(k, _)| k == "set-cookie"),
        "the token routes sit after it"
    );
}

#[tokio::test]
async fn logout_everywhere_is_written_where_the_backend_reads_it() {
    let f = front("logout", &[("STUDIO_LOCAL_USER", "1")]).await;
    let cookie = send(f.addr, "GET", "/auth/local", &[], "").await.cookies();
    // A copy kept elsewhere was issued before the logout: on a fast host both could fall in one millisecond.
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    let cross = send(
        f.addr,
        "POST",
        "/auth/logout?everywhere=1",
        &[("Cookie", &cookie), ("Sec-Fetch-Site", "cross-site")],
        "",
    )
    .await;
    assert_eq!(cross.status, 403);
    let out = send(
        f.addr,
        "POST",
        "/auth/logout?everywhere=1",
        &[("Cookie", &cookie)],
        "",
    )
    .await;
    assert_eq!(out.status, 200);
    assert!(out.headers.iter().any(|(k, v)| k == "set-cookie"
        && v.starts_with("studio.sid=; path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT")));
    let file = std::fs::read_to_string(f.auth.join("session-revocations.json")).unwrap();
    assert!(file.starts_with("{\n  \"dev-user\": "), "{file}");
    let me = send(f.addr, "GET", "/auth/me", &[("Cookie", &cookie)], "").await;
    assert_eq!(me.status, 401, "the copy kept elsewhere is dead too");
}

#[tokio::test]
async fn an_anonymous_me_is_a_401_with_no_provider() {
    let f = front("noprovider", &[("STUDIO_LOCAL_USER", "1")]).await;
    let me = send(f.addr, "GET", "/auth/me", &[], "").await;
    assert_eq!(me.status, 401);
    let health = send(f.addr, "GET", "/api/health", &[], "").await;
    assert_eq!(health.status, 200);
}

#[tokio::test]
async fn no_sign_in_route_is_left_to_the_front() {
    let f = front("nosignin", &[("STUDIO_LOCAL_USER", "1")]).await;
    for path in [
        "/auth/login",
        "/auth/callback",
        "/auth/handoff/issue",
        "/auth/handoff/redeem",
    ] {
        let r = send(f.addr, "GET", path, &[], "").await;
        assert_eq!(
            r.status, 502,
            "{path} goes to the backend, which has no such route"
        );
    }
}
