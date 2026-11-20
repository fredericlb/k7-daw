//! The hub's address in an account's environment: none when the address is empty, its secret when it has one.

use std::collections::BTreeMap;

use super::listings::hub_of;

#[test]
fn the_hub_is_found_by_its_address() {
    let env = BTreeMap::from([("STUDIO_HUB_URL".to_string(), String::new())]);
    assert!(hub_of(&env).is_none());
    let env = BTreeMap::from([
        ("STUDIO_HUB_URL".to_string(), "http://h".to_string()),
        ("STUDIO_HUB_SECRET".to_string(), "s".to_string()),
    ]);
    assert_eq!(
        hub_of(&env),
        Some(("http://h".to_string(), "s".to_string()))
    );
}
