//! tunnel_files.rs — the agent's file requests over a link whose hub is the test: a stat answered, a download
//! stamped with the server's mtime and renamed over its target, an upload cut to chunks, and what each failure
//! says — a refusal, a download gone quiet, a tunnel dropped with requests in flight.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use k7_engine::tunnel::frames::decode_binary;
use k7_supervisor::tunnel::files::{FileRequests, RemoteStat};
use k7_supervisor::tunnel::filetime;
use k7_supervisor::tunnel::link::{Link, Out};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::scratch::Scratch;

/// A link whose writer is the test: what the agent sends comes out of `sent`.
fn wired() -> (Arc<FileRequests>, mpsc::UnboundedReceiver<Out>, Arc<Link>) {
    let link = Arc::new(Link::default());
    let (tx, rx) = mpsc::unbounded_channel();
    link.attach(tx);
    (Arc::new(FileRequests::new(link.clone())), rx, link)
}

async fn text(rx: &mut mpsc::UnboundedReceiver<Out>) -> Value {
    match rx.recv().await {
        Some(Out::Text(t)) => serde_json::from_str(&t).unwrap(),
        _ => panic!("a text frame"),
    }
}

fn remote(path: &str, mtime: f64) -> RemoteStat {
    RemoteStat {
        path: path.into(),
        kind: "file".into(),
        size: Some(0),
        mtime_ms: Some(mtime),
    }
}

#[tokio::test]
async fn a_stat_is_answered_by_its_request_id() {
    let (reqs, mut sent, _link) = wired();
    let r = reqs.clone();
    let asked = tokio::spawn(async move { r.stat(3, vec!["/a".into(), "/b".into()]).await });
    let m = text(&mut sent).await;
    assert_eq!(
        (m["t"].as_str(), m["ch"].as_i64(), m["paths"].clone()),
        (Some("stat"), Some(3), json!(["/a", "/b"]))
    );
    reqs.on_stat_reply(m["req"].as_i64().unwrap() + 100, &json!([]));
    reqs.on_stat_reply(m["req"].as_i64().unwrap(), &json!([{"path": "/a", "kind": "file", "size": 4, "mtimeMs": 7.5}, {"path": "/b", "kind": "missing"}]));
    let got = asked.await.unwrap().unwrap();
    assert_eq!(
        got[0],
        RemoteStat {
            path: "/a".into(),
            kind: "file".into(),
            size: Some(4),
            mtime_ms: Some(7.5)
        }
    );
    assert_eq!(got[1].kind, "missing");
}

#[tokio::test]
async fn a_download_lands_beside_its_target_then_replaces_it_stamped_with_the_servers_time() {
    let (reqs, mut sent, _link) = wired();
    let dir = Scratch::new("files");
    let local = dir.path().join("deep").join("a.wav");
    std::fs::create_dir_all(local.parent().unwrap()).unwrap();
    std::fs::write(&local, b"old").unwrap();
    let (r, l) = (reqs.clone(), local.clone());
    let fetch = tokio::spawn(async move {
        r.download(2, &remote("/srv/a.wav", 1_600_000_000_000.0), &l)
            .await
    });
    let m = text(&mut sent).await;
    assert_eq!(
        (m["t"].as_str(), m["path"].as_str()),
        (Some("get"), Some("/srv/a.wav"))
    );
    let req = m["req"].as_i64().unwrap();
    reqs.on_chunk(req, false, b"new ").await;
    assert_eq!(
        std::fs::read(&local).unwrap(),
        b"old",
        "the target is not touched until the file is whole"
    );
    reqs.on_chunk(req, true, b"bytes").await;
    fetch.await.unwrap().unwrap();
    assert_eq!(std::fs::read(&local).unwrap(), b"new bytes");
    assert!((filetime::stat(&local).unwrap().mtime_ms - 1_600_000_000_000.0).abs() < 2.0);
    let left: Vec<_> = std::fs::read_dir(local.parent().unwrap())
        .unwrap()
        .collect();
    assert_eq!(left.len(), 1, "no half file is left");
}

#[tokio::test]
async fn a_refused_download_says_why_and_leaves_nothing() {
    let (reqs, mut sent, _link) = wired();
    let dir = Scratch::new("files");
    let local = dir.path().join("a.wav");
    let (r, l) = (reqs.clone(), local.clone());
    let fetch = tokio::spawn(async move { r.download(2, &remote("/srv/a.wav", 1.0), &l).await });
    let m = text(&mut sent).await;
    reqs.on_get_error(m["req"].as_i64().unwrap(), "outside the roots");
    assert_eq!(
        fetch.await.unwrap().unwrap_err().to_string(),
        "outside the roots"
    );
    assert!(!local.exists() && std::fs::read_dir(dir.path()).unwrap().count() == 0);
}

