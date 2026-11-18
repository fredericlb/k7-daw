//! The tokens the TypeScript minted (fixtures/auth-vectors.json): Rust must read and mint the same bytes.

use k7_auth::api_token::{claims, mint};
use serde_json::Value;

fn vectors() -> Value {
    // Compiled in, so the test binary runs on a phone that has no checkout.
    serde_json::from_str(include_str!("../../../../fixtures/auth-vectors.json")).unwrap()
}

#[test]
fn tokens_open_and_mint_the_same_bytes() {
    for v in vectors()["tokens"].as_array().unwrap() {
        let (secret, token) = (v["secret"].as_str().unwrap(), v["token"].as_str().unwrap());
        let c = claims(secret, token).expect("the signature holds");
        assert_eq!(c.sub, v["sub"].as_str().unwrap());
        assert_eq!(c.jti, v["payload"]["jti"].as_str().unwrap());
        let (iat, exp) = (
            v["payload"]["iat"].as_f64().unwrap(),
            v["payload"]["exp"].as_f64().unwrap(),
        );
        let (minted, record) = mint(secret, &c.sub, "vector", &c.jti, iat, exp - iat);
        assert_eq!(minted, token);
        assert_eq!(Value::Object(record), v["record"]);
        assert!(claims("another", token).is_none());
        assert!(claims(secret, &format!("{token}x")).is_none());
    }
}
