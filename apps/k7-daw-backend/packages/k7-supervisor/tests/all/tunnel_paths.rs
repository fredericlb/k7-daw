//! tunnel_paths.rs — the path map held to its roots: a server path never resolves outside the local copy whatever
//! its segments hold, a local path goes up only from under a writable root, and a file moves BEFORE the frame
//! naming it. The hub is a table of files in memory; the files it moves are real, in a scratch folder.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use k7_supervisor::tunnel::files::{BoxFut, FileError, RemoteFiles, RemoteStat};
use k7_supervisor::tunnel::filetime;
use k7_supervisor::tunnel::ledger::SyncLedger;
use k7_supervisor::tunnel::path_map::{MappedRoot, PathMap};
use k7_supervisor::tunnel::segments::{join_under, relative_under, safe_segment};
use serde_json::json;

use crate::scratch::Scratch;

const SEP: char = std::path::MAIN_SEPARATOR;

fn under(root: &str, rel: &str) -> String {
    PathBuf::from(root).join(rel).to_string_lossy().into_owned()
}

#[test]
fn segments_that_could_climb_are_refused() {
    assert!(safe_segment("clip.wav") && safe_segment(".hidden"));
    for bad in [
        "",
        ".",
        "..",
        "...",
        ".. ",
        "..\\..\\x",
        "C:",
        "C:\\x",
        "\\Windows",
        "a/b",
        "a\tb",
        "a\0b",
    ] {
        assert!(!safe_segment(bad), "{bad:?}");
    }
}

#[test]
fn windows_device_names_are_refused_everywhere() {
    for bad in [
        "CON",
        "con",
        "Prn.txt",
        "aux.wav",
        "NUL",
        "nul.tar.gz",
        "COM1",
        "com9.log",
        "LPT3",
        "lpt0",
        "CONIN$",
        "COM\u{b9}",
        "AUX .wav",
    ] {
        assert!(!safe_segment(bad), "{bad:?}");
    }
    for ok in [
        "CONSOLE",
        "console.wav",
        "COM10",
        "LPT",
        "comic.wav",
        "nullable",
        "aux1.wav",
        "my-con.wav",
    ] {
        assert!(safe_segment(ok), "{ok:?}");
    }
    assert!(join_under("/r", "songs/nul.wav").is_none());
}

#[test]
fn a_server_path_stays_under_its_root() {
    let root = "/tmp/k7-pathmap/ws";
    assert_eq!(
        join_under(root, "songs/clip.wav"),
        Some(under(root, "songs/clip.wav"))
    );
    for bad in [
        "../x",
        "a/../../x",
        "a/./b",
        "a//b",
        "",
        "a/",
        "..\\..\\x",
        "C:\\x",
        "\\Windows\\x",
        "C:x",
    ] {
        assert!(join_under(root, bad).is_none(), "{bad:?}");
    }
}

#[test]
fn a_local_path_is_sent_only_from_under_its_root() {
    let root = "/tmp/k7-pathmap/ws";
    assert_eq!(relative_under(root, root), Some(String::new()));
    assert_eq!(
        relative_under(root, &under(root, "songs/clip.wav")).as_deref(),
        Some("songs/clip.wav")
    );
    for bad in [
        format!("{root}{SEP}.{SEP}..{SEP}..{SEP}etc"),
        format!("{root}{SEP}a{SEP}..{SEP}..{SEP}x"),
        format!("{root}x{SEP}y"),
        format!("{root}{SEP}{SEP}a"),
        format!("{root}{SEP}"),
    ] {
        assert!(relative_under(root, &bad).is_none(), "{bad:?}");
    }
}

#[derive(Default)]
struct Hub {
    tree: Mutex<BTreeMap<String, (Vec<u8>, f64)>>,
    stats: AtomicUsize,
    downloads: Mutex<Vec<String>>,
    uploads: Mutex<Vec<String>>,
    /// A target something holds open: every download then lands beside it.
    held: Mutex<Vec<PathBuf>>,
}

struct Files(Arc<Hub>);

impl RemoteFiles for Files {
    fn stat(&self, paths: Vec<String>) -> BoxFut<'_, Result<Vec<RemoteStat>, String>> {
        self.0.stats.fetch_add(1, Ordering::SeqCst);
        let tree = self.0.tree.lock().unwrap();
        let out = paths
            .into_iter()
            .map(|p| match tree.get(&p) {
                Some((b, m)) => RemoteStat {
                    path: p,
                    kind: "file".into(),
                    size: Some(b.len() as u64),
                    mtime_ms: Some(*m),
                },
                None => RemoteStat {
                    path: p,
                    kind: "missing".into(),
                    size: None,
                    mtime_ms: None,
                },
            })
            .collect();
        Box::pin(async move { Ok(out) })
    }

    fn download(&self, remote: RemoteStat, local: PathBuf) -> BoxFut<'_, Result<(), FileError>> {
        Box::pin(async move {
            self.0.downloads.lock().unwrap().push(remote.path.clone());
            let bytes = self.0.tree.lock().unwrap()[&remote.path].0.clone();
            std::fs::create_dir_all(local.parent().unwrap()).unwrap();
            let part = local.with_extension("part");
            std::fs::write(&part, bytes).unwrap();
            if self.0.held.lock().unwrap().contains(&local) {
                return Err(FileError::InUse("held open".into(), part));
            }
            std::fs::rename(&part, &local).unwrap();
            filetime::set_mtime_ms(&local, remote.mtime_ms.unwrap_or(0.0));
            Ok(())
        })
    }

    fn upload(&self, local: PathBuf, remote: String) -> BoxFut<'_, Result<RemoteStat, String>> {
        Box::pin(async move {
            self.0.uploads.lock().unwrap().push(remote.clone());
            let bytes = std::fs::read(local).map_err(|e| e.to_string())?;
            let size = bytes.len() as u64;
            self.0
                .tree
                .lock()
                .unwrap()
                .insert(remote.clone(), (bytes, 1_700_000_000_000.0));
            Ok(RemoteStat {
                path: remote,
                kind: "file".into(),
                size: Some(size),
                mtime_ms: Some(1_700_000_000_000.0),
            })
        })
    }
}

