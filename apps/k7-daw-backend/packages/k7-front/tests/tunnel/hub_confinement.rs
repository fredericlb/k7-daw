//! hub_confinement.rs — the hub's files attacked by a bare agent sending what no real agent would: dot segments,
//! encoded and foreign separators, a prefix of the root, links out of the tree, Windows device names and stream
//! suffixes, and uploads that stop or collide. Unix only: the tree is made of links Windows will not give a test.

#![cfg(unix)]

use std::os::unix::fs::symlink;

use serde_json::json;

use crate::bed::{Bare, Client, Dir, Hub, HubOptions, until};

/// The tree the attacks aim at: a root, a secret beside it, links out, a library whose clone is a link.
struct Tree {
    base: Dir,
    r: String,
    l: String,
}

fn tree() -> Tree {
    let base = Dir::new("confine");
    let r = base.join("ws");
    std::fs::create_dir_all(r.join("song")).unwrap();
    std::fs::write(r.join("song/a.wav"), "RIFF-a").unwrap();
    std::fs::create_dir(base.join("outside")).unwrap();
    std::fs::write(base.join("outside/secret"), "SECRET").unwrap();
    std::fs::create_dir(base.join("wsx")).unwrap();
    std::fs::write(base.join("wsx/secret"), "SECRET").unwrap();
    symlink(base.join("outside"), r.join("escape")).unwrap();
    std::fs::create_dir_all(base.join("checkout/wavetables")).unwrap();
    std::fs::write(base.join("checkout/wavetables/basic.wav"), "RIFF-b").unwrap();
    symlink(base.join("outside"), base.join("checkout/escape")).unwrap();
    std::fs::create_dir(base.join("edits")).unwrap();
    symlink(base.join("checkout"), base.join("edits/k7")).unwrap();
    let (r, l) = (
        r.to_string_lossy().into_owned(),
        base.join("edits").to_string_lossy().into_owned(),
    );
    Tree { base, r, l }
}

/// A hub with a bare agent dialled in, and a channel of `roots` open on it.
async fn channel_on(hub: &Hub, roots: serde_json::Value) -> (Bare, Client, i64) {
    let mut agent = Bare::dial(hub.relay.port, "e1", "t").await;
    hub.until_has("e1").await;
    let path = hub.bind_local("e1", roots, "engine", false).await;
    let client = Client::dial(hub.port(), &path, None).await.unwrap();
    let ch = agent.opened().await;
    (agent, client, ch)
}

struct Rig {
    t: Tree,
    hub: Hub,
    agent: Bare,
    _client: Client,
    ch: i64,
}

async fn rig() -> Rig {
    let t = tree();
    let hub = Hub::start(HubOptions::default()).await;
    let roots = json!([
        { "remote": t.r, "name": "ws", "writable": true },
        { "remote": t.l, "name": "edits", "writable": false, "perChild": true },
    ]);
    let (agent, client, ch) = channel_on(&hub, roots).await;
    Rig {
        t,
        hub,
        agent,
        _client: client,
        ch,
    }
}

impl Rig {
    /// Paths no channel may reach.
    fn attacks(&self) -> Vec<String> {
        let (r, l, b) = (&self.t.r, &self.t.l, self.t.base.s());
        vec![
            format!("{r}/../outside/secret"),
            format!("{r}/./song/a.wav"),
            format!("{r}/song/../../outside/secret"),
            format!("{b}/wsx/secret"),
            "song/a.wav".into(),
            String::new(),
            format!("{r}/escape/secret"),
            format!("{r}/..%2Foutside%2Fsecret"),
            format!("{r}/%2e%2e/outside/secret"),
            format!("{r}/..\\..\\outside\\secret"),
            format!("{r}/CON"),
            format!("{r}/nul.wav"),
            format!("{r}/COM1.txt"),
            format!("{r}/AUX .wav"),
            format!("{r}/CONIN$"),
            format!("{r}/lpt¹"),
            format!("{r}/song/a\tb.wav"),
            format!("{r}/song/a.wav:stream"),
            format!("{l}/k7/escape/secret"),
            format!("{r}//song/a.wav"),
        ]
    }
}

