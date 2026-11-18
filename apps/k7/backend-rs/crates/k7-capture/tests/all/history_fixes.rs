//! history_fixes.rs — the device history's failures: a room that cannot load, the history kept when a binary is in the
//! way of a restore, and a fork's copy that takes the room with it (`docs/files/history.md`).
//! What a reader gets wrong: a room's folder is edited by hand here, with the account stopped and the room let go of.

use k7_capture::account::Ask;
use k7_capture::disk::LiveLog;
use serde_json::json;

use crate::account_attach::{FILE, V1, WS, rig};
use crate::history_device::{again, room, swap};
use crate::rig::put;
use crate::support::wait_for;

const V2: &str = "tempo(140)\n";

#[tokio::test(flavor = "multi_thread")]
async fn a_room_that_cannot_load_never_stops_the_capture_and_the_page_says_why() {
    let r = rig("hf-corrupt");
    let root = format!("{}/{WS}", r.workspaces);
    put(&root, FILE, V1);
    let a = r.acct.attach(&root, WS).await.unwrap().unwrap();
    let log_dir = a.log_dir.clone();
    r.acct.stop_all().await;
    r.acct.histories().release(&log_dir);
    let room_json = r.acct.histories().dir(&log_dir).join("room.json");
    std::fs::write(&room_json, "{not json").unwrap();
    let second = again(&r);
    let a = second
        .attach(&root, WS)
        .await
        .expect("the capture starts")
        .unwrap();
    let before = LiveLog::new(&log_dir).tail(100.0).unwrap().len();
    swap(&root, FILE, V2);
    a.handle.capture(vec![FILE.to_string()]).await;
    wait_for("the edit is in the capture's log", || {
        LiveLog::new(&log_dir).tail(100.0).unwrap().len() > before
    })
    .await;
    let page = Ask::Page {
        before: None,
        limit: None,
    };
    let page = second.history_ask(&root, page).await.unwrap();
    assert!(
        page["error"].as_str().is_some_and(|e| !e.is_empty()),
        "{page}"
    );
    assert_eq!(page["entries"].as_array().map(Vec::len), Some(0));
    second.stop_all().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_restore_brings_back_the_texts_and_leaves_a_binary_file_as_it_is() {
    let r = rig("hf-binary");
    let root = format!("{}/{WS}", r.workspaces);
    put(&root, FILE, V1);
    let a = r.acct.attach(&root, WS).await.unwrap().unwrap();
    swap(&root, FILE, V2);
    a.handle.capture(vec![FILE.to_string()]).await;
    wait_for("the edit", || room(&r.acct, &a.log_dir, FILE).0 == 2.0).await;
    // A recording arrives at entry 3: a blob ref in the room, its bytes on the disk.
    let blob = format!("\u{0}k7blob:{}:4", "ab".repeat(32));
    let entry = json!({
        "id": "wav", "replica": "r", "local": 1, "ts": 5, "label": "take",
        "ops": [{ "kind": "text", "file": "take.wav", "text": blob, "was": null }],
    });
    r.acct.histories().record(&a.log_dir, &entry).unwrap();
    put(&root, "take.wav", [1u8, 2, 3, 4]);
    let back = Ask::Restore {
        body: json!({ "seq": 1 }),
    };
    let back = r.acct.history_ask(&root, back).await.unwrap();
    assert_eq!(back["kept"], json!(["take.wav"]));
    assert_eq!(
        std::fs::read_to_string(format!("{root}/{FILE}")).unwrap(),
        V1
    );
    assert_eq!(
        std::fs::read(format!("{root}/take.wav")).unwrap(),
        [1, 2, 3, 4]
    );
    assert_eq!(room(&r.acct, &a.log_dir, "take.wav").1, Some(blob));
    r.acct.stop_all().await;
}
