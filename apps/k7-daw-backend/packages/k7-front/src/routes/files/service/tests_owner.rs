//! tests_owner.rs — `claim_root`: the first account to reach a home is recorded and no other passes.

use super::claim_root;

#[test]
fn records_the_first_owner_and_refuses_another() {
    let dir = std::env::temp_dir().join(format!("k7-owner-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let _ = std::fs::remove_file(dir.join("owner.json"));
    assert!(claim_root(&dir, "a@x.org"));
    assert!(claim_root(&dir, "a@x.org"));
    assert!(!claim_root(&dir, "b@x.org"));
    assert_eq!(
        std::fs::read_to_string(dir.join("owner.json")).unwrap(),
        "{\n  \"owner\": \"a@x.org\"\n}\n"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
