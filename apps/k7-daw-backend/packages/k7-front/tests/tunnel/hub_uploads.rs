//! hub_uploads.rs — what one tunnel may upload: how many at once, how many bytes each, how long it may sit idle and
//! how many an hour. The limits are shrunk through the testbed's environment, never flooded.

use serde_json::json;

use crate::bed::{Bare, Client, Dir, Hub, HubOptions, UploadLimits, until};

struct Rig {
    dir: Dir,
    _hub: Hub,
    agent: Bare,
    _client: Client,
    ch: i64,
}

async fn open(uploads: UploadLimits) -> Rig {
    let dir = Dir::new("hub-uploads");
    let hub = Hub::start(HubOptions {
        uploads: Some(uploads),
        ..Default::default()
    })
    .await;
    let mut agent = Bare::dial(hub.relay.port, "e1", "t").await;
    hub.until_has("e1").await;
    let roots = json!([{ "remote": dir.s(), "name": "ws", "writable": true }]);
    let path = hub.bind_local("e1", roots, "engine", false).await;
    let client = Client::dial(hub.port(), &path, None).await.unwrap();
    let ch = agent.opened().await;
    Rig {
        dir,
        _hub: hub,
        agent,
        _client: client,
        ch,
    }
}

impl Rig {
    fn put(&self, req: i64, name: &str, bytes: usize, last: bool) {
        let path = format!("{}/{name}", self.dir.s());
        self.agent
            .put_chunk(self.ch, req, &path, &vec![7u8; bytes], last);
    }

    fn entries(&self) -> Vec<String> {
        std::fs::read_dir(&self.dir.0)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect()
    }

    fn has_part(&self) -> bool {
        self.entries().iter().any(|f| f.ends_with(".part"))
    }

    fn replies(&self) -> Vec<serde_json::Value> {
        self.agent.replies("put-reply")
    }
}

#[tokio::test]
async fn lands_a_file_whole_under_its_own_name() {
    let r = open(UploadLimits::default()).await;
    r.put(1, "take.wav", 5, false);
    r.put(1, "take.wav", 5, true);
    until(|| !r.replies().is_empty(), 10_000).await;
    assert_eq!(r.replies()[0]["req"], 1);
    assert!(r.replies()[0]["error"].is_null());
    assert_eq!(std::fs::read(r.dir.join("take.wav")).unwrap().len(), 10);
    assert!(!r.has_part());
}

#[tokio::test]
async fn refuses_an_upload_past_the_number_open_at_once() {
    let r = open(UploadLimits {
        max_open: 2,
        ..Default::default()
    })
    .await;
    r.put(1, "a", 1, false);
    r.put(2, "b", 1, false);
    r.put(3, "c", 1, false);
    until(|| !r.replies().is_empty(), 10_000).await;
    let replies = r.replies();
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0]["req"], 3);
    assert!(replies[0]["error"].is_string());
}

#[tokio::test]
async fn aborts_an_upload_past_its_size_deletes_its_part_file_and_ignores_its_later_chunks() {
    let r = open(UploadLimits {
        max_bytes: 10,
        ..Default::default()
    })
    .await;
    r.put(1, "big", 8, false);
    // The refusal is told before the chain has made its part file: overrun only once it exists.
    until(|| r.has_part(), 10_000).await;
    r.put(1, "big", 8, false);
    r.put(1, "big", 8, true);
    until(|| !r.replies().is_empty() && r.entries().is_empty(), 10_000).await;
    assert_eq!(r.replies().len(), 1);
    assert_eq!(r.replies()[0]["req"], 1);
    assert!(r.replies()[0]["error"].is_string());
    assert!(r.entries().is_empty());
}

#[tokio::test]
async fn aborts_an_upload_that_went_quiet() {
    let r = open(UploadLimits {
        idle_ms: 300,
        ..Default::default()
    })
    .await;
    r.put(1, "slow", 4, false);
    until(|| r.has_part(), 10_000).await;
    until(|| !r.replies().is_empty() && r.entries().is_empty(), 10_000).await;
    assert_eq!(r.replies()[0]["req"], 1);
    assert!(r.replies()[0]["error"].is_string());
    assert!(r.entries().is_empty());
}

#[tokio::test]
async fn stops_at_the_hourly_budget_of_the_tunnel() {
    let r = open(UploadLimits {
        bytes_per_hour: 12,
        ..Default::default()
    })
    .await;
    r.put(1, "a", 8, true);
    r.put(2, "b", 8, true);
    until(|| r.replies().len() >= 2, 10_000).await;
    let by = |n: i64| r.replies().into_iter().find(|x| x["req"] == n).unwrap();
    assert!(by(2)["error"].is_string());
    assert!(by(1)["error"].is_null());
}