struct Bed {
    hub: Arc<Hub>,
    dir: Scratch,
    map: PathMap,
    said: Arc<Mutex<Vec<String>>>,
}

fn bed_over(hub: Arc<Hub>, dir: Scratch, ledger: Arc<SyncLedger>, writable: bool) -> Bed {
    let said = Arc::<Mutex<Vec<String>>>::default();
    let sink = said.clone();
    let root = MappedRoot {
        remote: "/srv/ws".into(),
        local: dir.path().join("ws").to_string_lossy().into_owned(),
        writable,
    };
    let map = PathMap::new(
        vec![root],
        Box::new(Files(hub.clone())),
        ledger,
        Arc::new(move |m| sink.lock().unwrap().push(m.to_string())),
    );
    Bed {
        hub,
        dir,
        map,
        said,
    }
}

fn bed(writable: bool) -> Bed {
    bed_over(
        Arc::default(),
        Scratch::new("pathmap"),
        Arc::default(),
        writable,
    )
}

impl Bed {
    fn local(&self, rel: &str) -> String {
        self.dir
            .path()
            .join("ws")
            .join(rel)
            .to_string_lossy()
            .into_owned()
    }

    fn serve(&self, remote: &str, bytes: &[u8], mtime: f64) {
        self.hub
            .tree
            .lock()
            .unwrap()
            .insert(remote.into(), (bytes.to_vec(), mtime));
    }
}

#[test]
fn the_map_refuses_both_ways() {
    let b = bed(true);
    assert_eq!(b.map.to_local("/srv/ws/a/b.wav"), Some(b.local("a/b.wav")));
    assert!(b.map.to_local("/srv/ws/a/../../x").is_none());
    assert!(b.map.to_local("/srv/ws/..\\..\\x").is_none());
    assert!(b.map.to_local("/srv/other/a").is_none());
    assert_eq!(
        b.map.to_remote(&b.local("a/b.wav")).map(|m| m.0).as_deref(),
        Some("/srv/ws/a/b.wav")
    );
    assert!(
        b.map
            .to_remote(&format!("{}{SEP}.{SEP}..{SEP}..{SEP}home", b.local("")))
            .is_none()
    );
}

#[tokio::test]
async fn only_a_file_not_yet_here_is_asked_for_and_a_delta_trusts_the_document() {
    let b = bed(true);
    b.serve("/srv/ws/a.wav", b"abc", 1_600_000_000_000.0);
    let doc = json!({"type": "project", "clip": "/srv/ws/a.wav"}).to_string();
    let out: serde_json::Value =
        serde_json::from_str(&b.map.outbound(&doc, true).await.unwrap()).unwrap();
    assert_eq!(out["clip"], b.local("a.wav"));
    assert_eq!(std::fs::read(b.local("a.wav")).unwrap(), b"abc");
    let delta = json!({"type": "set", "clip": "/srv/ws/a.wav"}).to_string();
    b.map.outbound(&delta, true).await.unwrap();
    assert_eq!(
        b.hub.stats.load(Ordering::SeqCst),
        1,
        "a delta asks nothing"
    );
    b.map.outbound(&doc, true).await.unwrap();
    assert_eq!(b.hub.stats.load(Ordering::SeqCst), 2);
    assert_eq!(
        b.hub.downloads.lock().unwrap().len(),
        1,
        "a current file is not fetched again"
    );
}

