//! The bench and the slot answered natively: the rig made from a chain and kept while edited, a slot compiled with nothing written, what it pins declined.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use serde_json::json;

use super::bench::Run;
use super::tests::scratch;
use k7_file_service::libraries::Home;

const CHAIN: &str = "steps([\n  id(\"11111111-1111-4111-8111-111111111111\").extends(\"@k7-core/audio/verb\"),\n  to([chain(\"@out\")]),\n])\n";

pub(super) fn fixture(name: &str, pinned: bool) -> std::path::PathBuf {
    let dir = scratch(name);
    let put = |rel: &str, text: &str| {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    };
    put(
        "libraries/k7-core/modules/audio/verb/module.k7x",
        "extends(\"@engine/reverb\")\n",
    );
    put("libraries/k7-core/chains/solo.k7x", CHAIN);
    if pinned {
        put("home/workspace/Banc/_library-pins.k7x", "k7-core(1)\n");
    }
    dir
}

pub(super) fn run_in(dir: &Path) -> Run {
    let env: BTreeMap<String, String> = [
        (
            "STUDIO_HOME".to_owned(),
            crate::routes::files::service::native::tests::slashed(&dir.join("home")),
        ),
        (
            "STUDIO_LIBRARIES_HOME".to_owned(),
            dir.join("libraries").display().to_string(),
        ),
    ]
    .into();
    Run {
        home: Home::of(&env, false),
        code_root: Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../..")
            .display()
            .to_string(),
        acct: crate::routes::files::service::Acct::make(env.clone(), super::tests_account::HEAP),
        env,
        slug: "bench-test".to_owned(),
        log: Arc::new(|_: &str| {}),
    }
}