#[tokio::test]
async fn stats_nothing_outside_the_roots_and_what_is_inside() {
    let mut r = rig().await;
    let legit = vec![
        r.t.r.clone(),
        format!("{}/song/a.wav", r.t.r),
        format!("{}/k7/wavetables/basic.wav", r.t.l),
    ];
    let mut asked = legit.clone();
    asked.extend(r.attacks());
    let got = r.agent.stat(r.ch, &asked).await;
    let kinds: Vec<&str> = got[..3]
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["dir", "file", "file"]);
    for e in &got[3..] {
        assert_eq!(e["kind"], "missing", "{}", e["path"]);
    }
}

#[tokio::test]
async fn serves_no_byte_outside_the_roots_and_a_file_inside_whole() {
    let mut r = rig().await;
    let ok = r.agent.get(r.ch, &format!("{}/song/a.wav", r.t.r)).await;
    assert_eq!(ok.unwrap(), b"RIFF-a");
    for p in r.attacks() {
        assert!(r.agent.get(r.ch, &p).await.is_err(), "{p}");
    }
}

#[tokio::test]
async fn takes_no_upload_outside_a_writable_root_nor_one_beside_a_link_out() {
    let mut r = rig().await;
    let ch = r.ch;
    let (ok, _) = r
        .agent
        .put(ch, &format!("{}/takes/t1.wav", r.t.r), &[b"TAKE"])
        .await;
    assert_eq!(ok, None);
    assert_eq!(
        std::fs::read_to_string(format!("{}/takes/t1.wav", r.t.r)).unwrap(),
        "TAKE"
    );
    // An encoded separator is a name like any other: it lands inside the root, under that name.
    let (encoded, _) = r
        .agent
        .put(ch, &format!("{}/..%2Foutside%2Fx", r.t.r), &[b"E"])
        .await;
    assert_eq!(encoded, None);
    assert_eq!(
        std::fs::read_to_string(format!("{}/..%2Foutside%2Fx", r.t.r)).unwrap(),
        "E"
    );
    let mut refused: Vec<String> = r
        .attacks()
        .into_iter()
        .filter(|p| !p.contains('%'))
        .collect();
    refused.push(format!("{}/escape/planted", r.t.r));
    refused.push(format!("{}/k7/wavetables/new.wav", r.t.l));
    refused.push(format!("{}/elsewhere.wav", r.t.base.s()));
    for p in refused {
        let (err, _) = r.agent.put(ch, &p, &[b"X"]).await;
        assert!(err.is_some(), "{p}");
    }
    let outside: Vec<String> = std::fs::read_dir(r.t.base.join("outside"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(outside, ["secret"]);
    assert_eq!(
        std::fs::read_to_string(r.t.base.join("outside/secret")).unwrap(),
        "SECRET"
    );
    let takes: Vec<String> = std::fs::read_dir(format!("{}/takes", r.t.r))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(takes, ["t1.wav"]);
}

fn parts(dir: &str) -> Vec<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|f| f.ends_with(".part"))
        .collect()
}

#[tokio::test]
async fn leaves_no_part_file_and_no_target_when_the_tunnel_drops_mid_upload() {
    let r = rig().await;
    let song = format!("{}/song", r.t.r);
    let target = format!("{song}/half.wav");
    r.agent.put_chunk(r.ch, 7, &target, b"half", false);
    until(|| !parts(&song).is_empty(), 10_000).await;
    r.agent.close();
    until(|| parts(&song).is_empty(), 10_000).await;
    assert!(!std::path::Path::new(&target).exists());
    drop(r.hub);
}

#[tokio::test]
async fn keeps_two_uploads_of_one_path_apart_each_in_a_part_of_its_own() {
    let r = rig().await;
    let song = format!("{}/song", r.t.r);
    let target = format!("{song}/twice.wav");
    r.agent.put_chunk(r.ch, 1, &target, b"AAAA", false);
    r.agent.put_chunk(r.ch, 2, &target, b"BBBB", false);
    r.agent.put_chunk(r.ch, 1, &target, b"AAAA", true);
    r.agent.put_chunk(r.ch, 2, &target, b"BBBB", true);
    until(|| r.agent.replies("put-reply").len() == 2, 10_000).await;
    // One may lose the rename on Windows while the other holds the target: never both, never a mix.
    assert!(
        r.agent
            .replies("put-reply")
            .iter()
            .any(|p| p["error"].is_null())
    );
    let body = std::fs::read_to_string(&target).unwrap();
    assert!(body == "AAAAAAAA" || body == "BBBBBBBB", "{body}");
    assert!(parts(&song).is_empty());
}