#[tokio::test]
async fn a_download_fails_when_it_goes_quiet_not_when_it_is_long() {
    let link = Arc::new(Link::default());
    let (tx, mut sent) = mpsc::unbounded_channel();
    link.attach(tx);
    let mut reqs = FileRequests::new(link);
    reqs.bounds.download_idle = Duration::from_millis(150);
    let (reqs, dir) = (Arc::new(reqs), Scratch::new("files"));
    let (r, local) = (reqs.clone(), dir.path().join("a.wav"));
    let fetch =
        tokio::spawn(async move { r.download(2, &remote("/srv/a.wav", 1.0), &local).await });
    let req = text(&mut sent).await["req"].as_i64().unwrap();
    // Chunks keep coming past the idle bound in all: the download lives while they do.
    for _ in 0..4 {
        tokio::time::sleep(Duration::from_millis(80)).await;
        reqs.on_chunk(req, false, b"x").await;
    }
    assert!(!fetch.is_finished());
    assert_eq!(
        fetch.await.unwrap().unwrap_err().to_string(),
        "download stalled"
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn an_upload_goes_in_chunks_and_answers_with_the_servers_stat() {
    let (reqs, mut sent, _link) = wired();
    let dir = Scratch::new("files");
    let file = dir.path().join("take.wav");
    let bytes: Vec<u8> = (0..600_000u32).map(|i| (i % 251) as u8).collect();
    std::fs::write(&file, &bytes).unwrap();
    let (r, f) = (reqs.clone(), file.clone());
    let up = tokio::spawn(async move { r.upload(4, &f, "/srv/take.wav").await });
    let mut got = Vec::new();
    let req = loop {
        let Some(Out::Binary(frame)) = sent.recv().await else {
            panic!("a binary frame")
        };
        let (_, start) = decode_binary(&frame).unwrap();
        let head: Value = serde_json::from_slice(&frame[4..start]).unwrap();
        assert_eq!(
            (
                head["t"].as_str(),
                head["ch"].as_i64(),
                head["path"].as_str()
            ),
            (Some("put"), Some(4), Some("/srv/take.wav"))
        );
        got.extend_from_slice(&frame[start..]);
        if head["last"] == true {
            break head["req"].as_i64().unwrap();
        }
    };
    assert_eq!(got, bytes);
    reqs.on_put_reply(
        req,
        &json!({"path": "/srv/take.wav", "kind": "file", "size": 600000, "mtimeMs": 9.0}),
        "",
    );
    assert_eq!(up.await.unwrap().unwrap().mtime_ms, Some(9.0));
}

#[tokio::test]
async fn an_empty_file_is_one_last_chunk_and_a_refusal_is_the_servers_words() {
    let (reqs, mut sent, _link) = wired();
    let dir = Scratch::new("files");
    let empty = dir.path().join("e.wav");
    std::fs::write(&empty, b"").unwrap();
    let (r, f) = (reqs.clone(), empty.clone());
    let up = tokio::spawn(async move { r.upload(1, &f, "/srv/e.wav").await });
    let Some(Out::Binary(frame)) = sent.recv().await else {
        panic!("a frame")
    };
    let (_, start) = decode_binary(&frame).unwrap();
    let head: Value = serde_json::from_slice(&frame[4..start]).unwrap();
    assert_eq!(
        (head["last"].clone(), frame.len() == start),
        (json!(true), true)
    );
    reqs.on_put_reply(
        head["req"].as_i64().unwrap(),
        &Value::Null,
        "over its quota",
    );
    assert_eq!(up.await.unwrap().unwrap_err(), "over its quota");
    assert!(Path::new(&empty).exists());
}

#[tokio::test]
async fn a_dropped_tunnel_fails_every_request_in_flight() {
    let (reqs, mut sent, link) = wired();
    let dir = Scratch::new("files");
    let (r1, r2, local) = (reqs.clone(), reqs.clone(), dir.path().join("a.wav"));
    let stat = tokio::spawn(async move { r1.stat(1, vec!["/a".into()]).await });
    let get = tokio::spawn(async move { r2.download(1, &remote("/a", 1.0), &local).await });
    text(&mut sent).await;
    text(&mut sent).await;
    reqs.fail_all();
    assert_eq!(stat.await.unwrap().unwrap(), []);
    assert_eq!(
        get.await.unwrap().unwrap_err().to_string(),
        "tunnel dropped"
    );
    link.detach();
    assert_eq!(reqs.stat(1, vec![]).await.unwrap_err(), "tunnel is down");
}
