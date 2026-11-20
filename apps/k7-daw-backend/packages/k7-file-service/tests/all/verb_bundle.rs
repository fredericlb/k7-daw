//! The verb bundle (3d): a project's texts, the workspace's shared texts, binaries as length and seconds, listings, libraries by hash.

use crate::support;

use k7_file_service::verb_bundle::{Inputs, build, library_files, library_roots};
use k7_file_service::verb_paths::Virtual;
use serde_json::{Value, json};
use song::roots::{Library, Roots};
use support::entries::Fx;

/// A 16-bit mono 8 kHz WAV of `n` samples.
fn wav(n: u32) -> Vec<u8> {
    let data = n * 2;
    let mut b = b"RIFF".to_vec();
    b.extend((36 + data).to_le_bytes());
    b.extend(b"WAVEfmt ");
    b.extend(16u32.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(8000u32.to_le_bytes());
    b.extend(16000u32.to_le_bytes());
    b.extend(2u16.to_le_bytes());
    b.extend(16u16.to_le_bytes());
    b.extend(b"data");
    b.extend(data.to_le_bytes());
    b.extend(vec![0u8; data as usize]);
    b
}

fn made(fx: &Fx) -> Value {
    made_with(fx, fx.s.path("studio/models"))
}

fn made_with(fx: &Fx, models: String) -> Value {
    let lib = fx.lib("");
    let roots = Roots {
        project: fx.at("song.k7"),
        workspace: Some(fx.root.clone()),
        confine: Some(fx.root.clone()),
        libraries: vec![Library {
            name: "k7-core".into(),
            root: lib.trim_end_matches('/').to_string(),
            editable: false,
        }],
        meta: Some(fx.s.path("code/packages/song/meta")),
        scan: Some(fx.s.path("studio/exts.json")),
        models: Some(models),
        sample_banks: vec!["/srv/banks".into()],
        also_readable: vec![fx.s.path("libraries/k7-core"), fx.s.path("elsewhere/real")],
        ..Roots::default()
    };
    build(&Inputs {
        home: &fx.s.home(),
        root: &fx.root,
        project: &fx.at("song.k7"),
        at: "song.k7",
        rev: "r1",
        roots,
        rig: json!({}),
        code_root: &fx.s.path("code"),
    })
    .expect("built")
}

#[test]
fn the_bundle_holds_texts_lengths_listings_shared_texts_and_named_libraries() {
    let fx = Fx::new("vb-shape");
    std::fs::write(fx.at("song.k7/take.wav"), wav(8000)).unwrap();
    fx.s.file("studio/workspace/songs/song.k7/.hidden/x.txt", "secret");
    fx.s.file("studio/workspace/songs/_playlists/set.k7x", "set(1)\n");
    fx.s.file(
        "studio/workspace/songs/_modules/audio/m/module.k7x",
        "m(1)\n",
    );
    fx.s.file("studio/preferences.k7x", "p(1)\n");
    let b = made(&fx);
    assert_eq!(
        (b["at"].clone(), b["rev"].clone(), b["preferences"].clone()),
        (json!("song.k7"), json!("r1"), json!("p(1)\n"))
    );
    let files = &b["source"]["files"];
    assert_eq!(
        files["/w/song.k7/clips/riff.k7x"]["text"],
        json!("source(\"riff.wav\")\n")
    );
    assert_eq!(
        files["/w/song.k7/take.wav"],
        json!({ "len": 16044, "binary": true })
    );
    assert_eq!(b["source"]["lengths"]["/w/song.k7/take.wav"], json!(1.0));
    assert_eq!(files["/w/_playlists/set.k7x"]["text"], json!("set(1)\n"));
    assert!(files["/w/_modules/audio/m/module.k7x"].is_object());
    // A hidden folder is listed and never read.
    assert!(files.get("/w/song.k7/.hidden/x.txt").is_none());
    let names: Vec<&str> = b["source"]["lists"]["/w/song.k7"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&".hidden") && names.contains(&"take.wav") && names.contains(&"project.k7x"),
        "{names:?}"
    );
    // The library is named, never inside the source.
    assert!(
        !files
            .as_object()
            .unwrap()
            .keys()
            .any(|k| k.contains("libraries/k7-core"))
    );
    let lib = &b["libraries"][0];
    assert_eq!(lib["name"], json!("k7-core"));
    assert_eq!(lib["hash"].as_str().map(str::len), Some(64));
    assert_eq!(lib["files"], json!(1));
}

