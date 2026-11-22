//! tests_git_author.rs — the commit author's prefill: each source in order (forge, git config, system), each field on its
//! own, a typed field never replaced, `/user` read as GitHub's noreply form or Forgejo's, and no source is no identity.

use serde_json::json;

use super::git::Kind;
use super::git_author::{Local, Person, Source, learn, read_user, resolve};
use super::git_forge::{Forge, Saved, accounts, keep};
use super::tests_git::{Dev, ask, device, post};
use super::tests_git_forgejo::{added, sign_in};

fn github(d: &Dev) -> Forge {
    accounts(&d.ctx).remove(0)
}

fn sign_in_github(d: &Dev, body: serde_json::Value) {
    let f = github(d);
    let s = Saved {
        token: "t".into(),
        login: "octo".into(),
        scopes: vec![],
        refresh: None,
        expiry: None,
    };
    keep(&d.ctx, &f, &s).unwrap();
    learn(&d.ctx, &f, &body);
}

fn person(name: &str, email: &str) -> Person {
    Person {
        name: name.into(),
        email: email.into(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_forge_answer_is_read_with_the_noreply_address_for_a_private_github_email() {
    let d = device("author-read");
    let g = github(&d);
    let open = json!({ "id": 7, "login": "octo", "name": "Octo Cat", "email": "o@cat.org" });
    assert_eq!(read_user(&g, &open), person("Octo Cat", "o@cat.org"));
    let private = json!({ "id": 7, "login": "octo", "name": null, "email": null });
    let want = person("octo", "7+octo@users.noreply.github.com");
    assert_eq!(read_user(&g, &private), want);
}

#[tokio::test(flavor = "multi_thread")]
async fn each_source_fills_in_order_and_a_typed_field_stays() {
    let mut d = device("author-order");
    let none = resolve(&d.ctx).unwrap();
    assert!(!none.id.is_complete(), "no source is no author");
    d.ctx.local = || Local {
        config: person("Config Ann", "ann@config.org"),
        system_name: "System Ann".into(),
    };
    let r = resolve(&d.ctx).unwrap();
    assert_eq!(r.id.name, "Config Ann");
    assert_eq!(r.name_from, Some(Source::GitConfig));
    assert_eq!(r.email_from, Some(Source::GitConfig));
    // A signed-in GitHub comes first, and its private email still gives an address.
    sign_in_github(&d, json!({ "id": 9, "login": "octo", "name": "Octo" }));
    let r = resolve(&d.ctx).unwrap();
    let (n, e) = (r.id.name.as_str(), r.id.email.as_str());
    assert_eq!((n, e), ("Octo", "9+octo@users.noreply.github.com"));
    assert_eq!(r.name_from, Some(Source::GitHub));
    // An email typed by hand stays while the name still comes from GitHub.
    let typed = k7_git::Identity {
        name: String::new(),
        email: "me@mine.org".into(),
    };
    d.ctx.identity.save(&typed).unwrap();
    let r = resolve(&d.ctx).unwrap();
    let (n, e) = (r.id.name.as_str(), r.id.email.as_str());
    assert_eq!((n, e), ("Octo", "me@mine.org"));
    assert_eq!((r.name_from, r.email_from), (Some(Source::GitHub), None));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_system_account_gives_a_name_only() {
    let mut g = device("author-system");
    g.ctx.local = || Local {
        config: Person::default(),
        system_name: "Sys Tem".into(),
    };
    let r = resolve(&g.ctx).unwrap();
    assert_eq!(r.id.name, "Sys Tem");
    assert_eq!(r.name_from, Some(Source::System));
    assert!(r.id.email.is_empty() && !r.id.is_complete());
}

#[test]
fn the_git_config_gives_its_user_and_a_missing_one_gives_nothing() {
    let dir = std::env::temp_dir().join(format!("k7-author-cfg-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("gitconfig");
    std::fs::write(&file, "[user]\n\tname = Cfg Bob\n\temail = bob@cfg.org\n").unwrap();
    let got = k7_git::Identity::from_config_file(&file);
    assert_eq!(
        (got.name.as_str(), got.email.as_str()),
        ("Cfg Bob", "bob@cfg.org")
    );
    let empty = dir.join("empty");
    std::fs::write(&empty, "").unwrap();
    assert_eq!(
        k7_git::Identity::from_config_file(&empty),
        Default::default()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_forgejo_sign_in_names_the_author_and_the_route_says_where_from() {
    let (d, fake, id) = added("author-forgejo").await;
    {
        let mut st = fake.lock().unwrap();
        st.full_name = "Fred Forge".into();
        st.email = "fred@forge.org".into();
    }
    let (s, _) = sign_in(&d, &id).await;
    assert_eq!(s, 200);
    let (_, v, _) = ask(&d, Kind::Identity, "GET", "/git/identity", None).await;
    assert_eq!(v["name"], json!("Fred Forge"));
    assert_eq!(v["email"], json!("fred@forge.org"));
    assert_eq!(v["nameSource"], json!("forgejo"));
    // Typed by hand, the name carries no source any more.
    let typed = json!({ "name": "Typed", "email": "" });
    post(&d, Kind::SetIdentity, "/git/identity", typed).await;
    let (_, v, _) = ask(&d, Kind::Identity, "GET", "/git/identity", None).await;
    assert_eq!(v["name"], json!("Typed"));
    assert!(v["nameSource"].is_null());
    assert_eq!(v["emailSource"], json!("forgejo"));
}
