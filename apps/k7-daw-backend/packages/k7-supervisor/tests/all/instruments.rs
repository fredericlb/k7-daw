//! instruments.rs — what an instrument's download does, with archives served on 127.0.0.1: a zip unpacked whole
//! with GitHub's one top folder dropped, a name climbing out of the folder refused, a redirect followed, a cut
//! download resumed (and every way a server may answer a resume), a wrong hash refused with nothing kept, and
//! the removal that takes what a cut install left.

use std::io::{Cursor, Write};
use std::path::Path;
use std::time::Duration;

use k7_supervisor::instruments::{self, Instrument, Phase, Progress};
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;

use crate::blob_server::{Answer, Asked, serve_blobs};
use crate::scratch::Scratch;

const WAIT: Duration = Duration::from_secs(5);

fn zip_of(files: &[(&str, &str)]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, body) in files {
        zip.start_file(*name, opts).unwrap();
        zip.write_all(body.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn instrument(name: &str, url: String, archive: &[u8]) -> Instrument {
    Instrument {
        name: name.into(),
        label: name.into(),
        url,
        sha256: hex(archive),
        bytes: archive.len() as u64,
    }
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_default()
}

async fn fetch(root: &Path, i: &Instrument) -> (Result<(), String>, Vec<Progress>) {
    let mut seen = Vec::new();
    let sink = std::sync::Mutex::new(&mut seen);
    let res = instruments::install(root, i, WAIT, |p| sink.lock().unwrap().push(p)).await;
    (res, seen)
}

/// A piano of a few files under one top folder, big enough to cut.
fn piano() -> Vec<u8> {
    let big = "x".repeat(4000);
    zip_of(&[
        ("Piano-abc/Piano.sfz", "<region> sample=Samples/A0.flac"),
        ("Piano-abc/Samples/A0.flac", &big),
    ])
}

#[test]
fn a_github_archive_is_unpacked_without_its_top_folder() {
    let dir = Scratch::new("ins-github");
    let zip = dir.path().join("a.zip");
    std::fs::write(&zip, piano()).unwrap();
    instruments::unpack_zip(&zip, &dir.path().join("piano")).unwrap();
    assert!(read(&dir.path().join("piano/Piano.sfz")).contains("A0.flac"));
    assert_eq!(read(&dir.path().join("piano/Samples/A0.flac")).len(), 4000);
    assert!(!dir.path().join("piano.unpack").exists());
}

#[test]
fn a_flat_archive_keeps_its_layout() {
    let dir = Scratch::new("ins-flat");
    let zip = dir.path().join("a.zip");
    std::fs::write(
        &zip,
        zip_of(&[("Piano.sfz", "x"), ("Samples/A0.flac", "y")]),
    )
    .unwrap();
    instruments::unpack_zip(&zip, &dir.path().join("piano")).unwrap();
    assert!(dir.path().join("piano/Piano.sfz").is_file());
    assert!(dir.path().join("piano/Samples/A0.flac").is_file());
}

#[test]
fn a_name_outside_the_folder_is_refused_and_nothing_is_left() {
    let dir = Scratch::new("ins-escape");
    for evil in ["../evil.txt", "/abs/evil.txt", "a/../../evil.txt"] {
        let zip = dir.path().join("a.zip");
        std::fs::write(&zip, zip_of(&[("ok.sfz", "x"), (evil, "y")])).unwrap();
        let err = instruments::unpack_zip(&zip, &dir.path().join("piano")).unwrap_err();
        assert!(err.contains("outside"), "{evil}: {err}");
        assert!(!dir.path().join("evil.txt").exists());
        assert!(!dir.path().join("piano").exists());
        assert!(!dir.path().join("piano.unpack").exists());
    }
}

#[test]
fn the_hash_is_the_files_sha256() {
    let dir = Scratch::new("ins-hash");
    std::fs::write(dir.path().join("abc"), "abc").unwrap();
    assert_eq!(
        instruments::sha256_of(&dir.path().join("abc")),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert!(instruments::sha256_of(&dir.path().join("missing")).is_empty());
}

#[test]
fn every_instrument_is_pinned() {
    for i in instruments::all() {
        assert_eq!(i.sha256.len(), 64, "{}", i.name);
        assert!(i.url.starts_with("https://"), "{}", i.name);
        assert!(i.bytes > 0);
    }
    assert!(instruments::find("nbo-wurt").is_some());
    assert!(instruments::find("no-such-instrument").is_none());
}

#[test]
fn a_removal_takes_the_folder_and_what_a_cut_install_left() {
    let dir = Scratch::new("ins-remove");
    let root = dir.path();
    let wurt = instruments::find("nbo-wurt").unwrap();
    assert!(!instruments::present(root, wurt));
    std::fs::create_dir_all(root.join("nbo-wurt")).unwrap();
    std::fs::write(root.join("nbo-wurt/.k7-archive"), "an-older-hash\n").unwrap();
    std::fs::write(root.join("nbo-wurt.zip.part"), "half").unwrap();
    std::fs::create_dir_all(root.join("nbo-wurt.unpack")).unwrap();
    // An older archive's marker is still an install a first boot keeps, and is not the pinned one.
    assert!(instruments::present(root, wurt));
    assert!(!instruments::installed(root, wurt));
    instruments::remove(root, wurt).unwrap();
    for left in ["nbo-wurt", "nbo-wurt.zip.part", "nbo-wurt.unpack"] {
        assert!(!root.join(left).exists(), "{left}");
    }
    assert!(!instruments::present(root, wurt));
    // Nothing to remove is not a failure.
    instruments::remove(root, wurt).unwrap();
}

#[tokio::test]
async fn an_archive_is_downloaded_verified_unpacked_and_marked() {
    let dir = Scratch::new("ins-install");
    let archive = piano();
    let body = archive.clone();
    let server = serve_blobs(move |_| Answer::ok(body.clone())).await;
    let i = instrument("piano", format!("{}/piano.zip", server.origin), &archive);
    let (res, progress) = fetch(dir.path(), &i).await;
    res.unwrap();
    assert!(instruments::installed(dir.path(), &i));
    assert!(read(&dir.path().join("piano/Piano.sfz")).contains("A0.flac"));
    assert_eq!(
        read(&dir.path().join("piano/.k7-archive")),
        format!("{}\n", i.sha256)
    );
    assert!(!dir.path().join("piano.zip.part").exists());
    // The total is the pinned size, and the last word is that it was unpacking.
    assert!(
        progress
            .iter()
            .all(|p| p.total == i.bytes && p.received <= i.bytes)
    );
    assert_eq!(progress.last().unwrap().phase, Phase::Unpacking);
    assert!(progress.iter().any(|p| p.phase == Phase::Downloading));
}

#[tokio::test]
async fn a_redirect_is_followed_and_a_loop_is_not() {
    let dir = Scratch::new("ins-redirect");
    let archive = piano();
    let body = archive.clone();
    let server = serve_blobs(move |a| match a.path.as_str() {
        "/old.zip" => Answer::redirect("/mid.zip"),
        "/mid.zip" => Answer::redirect("/new.zip"),
        "/new.zip" => Answer::ok(body.clone()),
        _ => Answer::redirect("/loop"),
    })
    .await;
    let i = instrument("piano", format!("{}/old.zip", server.origin), &archive);
    fetch(dir.path(), &i).await.0.unwrap();
    assert!(instruments::installed(dir.path(), &i));
    let looping = instrument("loop", format!("{}/loop", server.origin), &archive);
    let err = fetch(dir.path(), &looping).await.0.unwrap_err();
    assert!(err.contains("redirected more than"), "{err}");
    assert!(!dir.path().join("loop").exists());
}

/// A server whose answer to a `Range` request depends on the path, as each kind of server does.
async fn resuming(archive: &[u8]) -> crate::blob_server::Blobs {
    let body = archive.to_vec();
    serve_blobs(move |a: &Asked| {
        let n = body.len();
        match (a.path.as_str(), &a.range) {
            ("/honours.zip", Some(r)) => {
                let from: usize = r
                    .trim_start_matches("bytes=")
                    .trim_end_matches('-')
                    .parse()
                    .unwrap();
                Answer::partial(&body, from)
            }
            // A server that cannot resume sends it whole.
            ("/ignores.zip", _) => Answer::ok(body.clone()),
            ("/whole.zip", Some(_)) => {
                Answer::status(416).header("Content-Range", &format!("bytes */{n}"))
            }
            ("/elsewhere.zip", Some(_)) => Answer::partial(&body, 7),
            ("/lost.zip", Some(_)) => Answer::status(416).header("Content-Range", "bytes */5"),
            _ => Answer::ok(body.clone()),
        }
    })
    .await
}

#[tokio::test]
async fn a_cut_download_is_resumed_from_its_part() {
    let archive = piano();
    let server = resuming(&archive).await;
    for kind in ["honours", "ignores", "whole", "elsewhere", "lost"] {
        let dir = Scratch::new("ins-resume");
        let i = instrument("piano", format!("{}/{kind}.zip", server.origin), &archive);
        let cut = if kind == "whole" { archive.len() } else { 1500 };
        std::fs::write(dir.path().join("piano.zip.part"), &archive[..cut]).unwrap();
        let (res, progress) = fetch(dir.path(), &i).await;
        res.unwrap_or_else(|e| panic!("{kind}: {e}"));
        assert!(instruments::installed(dir.path(), &i), "{kind}");
        assert!(!dir.path().join("piano.zip.part").exists(), "{kind}");
        let asked = server.to(&format!("/{kind}.zip"));
        assert_eq!(asked[0].range, Some(format!("bytes={cut}-")), "{kind}");
        // A prefix the server cannot use costs one more request without a range, never a loop.
        match kind {
            "honours" => assert_eq!((asked.len(), progress[0].received), (1, 1500)),
            "whole" => assert_eq!(asked.len(), 1),
            _ => assert_eq!(asked.len(), if kind == "ignores" { 1 } else { 2 }, "{kind}"),
        }
        if kind == "elsewhere" || kind == "lost" {
            assert_eq!(asked[1].range, None, "{kind}");
        }
    }
}

#[tokio::test]
async fn a_wrong_hash_is_refused_and_nothing_is_kept() {
    let dir = Scratch::new("ins-hash-refused");
    let archive = piano();
    let other = zip_of(&[("Other.sfz", "changed upstream")]);
    let body = other.clone();
    let server = serve_blobs(move |_| Answer::ok(body.clone())).await;
    let i = instrument("piano", format!("{}/piano.zip", server.origin), &archive);
    let err = fetch(dir.path(), &i).await.0.unwrap_err();
    assert!(
        err.contains("changed upstream") && err.contains(&hex(&other)),
        "{err}"
    );
    for left in ["piano", "piano.zip.part", "piano.unpack"] {
        assert!(!dir.path().join(left).exists(), "{left}");
    }
    assert!(!instruments::present(dir.path(), &i));
}

#[tokio::test]
async fn a_resumed_archive_that_does_not_match_is_dropped_whole() {
    let dir = Scratch::new("ins-resume-bad");
    let archive = piano();
    let server = resuming(&archive).await;
    let i = instrument("piano", format!("{}/honours.zip", server.origin), &archive);
    // A prefix that is not this archive's: the whole of it fails the pin.
    std::fs::write(dir.path().join("piano.zip.part"), vec![9u8; 1500]).unwrap();
    let err = fetch(dir.path(), &i).await.0.unwrap_err();
    assert!(err.contains("resumed") && err.contains("whole"), "{err}");
    assert!(!dir.path().join("piano.zip.part").exists());
    assert!(!dir.path().join("piano").exists());
    // The next try has no part and takes it whole.
    let server = serve_blobs({
        let body = archive.clone();
        move |_| Answer::ok(body.clone())
    })
    .await;
    let again = instrument("piano", format!("{}/p.zip", server.origin), &archive);
    fetch(dir.path(), &again).await.0.unwrap();
    assert!(instruments::installed(dir.path(), &again));
}

#[tokio::test]
async fn an_error_page_never_becomes_the_part_and_a_bad_zip_is_kept() {
    let dir = Scratch::new("ins-errors");
    let archive = piano();
    let junk = b"not a zip at all".to_vec();
    let body = junk.clone();
    let server = serve_blobs(move |a| match a.path.as_str() {
        "/junk.zip" => Answer::ok(body.clone()),
        _ => Answer::status(404),
    })
    .await;
    let missing = instrument("piano", format!("{}/gone.zip", server.origin), &archive);
    let err = fetch(dir.path(), &missing).await.0.unwrap_err();
    assert!(err.contains("answered 404"), "{err}");
    assert!(!dir.path().join("piano.zip.part").exists());
    // Verified but not a zip: the archive stays, so the next try fetches nothing.
    let bad = instrument("junk", format!("{}/junk.zip", server.origin), &junk);
    let err = fetch(dir.path(), &bad).await.0.unwrap_err();
    assert!(err.contains("not a zip"), "{err}");
    assert!(dir.path().join("junk.zip.part").is_file());
    assert!(!dir.path().join("junk").exists());
}