#[test]
fn a_library_hash_names_its_content_and_its_files_are_served_by_it() {
    let fx = Fx::new("vb-lib");
    let dir = fx.lib("").trim_end_matches('/').to_string();
    let hash = |d: &str| library_files(d).unwrap().hash();
    let (h1, n, _) = hash(&dir);
    assert_eq!(n, 1);
    assert_eq!(hash(&dir).0, h1, "stable");
    fx.s.file(
        "libraries/k7-core/modules/audio/verb/module.k7x",
        "extends(\"@engine/other\")\n",
    );
    assert_ne!(hash(&dir).0, h1, "a text moved");
    let (h2, _, _) = hash(&dir);
    fx.s.file("libraries/k7-core/modules/new.k7x", "n(1)\n");
    assert_ne!(hash(&dir).0, h2, "a file came");
    let files = library_files(&dir).unwrap().files;
    assert_eq!(
        files["modules/audio/verb/module.k7x"].text.as_deref(),
        Some("extends(\"@engine/other\")\n")
    );
    let roots = Roots {
        libraries: vec![Library {
            name: "k7-core".into(),
            root: dir.clone(),
            editable: false,
        }],
        meta: Some(fx.s.path("meta")),
        ..Roots::default()
    };
    let names: Vec<String> = library_roots(&roots).into_iter().map(|(n, _)| n).collect();
    assert_eq!(names, ["k7-core", "@meta"]);
}

// B2: no absolute server path reaches the browser: every key, root and mount is virtual, and the server maps each back.
#[test]
fn the_bundle_names_no_folder_of_the_server() {
    let fx = Fx::new("vb-virtual");
    std::fs::write(fx.at("song.k7/take.wav"), wav(800)).unwrap();
    let b = made(&fx);
    let all = b.to_string();
    for real in [
        fx.s.path(""),
        "/srv/banks".to_string(),
        "elsewhere/real".to_string(),
    ] {
        assert!(!all.contains(&real), "{real} reached the browser");
    }
    assert_eq!(b["workspaceRoot"], json!("/w"));
    assert_eq!(b["libraries"][0]["root"], json!("/l/k7-core"));
    let roots = &b["source"]["request"]["roots"];
    assert_eq!(roots["project"], json!("/w/song.k7"));
    assert_eq!(roots["meta"], json!("/l/@meta"));
    assert_eq!(roots["scan"], json!("/s/exts.json"));
    assert_eq!(roots["sample_banks"], json!(["/b/0"]));
    assert_eq!(roots["paths"], json!("posix"));
    assert!(b["source"]["lengths"]["/w/song.k7/take.wav"].is_number());
    let mounts: Vec<&str> = b["source"]["mounts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m.as_str().unwrap())
        .collect();
    assert!(
        mounts
            .iter()
            .all(|m| m.starts_with('/') && !m.contains(&fx.s.path(""))),
        "{mounts:?}"
    );
    // The server maps each virtual path back, and refuses one that climbs or names no root.
    let libs = vec![(
        "k7-core".to_string(),
        fx.lib("").trim_end_matches('/').to_string(),
    )];
    let v = Virtual::new(
        &fx.root,
        &libs,
        &fx.s.path("code"),
        &fx.s.path("studio"),
        &Roots::default(),
    );
    assert_eq!(v.real("/w/song.k7/a.txt"), Some(fx.at("song.k7/a.txt")));
    assert_eq!(v.real("/l/k7-core/modules/x"), Some(fx.lib("modules/x")));
    assert_eq!(v.real("/w/../etc/passwd"), None);
    assert_eq!(v.real("/wx/a"), None);
    assert_eq!(v.virt(&fx.at("song.k7")), "/w/song.k7");
    assert!(v.virt("/somewhere/else").starts_with("/x/"));
}

// F3: a library's WAV seconds are served with its files, and a change of them is a new hash.
#[test]
fn a_library_serves_its_lengths_and_they_are_part_of_its_hash() {
    let fx = Fx::new("vb-lib-lengths");
    let dir = fx.lib("").trim_end_matches('/').to_string();
    std::fs::create_dir_all(fx.lib("samples")).unwrap();
    std::fs::write(fx.lib("samples/kick.wav"), wav(8000)).unwrap();
    let lib = library_files(&dir).unwrap();
    assert_eq!(lib.lengths["samples/kick.wav"], 1.0);
    assert_eq!(lib.files["samples/kick.wav"].binary, Some(true));
    let (h1, _, _) = lib.hash();
    // A header that says another length for the same byte count is another library.
    let mut other = wav(8000);
    other[24..28].copy_from_slice(&16000u32.to_le_bytes());
    std::fs::write(fx.lib("samples/kick.wav"), other).unwrap();
    let again = library_files(&dir).unwrap();
    assert_eq!(again.lengths["samples/kick.wav"], 0.5);
    assert_ne!(again.hash().0, h1);
}

// A compile stats the renders an automatic freeze would play, one per lane: the folder is listed, or listed empty, so none is asked for.
#[test]
fn the_renders_folder_is_listed_and_listed_empty_when_it_is_not_there() {
    let fx = Fx::new("vb-renders");
    let none = made(&fx);
    assert_eq!(none["source"]["lists"]["/s/.renders"], json!([]));
    fx.s.file("studio/.renders/a-1.wav", "RIFF");
    let some = made(&fx);
    assert_eq!(
        some["source"]["files"]["/s/.renders/a-1.wav"],
        json!({ "len": 4, "binary": true })
    );
    assert_eq!(some["source"]["lists"]["/s/.renders"][0]["name"], "a-1.wav");
}

// A verb makes files beside the project (the pins, a playlist): the workspace's own names are listed, so they are new, not unknown.
// An automatic freeze fingerprints a small audio file by its bytes, which a bundle holds only by length: the digest travels instead.
#[test]
fn a_small_binary_carries_its_digest() {
    let fx = Fx::new("vb-digest");
    fx.s.file("studio/workspace/songs/song.k7/clips/kick.wav", "RIFF");
    let b = made(&fx);
    let d = &b["source"]["digests"]["/w/song.k7/clips/kick.wav"];
    // sha256("RIFF") = 4ff1..., first 16 hex.
    assert_eq!(d.as_str().map(str::len), Some(16), "{d}");
}

// `library versions` and `upgrade` ask which versions are installed; a Worker has no folder to read, so the bundle says.
#[test]
fn the_installed_versions_of_a_library_travel_with_their_levels() {
    let fx = Fx::new("vb-installed");
    fx.s.file(
        "libraries/k7-core/1/library.k7x",
        "library({ name: \"k7-core\", version: 1 })\n",
    );
    fx.s.file(
        "libraries/k7-core/3/library.k7x",
        "library({ name: \"k7-core\", version: 3, requires: 2 })\n",
    );
    fx.s.file("libraries/k7-core/draft/x.txt", "d");
    let b = made(&fx);
    assert_eq!(
        b["installed"]["k7-core"],
        json!([{ "version": 1, "requires": 1 }, { "version": 3, "requires": 2 }])
    );
}

#[test]
fn the_workspaces_own_names_are_listed() {
    let fx = Fx::new("vb-rootlist");
    fx.s.file("studio/workspace/songs/stray.txt", "x");
    let b = made(&fx);
    let names: Vec<&str> = b["source"]["lists"]["/w"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"song.k7"), "{names:?}");
    assert!(b["source"]["files"].get("/w/stray.txt").is_none());
}

