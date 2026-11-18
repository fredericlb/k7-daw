//! blob.rs — `blob.test.ts`'s refs and their listing (1114), and a ref carried by a room and two replicas is in
//! `replica.rs`. What a reader gets wrong: a ref ends with a newline and starts with a NUL, and size is a JS number.

use k7_replica::blob::{BlobId, blob_ref, blobs_of, is_blob_hash, parse_blob_ref};
use serde_json::json;

const H1: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const H2: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[test]
fn a_ref_round_trips_and_refuses_what_is_not_one() {
    let r = blob_ref(H1, 1234).unwrap();
    assert!(r.starts_with('\0'));
    assert_eq!(
        parse_blob_ref(Some(&r)),
        Some(BlobId {
            hash: H1.into(),
            size: 1234.0
        })
    );
    assert_eq!(parse_blob_ref(Some("plugin: mixer\n")), None);
    assert_eq!(parse_blob_ref(None), None);
    assert_eq!(parse_blob_ref(Some(&format!("{r}x"))), None);
    assert_eq!(
        parse_blob_ref(Some(&format!("\0k7fs-blob {} 1\n", "g".repeat(64)))),
        None
    );
    assert!(blob_ref("abc", 1).is_err());
}

#[test]
fn what_is_not_a_hash_or_not_a_ref() {
    assert!(is_blob_hash(H1));
    assert!(!is_blob_hash(&"a".repeat(63)));
    assert!(!is_blob_hash(&"A".repeat(64)));
    assert!(blob_ref(&"A".repeat(64), 1).is_err());
    let at = |tail: &str| parse_blob_ref(Some(&format!("\0k7fs-blob {tail}")));
    assert_eq!(
        at(&format!("{H1} 01\n")),
        Some(BlobId {
            hash: H1.into(),
            size: 1.0
        })
    );
    assert!(
        at(&format!("{H1} {}\n", "9".repeat(400)))
            .unwrap()
            .size
            .is_infinite()
    );
    assert_eq!(at(&format!("{H1} 1")), None);
    assert_eq!(at(&format!("{} 1\n", "a".repeat(63))), None);
    assert_eq!(at(&format!("{H1} -1\n")), None);
}

#[test]
fn listed_once_per_hash_in_first_seen_order() {
    let a = blob_ref(H1, 1).unwrap();
    let ops = vec![
        json!({ "kind": "text", "file": "a.wav", "text": a }),
        json!({ "kind": "text", "file": "b.wav", "text": blob_ref(H1, 1).unwrap() }),
        json!({ "kind": "text", "file": "c.md", "text": "- x\n" }),
        json!({ "kind": "remove", "file": "d.wav" }),
    ];
    assert_eq!(
        blobs_of(&ops),
        vec![BlobId {
            hash: H1.into(),
            size: 1.0
        }]
    );
    // A repeated hash keeps its first position and takes the last ref's size, as a Map's `set` does.
    let ops = vec![
        json!({ "kind": "text", "file": "a", "text": blob_ref(H1, 1).unwrap() }),
        json!({ "kind": "text", "file": "b", "text": blob_ref(H2, 2).unwrap() }),
        json!({ "kind": "text", "file": "c", "text": blob_ref(H1, 3).unwrap() }),
    ];
    let ids = blobs_of(&ops);
    assert_eq!(
        ids.iter()
            .map(|i| (i.hash.as_str(), i.size))
            .collect::<Vec<_>>(),
        vec![(H1, 3.0), (H2, 2.0)]
    );
}
