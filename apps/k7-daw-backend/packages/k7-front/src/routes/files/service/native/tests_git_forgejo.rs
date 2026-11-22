//! tests_git_forgejo.rs — the Forgejo routes against the fake Forgejo (`tests_git_forgejo_fake.rs`): a server probed,
//! added, listed and removed; the browser sign-in from start to callback; the refresh's rotation and its reuse; the key
//! here, elsewhere or under a taken title; the clamped repository pages; creation with its topic and a taken name.

use hyper::Method;
use serde_json::{Value, json};

use super::git::Kind;
use super::git_forge_routes::Verb;
use super::tests_git::{Dev, ask, device, post};
use super::tests_git_forgejo_fake::{self as fake, Fake, LOGIN};

pub(super) fn on(id: &str, v: Verb) -> Kind {
    Kind::Forge(id.into(), v)
}

/// The fake's id as k7 names it: `127.0.0.1:<port>`.
fn id_of(url: &str) -> String {
    url.trim_start_matches("http://").to_string()
}

pub(super) async fn added(name: &str) -> (Dev, Fake, String) {
    let (f, url) = fake::start().await;
    let mut d = device(name);
    d.ctx.port = 43210;
    let body = json!({ "url": url, "clientId": "cid" });
    let (s, v, _) = post(&d, Kind::ForgeAdd, "/git/forges", body).await;
    assert_eq!((s, &v["added"]), (200, &json!(true)), "{v}");
    (d, f, id_of(&url))
}

/// Start, the browser's visit to the consent page, and the callback; answers the callback's page.
pub(super) async fn sign_in(d: &Dev, id: &str) -> (u16, String) {
    let (_, began, _) = post(d, on(id, Verb::Start), "/start", json!({})).await;
    let (s, at) = fake::visit(began["authorizeUrl"].as_str().unwrap()).await;
    assert_eq!(s, 302);
    let at = at.unwrap();
    let query = at.split_once('?').unwrap().1;
    let (s, _, page) = ask(
        d,
        Kind::ForgeCallback,
        "GET",
        &format!("/git/forge/callback?{query}"),
        None,
    )
    .await;
    (s, page)
}

async fn poll(d: &Dev, id: &str) -> Value {
    post(d, on(id, Verb::Poll), "/poll", json!({})).await.1
}

pub(super) fn kept(d: &Dev, id: &str) -> Value {
    let text = d.ctx.secrets.get(&format!("forge:{id}"), "k7").unwrap();
    text.map_or(Value::Null, |t| serde_json::from_str(&t).unwrap())
}

#[tokio::test(flavor = "multi_thread")]
async fn forgejo_server_is_probed_added_listed_and_removed() {
    let (f, url) = fake::start().await;
    let d = device("fj-servers");
    let (s, v, _) = post(
        &d,
        Kind::ForgeProbe,
        "/git/forges/probe",
        json!({ "url": url }),
    )
    .await;
    assert_eq!(s, 200, "{v}");
    assert_eq!(
        (&v["pageSize"], &v["lfs"], &v["rootAnswers"]),
        (&json!(50), &json!(true), &json!(true))
    );
    assert_eq!(
        v["redirects"],
        json!([
            "http://127.0.0.1/api/files/git/forge/callback",
            "rocks.k7.daw://callback"
        ])
    );
    let refused = [
        (json!({ "url": "ftp://x.example" }), 400),
        (json!({ "url": "http://127.0.0.1:1" }), 502),
        (json!({ "url": format!("{url}/nothing") }), 502),
    ];
    for (b, want) in refused {
        assert_eq!(
            post(&d, Kind::ForgeProbe, "/git/forges/probe", b).await.0,
            want
        );
    }
    // No client id is Forgejo's built-in one: accepted, and replaced by the registered one below.
    let no_client = json!({ "url": url });
    assert_eq!(
        post(&d, Kind::ForgeAdd, "/git/forges", no_client).await.0,
        200
    );
    let add = json!({ "url": url, "clientId": "cid" });
    assert_eq!(post(&d, Kind::ForgeAdd, "/git/forges", add).await.0, 200);
    let (_, list, _) = ask(&d, Kind::Forges, "GET", "/git/forges", None).await;
    let id = id_of(&url);
    let fj = &list["forges"][1];
    assert_eq!(
        (&fj["id"], &fj["kind"], &fj["url"]),
        (&json!(id), &json!("forgejo"), &json!(url))
    );
    assert_eq!(fj["available"], true);
    assert!(
        f.lock().unwrap().asked.iter().all(|a| !a.contains("/user")),
        "the probe needs no token"
    );
    let (s, _, _) = post(&d, on(&id, Verb::Remove), "/remove", json!({})).await;
    assert_eq!(s, 200);
    let (_, list, _) = ask(&d, Kind::Forges, "GET", "/git/forges", None).await;
    assert_eq!(list["forges"].as_array().unwrap().len(), 1);
    let gh = post(&d, on("github.com", Verb::Remove), "/remove", json!({})).await;
    assert_eq!((gh.0, &gh.1["code"]), (400, &json!("built-in")));
}