// A machine with no models: the compile stats a model it chose, which a listing of the empty folder answers as absent.
#[test]
fn a_models_folder_that_is_not_there_is_listed_empty() {
    let fx = Fx::new("vb-nomodels");
    let b = made(&fx);
    assert_eq!(b["source"]["lists"]["/s/models"], json!([]));
}

#[test]
fn the_models_folder_is_listed_with_lengths_and_never_read() {
    let fx = Fx::new("vb-models");
    fx.s.file("studio/models/vocals/net.onnx", "12345");
    fx.s.file("studio/models/readme.txt", "text that is not read");
    let b = made(&fx);
    let files = &b["source"]["files"];
    assert_eq!(
        files["/s/models/vocals/net.onnx"],
        json!({ "len": 5, "binary": true })
    );
    assert_eq!(
        files["/s/models/readme.txt"],
        json!({ "len": 21, "binary": true })
    );
    let names: Vec<&str> = b["source"]["lists"]["/s/models"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["readme.txt", "vocals"]);
}

#[test]
fn the_models_listing_skips_hidden_names_and_a_clone_s_git_objects() {
    let fx = Fx::new("vb-models-hidden");
    fx.s.file("studio/models/net.onnx", "123");
    fx.s.file("studio/models/.git/objects/ab/cd", "packed");
    fx.s.file("studio/models/.cache", "x");
    let b = made(&fx);
    let files = b["source"]["files"].as_object().unwrap();
    assert!(files.contains_key("/s/models/net.onnx"));
    assert!(
        !files
            .keys()
            .any(|k| k.contains(".git") || k.contains(".cache"))
    );
    let names: Vec<&str> = b["source"]["lists"]["/s/models"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["net.onnx"]);
}

#[cfg(unix)]
#[test]
fn a_model_that_is_a_link_is_listed_when_it_stays_below_the_folder_and_left_out_when_it_leaves() {
    let fx = Fx::new("vb-models-links");
    fx.s.file("studio/models/store/real.onnx", "12345");
    fx.s.file("elsewhere/outside.onnx", "123456");
    let models = fx.s.path("studio/models");
    std::os::unix::fs::symlink(
        format!("{models}/store/real.onnx"),
        format!("{models}/alias.onnx"),
    )
    .unwrap();
    std::os::unix::fs::symlink(
        fx.s.path("elsewhere/outside.onnx"),
        format!("{models}/leak.onnx"),
    )
    .unwrap();
    let b = made(&fx);
    let files = b["source"]["files"].as_object().unwrap();
    assert_eq!(
        files["/s/models/alias.onnx"],
        json!({ "len": 5, "binary": true })
    );
    assert!(!files.contains_key("/s/models/leak.onnx"));
    let names: Vec<&str> = b["source"]["lists"]["/s/models"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["alias.onnx", "store"]);
}

#[test]
fn a_models_root_outside_the_studio_home_is_listed_under_its_own_virtual_name() {
    let fx = Fx::new("vb-models-outside");
    fx.s.file("elsewhere/zoo/big.onnx", "1234");
    let b = made_with(&fx, fx.s.path("elsewhere/zoo"));
    let files = b["source"]["files"].as_object().unwrap();
    let hit: Vec<&String> = files.keys().filter(|k| k.ends_with("/big.onnx")).collect();
    assert_eq!(hit.len(), 1);
    assert!(hit[0].starts_with("/x/"), "{hit:?}");
    assert!(!hit[0].contains("elsewhere"));
}