pub(super) fn bench_file(dir: &Path, rel: &str) -> std::path::PathBuf {
    dir.join("home/workspace/Banc/live.k7").join(rel)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_bench_is_made_from_its_chain_and_kept_while_it_is_edited() {
    let dir = fixture("b-keep", true);
    let run = run_in(&dir);
    let (status, body) = run
        .bench("@k7-core/solo/live.k7")
        .await
        .expect("answered natively");
    assert_eq!(
        (status, &body),
        (200, &json!({ "workspace": "Banc", "project": "live.k7" }))
    );
    let graph = std::fs::read_to_string(bench_file(&dir, "graph.k7x")).unwrap();
    assert!(graph.contains("verb"), "{graph}");
    // A knob turned on the bench survives a reload of the same address.
    let fader = bench_file(&dir, "nodes/fader.k7x");
    let turned = format!("{}// turned\n", std::fs::read_to_string(&fader).unwrap());
    std::fs::write(&fader, &turned).unwrap();
    run.bench("@k7-core/solo/live.k7").await.unwrap();
    assert_eq!(std::fs::read_to_string(&fader).unwrap(), turned);
    // Another address is a new bench, and the old one is thrown away.
    run.bench("live.k7").await.unwrap();
    assert!(!std::fs::read_to_string(&fader).unwrap().contains("turned"));
    assert!(
        !std::fs::read_to_string(bench_file(&dir, "graph.k7x"))
            .unwrap()
            .contains("verb")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_address_that_is_no_bench_and_a_chain_that_is_not_there_are_answered_as_the_route_does()
{
    let dir = fixture("b-refuse", true);
    let run = run_in(&dir);
    let (status, body) = run.bench("song.k7").await.unwrap();
    assert_eq!(status, 404);
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .starts_with("@banc opens one live rig")
    );
    let (status, body) = run.bench("@k7-core/nothing/live.k7").await.unwrap();
    assert_eq!(
        (status, body["error"].as_str().unwrap()),
        (409, "no chain \"@k7-core/nothing\" in the library")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_workspace_with_no_pins_is_pinned_afresh_by_the_bench_and_the_slot() {
    let dir = fixture("b-pins", false);
    let pins = dir.join("home/workspace/Banc/_library-pins.k7x");
    let run = run_in(&dir);
    assert!(run.slot("@k7-core/solo").await.is_some());
    assert!(pins.exists());
    std::fs::remove_file(&pins).unwrap();
    assert!(run.bench("@k7-core/solo/live.k7").await.is_some());
    assert!(pins.exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_slot_is_compiled_and_nothing_is_written_beside_the_bench() {
    let dir = fixture("b-slot", true);
    let run = run_in(&dir);
    let before: Vec<_> = std::fs::read_dir(dir.join("home/workspace/Banc"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    let (status, slot) = run.slot("@k7-core/solo").await.expect("answered natively");
    assert_eq!(status, 200, "{slot}");
    assert_eq!(
        (
            slot["id"].as_str(),
            slot["chain"].as_str(),
            slot["protocol"].as_u64()
        ),
        (Some("solo"), Some("@k7-core/solo"), Some(1))
    );
    assert_eq!(slot["chainPlugins"], json!(["IN:verb"]));
    assert_eq!(slot["doc"]["name"], "slot solo");
    let after: Vec<_> = std::fs::read_dir(dir.join("home/workspace/Banc"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert_eq!(before, after);
    let (status, body) = run.slot("@k7-core/none").await.unwrap();
    assert_eq!(
        (status, body["error"].as_str().unwrap()),
        (400, "no chain \"@k7-core/none\" in the libraries")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_bench_saved_is_the_accounts_chain_and_a_copy_and_a_delete_follow() {
    let dir = fixture("b-save", true);
    let run = run_in(&dir);
    run.bench("@k7-core/solo/live.k7")
        .await
        .expect("answered natively");
    let bench = dir
        .join("home/workspace/Banc/live.k7")
        .display()
        .to_string();
    let (status, body) = run
        .save(&bench, Some("mine"))
        .await
        .expect("answered natively");
    assert_eq!(
        (status, &body),
        (
            200,
            &json!({ "ref": "@account/mine", "steps": 1, "outside": [] })
        )
    );
    let written = std::fs::read_to_string(dir.join("home/library/chains/mine.k7x")).unwrap();
    assert!(
        written.contains("11111111-1111-4111-8111-111111111111"),
        "{written}"
    );
    // A Worker names its project, not a folder: the bench's own name saves, any other is refused.
    let (status, body) = run.save_at("live.k7", Some("viaat")).await.unwrap();
    assert_eq!(
        (status, body["ref"].as_str()),
        (200, Some("@account/viaat"))
    );
    for at in ["song.k7", "../live.k7/../x", "/w/live.k7"] {
        let (status, body) = run.save_at(at, Some("nope")).await.unwrap();
        assert_eq!(
            (
                status,
                body["error"]
                    .as_str()
                    .map(|e| e.starts_with("only the bench"))
            ),
            (400, Some(true)),
            "{at}"
        );
    }
    assert!(!dir.join("home/library/chains/nope.k7x").exists());
    // A folder that is not the bench is refused in the TypeScript's words, and nothing more is written.
    let (status, body) = run.save("/elsewhere", Some("other")).await.unwrap();
    assert_eq!(status, 400);
    assert_eq!(
        body["error"],
        "only the bench saves a chain — open one on the bench from the Frozen chains page"
    );
    assert!(!dir.join("home/library/chains/other.k7x").exists());
    let (status, row) = run.copy_chain("dupe", "@k7-core/solo").await.unwrap();
    assert_eq!((status, row["ref"].as_str()), (200, Some("@account/dupe")));
    assert_ne!(
        row["steps"][0]["id"],
        "11111111-1111-4111-8111-111111111111"
    );
    let (status, body) = run.copy_chain("dupe", "@k7-core/solo").await.unwrap();
    assert_eq!(
        (status, body["error"].as_str()),
        (400, Some("you already have a chain \"dupe\""))
    );
    assert_eq!(
        run.remove_chain("dupe").await,
        (200, json!({ "deleted": true }))
    );
    let (status, body) = run.remove_chain("dupe").await;
    assert_eq!(
        (status, body["error"].as_str()),
        (404, Some("no chain \"dupe\" of yours"))
    );
}
