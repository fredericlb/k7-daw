// facade.rs — the call-by-name surface itself: every listed name has an arm, a batch stops at its first throw, an unknown name is refused.
// A reader would get wrong that the replays prove the answers: this file proves only the table and the batch around them.

use serde_json::{Value as Json, json};
use song_edit::MemFs;
use song_edit::confine::Env;
use song_edit::facade::{self, Call, Ctx, Failure, env_from_json};

fn run(fs: &mut MemFs, name: &str, args: &[Json]) -> facade::Out {
    let env = Env::at("/home", "/meta");
    let mut ids = || "id".to_owned();
    let mut cx = Ctx {
        fs,
        env: &env,
        new_id: &mut ids,
    };
    facade::run_named(&mut cx, name, args)
}

#[test]
fn every_listed_name_has_an_arm() {
    let names = facade::names();
    assert!(names.len() >= 190, "the table lists {} names", names.len());
    let mut unique = names.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), names.len(), "a name is listed twice");
    for name in names {
        let mut fs = MemFs::new();
        let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&mut fs, &name, &[])));
        match out {
            Ok(Err(Failure::Unsupported(why @ ("no port" | "a callback")))) => panic!("{name} has no arm: {why}"),
            Ok(_) => {}
            Err(_) => panic!("{name} panics on an empty call"),
        }
    }
}

#[test]
fn a_name_off_the_table_is_refused() {
    let mut fs = MemFs::new();
    assert_eq!(
        run(&mut fs, "song-edit/projectEdit.nothing", &[]),
        Err(Failure::Unsupported("no port"))
    );
    assert_eq!(run(&mut fs, "nothing", &[]), Err(Failure::Unsupported("no port")));
    assert_eq!(
        run(&mut fs, "song-edit/trackMix.editTrackStruct", &[]),
        Err(Failure::Unsupported("a callback"))
    );
}

#[test]
fn a_batch_stops_at_its_first_throw() {
    let mut fs = MemFs::new();
    let env = Env::at("/home", "/meta");
    let mut ids = || "id".to_owned();
    let mut cx = Ctx {
        fs: &mut fs,
        env: &env,
        new_id: &mut ids,
    };
    let calls: Vec<Call> = [
        json!({ "fn": "song/yaml/workspaceFile.wsExists", "args": ["/p/a.k7x"] }),
        json!({ "fn": "song/yaml/workspaceFile.wsRead", "args": ["/p/a.k7x"] }),
        json!({ "fn": "song/yaml/workspaceFile.wsExists", "args": ["/p/b.k7x"] }),
    ]
    .iter()
    .filter_map(Call::from_json)
    .collect();
    assert_eq!(calls.len(), 3);
    let out = facade::batch(&mut cx, &calls);
    assert_eq!(out.done, vec![json!(false)]);
    assert!(matches!(out.failed, Some(Failure::Thrown(_))), "{:?}", out.failed);
}

#[test]
fn a_batch_without_a_throw_answers_every_call() {
    let mut fs = MemFs::new();
    let env = Env::at("/home", "/meta");
    let mut ids = || "id".to_owned();
    let mut cx = Ctx {
        fs: &mut fs,
        env: &env,
        new_id: &mut ids,
    };
    let calls = vec![
        Call {
            name: "song/yaml/workspaceFile.isDocName".to_owned(),
            args: vec![json!("a.k7x")],
        },
        Call {
            name: "song/yaml/workspaceFile.wsExists".to_owned(),
            args: vec![json!("/p/a.k7x")],
        },
    ];
    let out = facade::batch(&mut cx, &calls);
    assert_eq!(out.done.len(), 2);
    assert!(out.failed.is_none());
}

#[test]
fn an_environment_crosses_as_json() {
    let env = env_from_json(&json!({
        "studioHome": "/s",
        "metaRoot": "/m",
        "modelsRoot": "/models",
        "overlays": [["acme", "/lib/acme"]],
        "libraryRoots": [["core", "/lib/core"]],
    }));
    assert_eq!(env.studio_home, "/s");
    assert_eq!(env.meta_root, "/m");
    assert_eq!(env.models_root, "/models");
    assert_eq!(env.overlays, vec![("acme".to_owned(), "/lib/acme".to_owned())]);
    assert_eq!(
        env.library_roots,
        Some(vec![("core".to_owned(), "/lib/core".to_owned())])
    );
}
