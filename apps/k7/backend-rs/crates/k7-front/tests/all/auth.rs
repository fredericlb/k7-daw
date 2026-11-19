//! The session a device has, through the front: `/auth/me` hands a page the install's token, the routes behind
//! the session answer that token alone, a page on another site gets none, and no sign-in route is left.

use crate::support::{front, send};

fn token_of(cookies: &str) -> &str {
    cookies.strip_prefix("k7.sid=").expect("the k7.sid cookie")
}

#[tokio::test]
async fn a_page_is_given_the_installs_token_and_only_that_token_signs_in() {
    let f = front(
        "grant",
        &[("STUDIO_LOCAL_USER", "1"), ("K7_SESSION_TOKEN", "")],
    )
    .await;
    let me = send(f.addr, "GET", "/auth/me", &[], "").await;
    assert_eq!(me.status, 200);
    assert!(me.body.contains("\"local\":true"), "{}", me.body);
    assert_eq!(me.header("cache-control"), Some("no-store"));
    let line = me.header("set-cookie").expect("the token is handed over");
    assert!(
        line.contains("HttpOnly") && line.contains("SameSite=Strict"),
        "{line}"
    );
    let cookie = me.cookies();
    let file = std::fs::read_to_string(f.auth.join("session-token")).unwrap();
    assert_eq!(
        token_of(&cookie),
        file.trim(),
        "the cookie is the file's token"
    );
    let held = send(f.addr, "GET", "/auth/me", &[("Cookie", &cookie)], "").await;
    assert!(
        held.header("set-cookie").is_none(),
        "a page that holds it is not handed it again"
    );
    let prefs = send(
        f.addr,
        "GET",
        "/api/preferences",
        &[("Cookie", &cookie)],
        "",
    )
    .await;
    assert_eq!(prefs.status, 200);
    let bare = send(f.addr, "GET", "/api/preferences", &[], "").await;
    assert_eq!(bare.status, 401);
    let wrong = send(
        f.addr,
        "GET",
        "/api/preferences",
        &[("Cookie", "k7.sid=not-the-token")],
        "",
    )
    .await;
    assert_eq!(wrong.status, 401);
}

#[cfg(unix)]
#[tokio::test]
async fn the_token_is_kept_for_the_owner_alone() {
    use std::os::unix::fs::PermissionsExt;
    let f = front(
        "private",
        &[("STUDIO_LOCAL_USER", "1"), ("K7_SESSION_TOKEN", "")],
    )
    .await;
    let mode = std::fs::metadata(f.auth.join("session-token"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o077, 0);
}

#[tokio::test]
async fn a_page_on_another_site_is_given_nothing() {
    let f = front("cross", &[("STUDIO_LOCAL_USER", "1")]).await;
    let me = send(
        f.addr,
        "GET",
        "/auth/me",
        &[("Sec-Fetch-Site", "cross-site")],
        "",
    )
    .await;
    assert_eq!(me.status, 403);
    assert!(me.header("set-cookie").is_none());
}

#[tokio::test]
async fn no_sign_in_route_is_left_to_the_front() {
    let f = front("nosignin", &[("STUDIO_LOCAL_USER", "1")]).await;
    for path in [
        "/auth/login",
        "/auth/callback",
        "/auth/local",
        "/auth/logout",
        "/auth/handoff/issue",
    ] {
        let r = send(f.addr, "GET", path, &[], "").await;
        assert_eq!(r.status, 404, "{path} is no route of this front");
    }
    let health = send(f.addr, "GET", "/api/health", &[], "").await;
    assert_eq!(health.status, 200);
}