#[tokio::test]
async fn a_frame_with_no_path_in_it_passes_untouched() {
    let b = bed(true);
    for text in [
        "not json /srv/ws/x",
        r#"{"type":"state","a":"/srv/ws/x"}"#,
        r#"{"type":"io-levels","a":"/srv/ws/x"}"#,
        r#"{"type":"x","a":"elsewhere"}"#,
    ] {
        assert_eq!(b.map.outbound(text, true).await.unwrap(), text);
    }
    assert_eq!(b.hub.stats.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_file_goes_up_only_from_a_writable_root_and_only_when_it_changed() {
    let b = bed(true);
    std::fs::create_dir_all(b.local("")).unwrap();
    std::fs::write(b.local("take.wav"), b"audio").unwrap();
    let said =
        json!({"type": "rec", "file": b.local("take.wav"), "raw": {"file": b.local("take.wav")}})
            .to_string();
    let sent: serde_json::Value = serde_json::from_str(&b.map.inbound(&said).await).unwrap();
    assert_eq!(sent["file"], "/srv/ws/take.wav");
    assert_eq!(
        sent["raw"]["file"],
        b.local("take.wav"),
        "`raw` stays the machine's own spelling"
    );
    assert_eq!(sent.as_object().unwrap().keys().next_back().unwrap(), "raw");
    b.map.inbound(&said).await;
    assert_eq!(b.hub.uploads.lock().unwrap().len(), 1);
    let read_only = bed(false);
    std::fs::create_dir_all(read_only.local("")).unwrap();
    std::fs::write(read_only.local("take.wav"), b"audio").unwrap();
    let kept = read_only
        .map
        .inbound(&json!({"file": read_only.local("take.wav")}).to_string())
        .await;
    assert!(kept.contains("/srv/ws/take.wav") && read_only.hub.uploads.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_held_file_gets_its_new_version_beside_it_and_frames_name_that() {
    let b = bed(true);
    b.serve("/srv/ws/clip.wav", b"one", 1_600_000_000_000.0);
    let doc = json!({"type": "project", "clip": "/srv/ws/clip.wav"}).to_string();
    b.map.outbound(&doc, true).await.unwrap();
    b.serve("/srv/ws/clip.wav", b"version two", 1_600_000_005_000.0);
    b.hub
        .held
        .lock()
        .unwrap()
        .push(PathBuf::from(b.local("clip.wav")));
    let out: serde_json::Value =
        serde_json::from_str(&b.map.outbound(&doc, true).await.unwrap()).unwrap();
    let beside = b.local("clip~1600000005000.wav");
    assert_eq!(out["clip"], beside);
    assert_eq!(std::fs::read(&beside).unwrap(), b"version two");
    assert_eq!(
        std::fs::read(b.local("clip.wav")).unwrap(),
        b"one",
        "the held file is left alone"
    );
    assert_eq!(
        b.map.to_remote(&beside).map(|m| m.0).as_deref(),
        Some("/srv/ws/clip.wav")
    );
    assert!(
        b.said
            .lock()
            .unwrap()
            .iter()
            .any(|l| l.contains("is held open"))
    );
    // After a restart the version beside the file is adopted, not fetched again.
    let reroot = MappedRoot {
        remote: "/srv/ws".into(),
        local: b.local(""),
        writable: true,
    };
    let map = PathMap::new(
        vec![reroot],
        Box::new(Files(b.hub.clone())),
        Arc::default(),
        Arc::new(|_| {}),
    );
    let downloads = b.hub.downloads.lock().unwrap().len();
    let out: serde_json::Value =
        serde_json::from_str(&map.outbound(&doc, true).await.unwrap()).unwrap();
    assert_eq!(out["clip"], beside);
    assert_eq!(b.hub.downloads.lock().unwrap().len(), downloads);
}

#[tokio::test]
async fn a_sync_counts_only_the_files_it_had_to_copy() {
    let b = bed(true);
    b.serve("/srv/ws/old.wav", b"old", 1_500_000_000_000.0);
    b.serve("/srv/ws/new.wav", b"new!", 1_500_000_000_000.0);
    b.map.ensure_local(&b.local("old.wav")).await.unwrap();
    let reports = Mutex::new(Vec::new());
    let locals = ["old.wav", "new.wav", "none.wav"].map(|n| b.local(n));
    let done = b
        .map
        .sync(&locals, &|p| reports.lock().unwrap().push(p.clone()))
        .await
        .unwrap();
    assert_eq!(
        (done.files, done.files_done, done.bytes, done.bytes_done),
        (1, 1, 4, 4)
    );
    assert!(done.failures.is_empty());
    assert_eq!(reports.lock().unwrap().first().unwrap().files_done, 0);
    assert_eq!(std::fs::read(b.local("new.wav")).unwrap(), b"new!");
    assert!(!b.map.ensure_local(&b.local("none.wav")).await.unwrap());
}

#[tokio::test]
async fn a_second_asker_waits_for_the_download_in_flight() {
    let ledger = Arc::new(SyncLedger::default());
    let ran = Arc::new(AtomicUsize::new(0));
    let one = |ledger: Arc<SyncLedger>, ran: Arc<AtomicUsize>| async move {
        ledger
            .download_once("/x", async {
                ran.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                Err("disk full".to_string())
            })
            .await
    };
    let (a, b) = tokio::join!(
        one(ledger.clone(), ran.clone()),
        one(ledger.clone(), ran.clone())
    );
    assert_eq!(
        (a, b),
        (Err("disk full".to_string()), Err("disk full".to_string()))
    );
    assert_eq!(ran.load(Ordering::SeqCst), 1);
    // The entry is gone with it: the next ask runs afresh.
    assert!(ledger.download_once("/x", async { Ok(()) }).await.is_ok());
}
