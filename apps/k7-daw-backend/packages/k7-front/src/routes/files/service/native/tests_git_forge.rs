//! tests_git_forge.rs — the forge seam's routes: `/git/forges/<id>/...` and the `/git/github/...` aliases reach the same
//! account, the account list, an unknown or malformed id, and GitHub's token read from its existing keychain entry.

use hyper::Method;
use serde_json::json;

use super::git::Kind;
use super::git_forge_routes::Verb;
use super::tests_git::{ask, device};

fn forge(v: Verb) -> Option<Kind> {
    Some(Kind::Forge("github.com".into(), v))
}

#[test]
fn forge_routes_and_their_github_aliases_name_the_same_verb() {
    for (m, new, old, v) in [
        (
            Method::GET,
            "/git/forges/github.com",
            "/git/github",
            Verb::Status,
        ),
        (
            Method::POST,
            "/git/forges/github.com/start",
            "/git/github/start",
            Verb::Start,
        ),
        (
            Method::POST,
            "/git/forges/github.com/poll",
            "/git/github/poll",
            Verb::Poll,
        ),
        (
            Method::GET,
            "/git/forges/github.com/repos",
            "/git/github/repos",
            Verb::Repos,
        ),
        (
            Method::POST,
            "/git/forges/github.com/repos",
            "/git/github/repos",
            Verb::Create,
        ),
        (
            Method::POST,
            "/git/forges/github.com/signout",
            "/git/github/signout",
            Verb::Signout,
        ),
    ] {
        assert_eq!(Kind::of(&m, new), forge(v), "{m} {new}");
        assert_eq!(Kind::of(&m, old), forge(v), "{m} {old}");
    }
    assert_eq!(Kind::of(&Method::GET, "/git/forges"), Some(Kind::Forges));
    for (m, p) in [
        (Method::POST, "/git/forges/github.com"),
        (Method::GET, "/git/forges/"),
        (Method::GET, "/git/forges/a%2Fb/repos"),
        (Method::GET, "/git/forges/github.com/repos/x"),
        (Method::GET, "/git/githubx"),
    ] {
        assert_eq!(Kind::of(&m, p), None, "{m} {p}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn forge_list_has_github_and_reads_the_token_github_already_kept() {
    let d = device("forge-list");
    let (s, v, _) = ask(&d, Kind::Forges, "GET", "/git/forges", None).await;
    assert_eq!(s, 200);
    let gh = &v["forges"][0];
    assert_eq!(
        (&gh["id"], &gh["kind"]),
        (&json!("github.com"), &json!("github"))
    );
    assert!(gh.get("login").is_none());
    let kept = json!({ "token": "tok", "login": "octo", "scopes": ["repo"] });
    d.ctx
        .secrets
        .set("github-oauth", "k7", &kept.to_string())
        .unwrap();
    let (_, v, text) = ask(&d, Kind::Forges, "GET", "/git/forges", None).await;
    assert_eq!(v["forges"][0]["login"], "octo");
    assert!(!text.contains("tok\""), "the token is never answered");
    let new = forge(Verb::Status).unwrap();
    let (_, by_id, _) = ask(&d, new, "GET", "/git/forges/github.com", None).await;
    let old = Kind::of(&Method::GET, "/git/github").unwrap();
    let (_, by_alias, _) = ask(&d, old, "GET", "/git/github", None).await;
    assert_eq!(by_id, by_alias);
    assert_eq!(by_id["login"], "octo");
    let out = Kind::of(&Method::POST, "/git/github/signout").unwrap();
    assert_eq!(
        ask(&d, out, "POST", "/git/github/signout", None).await.0,
        200
    );
    assert!(d.ctx.secrets.get("github-oauth", "k7").unwrap().is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn forge_unknown_account_is_404() {
    let d = device("forge-unknown");
    let k = Kind::Forge("gitlab.com".into(), Verb::Repos);
    let (s, v, _) = ask(&d, k, "GET", "/git/forges/gitlab.com/repos", None).await;
    assert_eq!((s, &v["code"]), (404, &json!("unknown-forge")));
}
