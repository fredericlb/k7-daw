//! `plan` inside a library: a module renamed with the refs to repoint, an engine refused, a move into a library, and
//! the bin a library keeps for itself (1221, 1224, 1231).

use crate::support;

use k7_file_service::entries::{EntryError, Outcome};
use support::entries::{Fx, delete, mv, rename, said};

#[test]
fn a_library_module_is_renamed_by_an_admin_with_its_refs_to_repoint() {
    let fx = Fx::new("p-lib-rename");
    let verb = "_libraries/k7-core/modules/audio/verb";
    assert_eq!(
        fx.admin(rename(verb, "hall")),
        Ok(Outcome::Rename {
            from: fx.lib("modules/audio/verb"),
            to: fx.lib("modules/audio/hall"),
            refs: Some((
                "@k7-core/audio/verb".into(),
                "@k7-core/audio/hall".into(),
                true
            )),
        })
    );
    assert_eq!(
        said(&fx, true, rename(verb, "hall")),
        serde_json::json!({ "path": "_libraries/k7-core/modules/audio/hall" })
    );
    // A folder of modules repoints every module in it.
    assert_eq!(
        fx.admin(rename("_libraries/k7-core/modules/audio", "sound")),
        Ok(Outcome::Rename {
            from: fx.lib("modules/audio"),
            to: fx.lib("modules/sound"),
            refs: Some(("@k7-core/audio".into(), "@k7-core/sound".into(), true)),
        })
    );
    // A library's own folder, a chain: no ref.
    fx.s.file("libraries/k7-core/chains/c/chain.k7x", "x");
    assert!(matches!(
        fx.admin(rename("_libraries/k7-core/chains/c", "d")),
        Ok(Outcome::Rename { refs: None, .. })
    ));
    assert_eq!(
        fx.refusal(false, rename(verb, "hall")),
        "can't rename '_libraries/k7-core/modules/audio/verb': only the origin's admins edit a library"
    );
}

#[test]
fn an_engine_is_not_renamed_because_nodes_pick_it_by_name() {
    let fx = Fx::new("p-engine");
    fx.s.file(
        "libraries/k7-core/modules/audio/verb/engines/plate/module.k7x",
        "x",
    );
    assert_eq!(
        fx.refusal(
            true,
            rename(
                "_libraries/k7-core/modules/audio/verb/engines/plate",
                "room"
            )
        ),
        "an engine is picked by its name (`engine: <name>` in every node using it), which renaming would strand — make a new engine instead"
    );
}

#[test]
fn a_move_into_a_library_is_a_write_there() {
    let fx = Fx::new("p-move-lib");
    fx.s.file("studio/workspace/songs/a.txt", "a");
    let to = "_libraries/k7-core/modules/audio";
    assert_eq!(
        fx.refusal(false, mv("a.txt", to)),
        "can't move into '_libraries/k7-core/modules/audio': only the origin's admins edit a library"
    );
    assert_eq!(
        fx.admin(mv("a.txt", to)),
        Ok(Outcome::Move {
            from: fx.at("a.txt"),
            to: fx.lib("modules/audio/a.txt")
        })
    );
    assert!(
        fx.refusal(false, mv("a.txt", "_modules"))
            .contains("a shared module is edited in place")
    );
}

#[test]
fn a_delete_recycles_into_the_first_free_name_of_the_right_bin() {
    let fx = Fx::new("p-delete");
    fx.s.file("studio/workspace/songs/a.txt", "a");
    assert_eq!(
        said(&fx, false, delete("a.txt")),
        serde_json::json!({ "recycled": "_recycle_bin/a.txt" })
    );
    assert_eq!(
        fx.user(delete("a.txt")),
        Ok(Outcome::Recycle {
            from: fx.at("a.txt"),
            to: fx.at("_recycle_bin/a.txt")
        })
    );
    fx.s.file("studio/workspace/songs/_recycle_bin/a.txt", "old");
    assert_eq!(
        said(&fx, false, delete("a.txt")),
        serde_json::json!({ "recycled": "_recycle_bin/a.txt-2" })
    );
    fx.s.file("studio/workspace/songs/_recycle_bin/a.txt-2", "old");
    assert_eq!(
        said(&fx, false, delete("a.txt")),
        serde_json::json!({ "recycled": "_recycle_bin/a.txt-3" })
    );
    // A library entry goes to the library's own bin, a root's never to the workspace's.
    assert_eq!(
        said(&fx, true, delete("_libraries/k7-core/modules/audio/verb")),
        serde_json::json!({ "recycled": "_libraries/k7-core/_recycle_bin/verb" })
    );
    assert_eq!(
        fx.user(delete("none")),
        Err(EntryError::NotFound { rel: "none".into() })
    );
}