#[tokio::test(flavor = "multi_thread")]
async fn forgejo_sign_in_ends_at_the_callback_with_the_token_kept_and_the_key_uploaded() {
    let (d, f, id) = added("fj-signin").await;
    assert_eq!(
        Kind::of(&Method::GET, "/git/forge/callback"),
        Some(Kind::ForgeCallback)
    );
    let (_, began, _) = post(&d, on(&id, Verb::Start), "/start", json!({})).await;
    let at = began["authorizeUrl"].as_str().unwrap();
    for part in [
        "code_challenge_method=S256",
        "scope=write%3Auser+write%3Arepository",
        "redirect_uri=http%3A%2F%2F127.0.0.1%3A43210%2Fapi%2Ffiles%2Fgit%2Fforge%2Fcallback",
    ] {
        assert!(at.contains(part), "{part} in {at}");
    }
    assert!(!at.contains("verifier"));
    assert_eq!(poll(&d, &id).await["state"], "pending");
    let (s, page) = sign_in(&d, &id).await;
    assert_eq!(s, 200);
    assert!(page.contains("you can close this tab"), "{page}");
    let done = poll(&d, &id).await;
    assert_eq!(
        (&done["state"], &done["login"]),
        (&json!("ok"), &json!(LOGIN))
    );
    assert_eq!(done["scopes"], json!(["write:user", "write:repository"]));
    assert!(done.get("keyNote").is_none() && !done.to_string().contains("at-"));
    let k = kept(&d, &id);
    assert!(k["refresh"].as_str().unwrap().starts_with("rt-") && k["expiry"].as_u64().unwrap() > 0);
    let keys = f.lock().unwrap().keys.clone();
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].0, "k7 — Test device");
    assert_eq!(poll(&d, &id).await["state"], "expired");
    let (_, status, _) = ask(&d, on(&id, Verb::Status), "GET", "/", None).await;
    assert_eq!(status["login"], LOGIN);
}

