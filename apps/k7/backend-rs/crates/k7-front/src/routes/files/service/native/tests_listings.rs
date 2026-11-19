//! The listings' front (1404 to 1408, 1431, 1440 to 1444): the tags, the four ops and `/module-libraries` answered
//! natively, and a decline refused.

use serde_json::{Value, json};

use super::listings;
use super::tests::{Lines, ctx, scratch};
use super::tests_account::{call, text};
use super::{Piece, Tag};
use crate::routes::files::service::routes::{self, H};

fn fixture(dir: &std::path::Path) {
    let ws = dir.join("home/workspace/songs");
    for (rel, body) in [
        ("_playlists/a.k7x", "title(\"A\")\nitems([\"x\", \"y\"])\n"),
        ("_references/r.wav", "RIFF"),
        ("_library-pins.k7x", "k7-core(1)\n"),
        ("../_modules/audio/own/module.k7x", ""),
    ] {
        let p = ws.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }
    for (rel, body) in [
        (
            "k7-core/1/library.k7x",
            "name(\"k7-core\")\nversion(1)\nrequires(1)\n",
        ),
        ("k7-core/1/modules/audio/eq/module.k7x", ""),
        ("k7-core/1/chains/lead.k7x", ""),
        ("k7-core/1/assemblies/box/assembly.k7x", ""),
        (
            "k7-core/2/library.k7x",
            "name(\"k7-core\")\nversion(2)\nrequires(1)\n",
        ),
    ] {
        let p = dir.join("libraries").join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }
}

#[test]
fn a_tag_names_its_route_its_piece_and_its_weight() {
    for (h, tag, route) in [
        (H::Playlists, Tag::Playlists, "GET /playlists"),
        (H::References, Tag::References, "GET /references"),
        (H::LibraryChains, Tag::LibraryChains, "GET /library-chains"),
        (
            H::LibraryAssemblies,
            Tag::LibraryAssemblies,
            "GET /library-assemblies",
        ),
    ] {
        assert_eq!(Tag::of(h), Some(tag));
        assert_eq!((tag.piece(), tag.route()), (Piece::Listings, route));
    }
    assert_eq!(Tag::ModuleLibraries.piece(), Piece::Listings);
    assert_eq!(Tag::Tree.piece(), Piece::Reads);
    // 1442: three weigh a tree, the playlists and the references do not.
    let heavy: Vec<bool> = [
        Tag::ModuleLibraries,
        Tag::LibraryChains,
        Tag::LibraryAssemblies,
        Tag::Playlists,
        Tag::References,
    ]
    .iter()
    .map(|t| listings::weighs_a_tree(*t))
    .collect();
    assert_eq!(heavy, [true, true, true, false, false]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_four_ops_are_answered_natively() {
    let dir = scratch("lst-ops");
    fixture(&dir);
    let lines = Lines::default();
    for (h, tag, key) in [
        (H::Playlists, Tag::Playlists, "playlists"),
        (H::References, Tag::References, "references"),
        (H::LibraryChains, Tag::LibraryChains, "chains"),
        (H::LibraryAssemblies, Tag::LibraryAssemblies, "assemblies"),
    ] {
        let c = ctx(&dir, tag, "", &lines);
        let (status, body) =
            text(routes::dispatch(h, &call(&dir, "", json!({}), Some(c))).await).await;
        assert_eq!(status, 200, "{tag:?}: {body}");
        let v: Value = serde_json::from_str(&body).unwrap();
        assert!(v[key].is_array(), "{tag:?}: {body}");
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn module_libraries_are_answered_natively_and_a_decline_is_refused() {
    let dir = scratch("lst-live");
    fixture(&dir);
    let lines = Lines::default();
    let c = ctx(&dir, Tag::ModuleLibraries, "", &lines);
    let (status, body) = text(listings::answer(c).await).await;
    assert_eq!(status, 200, "{body}");
    let v: Value = serde_json::from_str(&body).unwrap();
    assert!(v["libraries"].is_array(), "{body}");
    // A library as the open project declines, and the decline is the answer: no other side is asked (4-7).
    std::fs::write(
        dir.join("home/workspace/.active-projects/songs"),
        "_libraries/x/p.k7",
    )
    .unwrap();
    let c = ctx(&dir, Tag::ModuleLibraries, "", &lines);
    let (status, body) = text(super::refuse::scoped(listings::answer(c)).await).await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("library-active"), "{body}");
    let _ = std::fs::remove_dir_all(dir);
}
