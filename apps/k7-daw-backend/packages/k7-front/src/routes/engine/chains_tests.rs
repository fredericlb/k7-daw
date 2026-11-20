//! chains_tests.rs — what a `PUT /api/engines/:id/chains` body may say: two chain names, `""` to unsay one, nothing
//! else — the write is an engine's OWN settings, so a typo must not reach its `engine-host.json` (stemSplit.md).

use super::chains::chains_issues;
use serde_json::json;

#[test]
fn two_names_are_a_body() {
    assert_eq!(
        chains_issues(&json!({ "stems": "@k7-core/chains/stems.k7x" })),
        None
    );
    assert_eq!(
        chains_issues(&json!({
            "stems": "@k7-core/chains/stems.k7x",
            "instrumental": "@k7-core/chains/instrumental.k7x"
        })),
        None
    );
    // Empty is how an owner unsays one, so it is a name like any other here.
    assert_eq!(chains_issues(&json!({ "instrumental": "" })), None);
    // Naming neither is a no-op the machine answers with what it already holds.
    assert_eq!(chains_issues(&json!({})), None);
}

#[test]
fn anything_else_is_said_by_name() {
    assert!(
        chains_issues(&json!({ "stems": 4 }))
            .unwrap()
            .starts_with("stems:")
    );
    assert!(
        chains_issues(&json!({ "chain": "x" }))
            .unwrap()
            .starts_with("chain:")
    );
    assert!(chains_issues(&json!("@k7-core/chains/stems.k7x")).is_some());
    assert!(chains_issues(&json!(null)).is_some());
}
