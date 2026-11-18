//! The cookies @studio/oidc's TypeScript minted (fixtures/session-vectors.json): Rust must read and mint the same bytes.

use k7_auth::{Session, cookie_header, open, parse_cookie_header, valid_session_user};
use serde_json::{Value, json};

fn vectors() -> Vec<Value> {
    // Compiled in, so the test binary runs on a phone that has no checkout.
    serde_json::from_str(include_str!("../../../../fixtures/session-vectors.json")).unwrap()
}

#[test]
fn mints_the_same_header() {
    for v in vectors() {
        let user = v["user"].as_object().unwrap().clone();
        let header = cookie_header(
            user,
            v["cookieName"].as_str().unwrap(),
            v["secret"].as_str().unwrap(),
            v["iat"].as_f64().unwrap(),
        );
        assert_eq!(header, v["header"].as_str().unwrap());
    }
}

#[test]
fn opens_what_typescript_signed() {
    for v in vectors() {
        let (name, secret) = (
            v["cookieName"].as_str().unwrap(),
            v["secret"].as_str().unwrap(),
        );
        let session = open(
            name,
            v["value"].as_str().unwrap(),
            v["sig"].as_str().unwrap(),
            secret,
        )
        .expect("signature holds");
        assert_eq!(
            Value::Object(session).to_string(),
            v["json"].as_str().unwrap()
        );
    }
}

#[test]
fn refuses_a_wrong_secret_a_tampered_value_and_another_name() {
    let v = &vectors()[0];
    let (value, sig) = (v["value"].as_str().unwrap(), v["sig"].as_str().unwrap());
    assert!(open("studio.sid", value, sig, "not-the-secret").is_none());
    assert!(open("other.sid", value, sig, v["secret"].as_str().unwrap()).is_none());
    let tampered = format!("{}A", &value[..value.len() - 1]);
    assert!(open("studio.sid", &tampered, sig, v["secret"].as_str().unwrap()).is_none());
}

#[test]
fn reads_the_pairs_of_a_header() {
    let v = &vectors()[1];
    let pairs = parse_cookie_header(&format!(
        "a=1; {}; studio.sid=second",
        v["header"].as_str().unwrap()
    ));
    let get = |n: &str| pairs.iter().find(|(k, _)| k == n).map(|(_, x)| x.as_str());
    assert_eq!(get("studio.sid"), v["value"].as_str());
    assert_eq!(get("studio.sid.sig"), v["sig"].as_str());
    assert_eq!(parse_cookie_header("x=%E2%9C%93; y=%zz")[0].1, "\u{2713}");
    assert_eq!(parse_cookie_header("y=%zz")[0].1, "%zz");
}

fn session(user: Value, iat: f64, exp: f64) -> Session {
    json!({ "user": user, "iat": iat, "exp": exp })
        .as_object()
        .unwrap()
        .clone()
}

#[test]
fn validity_follows_the_stamps() {
    let day = 86_400_000.0;
    let none = |_: &str| None;
    let s = session(json!({ "sub": "a", "groups": [] }), 0.0, 7.0 * day);
    assert!(valid_session_user(&s, day, 12.0 * 3_600_000.0, none).is_some());
    assert!(
        valid_session_user(&s, 7.0 * day, 12.0 * 3_600_000.0, none).is_none(),
        "expired"
    );
    let grouped = session(json!({ "sub": "a", "groups": ["g"] }), 0.0, 7.0 * day);
    assert!(
        valid_session_user(&grouped, day, 12.0 * 3_600_000.0, none).is_none(),
        "groups too old"
    );
    assert!(
        valid_session_user(&s, day, 12.0 * 3_600_000.0, |sub| (sub == "a")
            .then_some(1.0))
        .is_none(),
        "revoked"
    );
    let future = session(json!({ "sub": "a" }), 2.0 * day, 9.0 * day);
    assert!(
        valid_session_user(&future, day, 12.0 * 3_600_000.0, none).is_none(),
        "issued in the future"
    );
    let mut expiring = s.clone();
    expiring.insert("_expire".into(), json!(day - 1.0));
    assert!(
        valid_session_user(&expiring, day, 12.0 * 3_600_000.0, none).is_none(),
        "_expire passed"
    );
    let unstamped = json!({ "user": { "sub": "a" } })
        .as_object()
        .unwrap()
        .clone();
    assert!(
        valid_session_user(&unstamped, day, 12.0 * 3_600_000.0, none).is_none(),
        "unstamped"
    );
}