#[tokio::test(flavor = "multi_thread")]
async fn forgejo_built_in_client_signs_in_through_the_front_root_and_never_the_app_scheme() {
    let (f, url) = fake::start().await;
    let mut d = device("fj-builtin");
    d.ctx.port = 43210;
    let (s, v, _) = post(&d, Kind::ForgeAdd, "/git/forges", json!({ "url": url })).await;
    assert_eq!(s, 200, "{v}");
    let id = id_of(&url);
    // A phone asks for the app's scheme, which the built-in client is not registered with: the root it is.
    for body in [json!({}), json!({ "redirect": "app" })] {
        let (_, began, _) = post(&d, on(&id, Verb::Start), "/start", body).await;
        assert_eq!(began["redirect"], "http://127.0.0.1:43210/", "{began}");
        let at = began["authorizeUrl"].as_str().unwrap();
        assert!(at.contains(&format!("client_id={}", fake::BUILTIN)), "{at}");
    }
    let (s, page) = sign_in(&d, &id).await;
    assert_eq!(s, 200, "{page}");
    assert_eq!(poll(&d, &id).await["login"], LOGIN);
    assert_eq!(f.lock().unwrap().keys.len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn forgejo_callback_refuses_a_stale_state_and_says_a_denial() {
    let (d, f, id) = added("fj-deny").await;
    let (s, page, _) = ask(
        &d,
        Kind::ForgeCallback,
        "GET",
        "/git/forge/callback?code=x&state=nope",
        None,
    )
    .await;
    assert!(s == 400 && page.is_null());
    f.lock().unwrap().deny = true;
    let (s, page) = sign_in(&d, &id).await;
    assert_eq!(s, 400);
    assert!(page.contains("denied"), "{page}");
    assert_eq!(poll(&d, &id).await["state"], "denied");
    assert!(kept(&d, &id).is_null());
    let (_, began, _) = post(
        &d,
        on(&id, Verb::Start),
        "/start",
        json!({ "redirect": "app" }),
    )
    .await;
    assert_eq!(began["redirect"], "rocks.k7.daw://callback");
}

#[tokio::test(flavor = "multi_thread")]
async fn forgejo_fake_refuses_what_the_real_one_refuses() {
    let (_, url) = fake::start().await;
    let ask_for = |redirect: &str, pkce: bool| {
        let mut q = format!(
            "{url}/login/oauth/authorize?client_id=cid&response_type=code&state=s&redirect_uri={redirect}"
        );
        if pkce {
            q.push_str("&code_challenge=abc&code_challenge_method=S256");
        }
        q
    };
    let good = "http%3A%2F%2F127.0.0.1%3A9%2Fapi%2Ffiles%2Fgit%2Fforge%2Fcallback";
    assert_eq!(
        fake::visit(&ask_for(
            "http%3A%2F%2Flocalhost%3A9%2Fapi%2Ffiles%2Fgit%2Fforge%2Fcallback",
            true
        ))
        .await
        .0,
        400
    );
    assert_eq!(
        fake::visit(&ask_for("http%3A%2F%2F127.0.0.1%3A9%2Fother", true))
            .await
            .0,
        400
    );
    let (_, at) = fake::visit(&ask_for(good, false)).await;
    assert!(at.unwrap().contains("error=invalid_request"));
    let (s, at) = fake::visit(&ask_for(good, true)).await;
    assert!(s == 302 && at.unwrap().contains("code="));
}

#[tokio::test(flavor = "multi_thread")]
async fn forgejo_refresh_rotates_and_a_reused_token_means_sign_in_again() {
    let (d, f, id) = added("fj-refresh").await;
    f.lock().unwrap().ttl = 30;
    sign_in(&d, &id).await;
    let first = kept(&d, &id)["refresh"].clone();
    let (s, _, _) = ask(&d, on(&id, Verb::Repos), "GET", "/repos", None).await;
    assert_eq!(s, 200);
    let second = kept(&d, &id)["refresh"].clone();
    assert_ne!(first, second);
    assert_eq!(
        f.lock().unwrap().refresh.as_deref(),
        second.as_str(),
        "the new refresh token is kept"
    );
    let mut stale = kept(&d, &id);
    stale["refresh"] = first;
    d.ctx
        .secrets
        .set(&format!("forge:{id}"), "k7", &stale.to_string())
        .unwrap();
    let (s, v, _) = ask(&d, on(&id, Verb::Repos), "GET", "/repos", None).await;
    assert_eq!((s, &v["code"]), (401, &json!("forgejo-signout")));
    assert!(kept(&d, &id).is_null());
    assert!(
        f.lock().unwrap().refresh.is_none(),
        "a reuse kills the newest refresh token too"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn forgejo_key_already_here_is_not_posted() {
    let (d, f, id) = added("fj-key-here").await;
    let ours = k7_git::auth::public_key(&*d.ctx.secrets).unwrap();
    f.lock().unwrap().keys.push(("laptop".into(), ours));
    sign_in(&d, &id).await;
    assert!(poll(&d, &id).await.get("keyNote").is_none());
    let st = f.lock().unwrap();
    assert!(
        st.asked
            .iter()
            .any(|a| a.starts_with("GET /api/v1/user/keys?fingerprint=SHA256%3A"))
    );
    assert!(!st.asked.iter().any(|a| a == "POST /api/v1/user/keys"));
}

#[tokio::test(flavor = "multi_thread")]
async fn forgejo_key_on_another_account_is_a_sentence() {
    let (d, f, id) = added("fj-key-else").await;
    let ours = k7_git::auth::public_key(&*d.ctx.secrets).unwrap();
    f.lock().unwrap().elsewhere.push(ours);
    sign_in(&d, &id).await;
    let v = poll(&d, &id).await;
    assert_eq!(v["state"], "ok");
    assert_eq!(
        v["keyNote"],
        json!(format!("This key belongs to another account on {id}"))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn forgejo_key_title_taken_is_retried_once_with_a_suffix() {
    let (d, f, id) = added("fj-key-title").await;
    f.lock()
        .unwrap()
        .keys
        .push(("k7 — Test device".into(), "ssh-ed25519 b3RoZXI=".into()));
    sign_in(&d, &id).await;
    assert!(poll(&d, &id).await.get("keyNote").is_none());
    let keys = f.lock().unwrap().keys.clone();
    assert_eq!(keys.len(), 2);
    assert!(keys[1].0.starts_with("k7 — Test device (") && keys[1].0.ends_with(')'));
}

#[tokio::test(flavor = "multi_thread")]
async fn forgejo_repos_follow_the_server_s_page_size() {
    let (d, f, id) = added("fj-repos").await;
    {
        let mut st = f.lock().unwrap();
        st.repos = (0..62).map(|i| format!("r{i:02}")).collect();
        st.topics
            .insert(format!("{LOGIN}/r07"), vec!["k7-workspace".into()]);
    }
    assert_eq!(
        ask(&d, on(&id, Verb::Repos), "GET", "/repos", None).await.0,
        401
    );
    sign_in(&d, &id).await;
    let (s, v, _) = ask(&d, on(&id, Verb::Repos), "GET", "/repos", None).await;
    assert_eq!(s, 200);
    let rows = v["repos"].as_array().unwrap();
    assert_eq!(rows.len(), 62);
    assert_eq!(
        (&rows[7]["fullName"], &rows[7]["k7"]),
        (&json!("fred/r07"), &json!(true))
    );
    assert_eq!(rows[61]["sshUrl"], "git@127.0.0.1:fred/r61.git");
    let st = f.lock().unwrap();
    assert!(
        st.asked
            .iter()
            .any(|a| a == "GET /api/v1/user/repos?limit=50&page=2")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn forgejo_creates_a_private_repository_with_the_topic_and_says_a_taken_name() {
    let (d, f, id) = added("fj-create").await;
    f.lock().unwrap().collab.push("ann/shared".into());
    sign_in(&d, &id).await;
    let (s, v, _) = post(
        &d,
        on(&id, Verb::Create),
        "/repos",
        json!({ "name": "song" }),
    )
    .await;
    assert_eq!(
        (s, &v["sshUrl"]),
        (200, &json!("git@127.0.0.1:fred/song.git")),
        "{v}"
    );
    {
        let st = f.lock().unwrap();
        let sent = &st.created[0];
        assert_eq!(
            (
                &sent["private"],
                &sent["auto_init"],
                &sent["default_branch"]
            ),
            (&json!(true), &json!(false), &json!("main"))
        );
        assert_eq!(st.topics["fred/song"], ["k7-workspace"]);
    }
    let (s, v, _) = post(
        &d,
        on(&id, Verb::Create),
        "/repos",
        json!({ "name": "song" }),
    )
    .await;
    assert_eq!((s, &v["code"]), (409, &json!("taken")));
    assert!(
        v["error"]
            .as_str()
            .unwrap()
            .contains("song is already taken")
    );
    let Some(forge) = super::git_forge::find(&d.ctx, &id) else {
        panic!("the server is listed");
    };
    super::git_forge_routes::tag_topic(&d.ctx, &forge, "git@127.0.0.1:ann/shared.git").await;
    super::git_forge_routes::tag_topic(&d.ctx, &forge, "git@127.0.0.1:fred/old.git").await;
    let st = f.lock().unwrap();
    assert!(!st.topics.contains_key("ann/shared"));
    assert_eq!(st.topics["fred/old"], ["k7-workspace"]);
}
