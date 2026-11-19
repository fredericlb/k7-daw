//! What the J3 review asked to be held by a test: one prepare per document, the dry run playing the document sent (take hole
//! included), a hanging filing holding neither the reader nor the forwards, and the poll's start opening the module watch.

use std::time::Duration;

use k7_file_service::engine::docs::TakeArm;
use serde_json::json;

use super::fake::{Rig, until};
use super::stats::Stats;

fn projects(rig: &Rig) -> Vec<serde_json::Value> {
    rig.engine
        .of(Rig::SESSION)
        .into_iter()
        .filter(|f| f["type"] == "project")
        .collect()
}

// A document is prepared once, off the session's lock: three documents are three prepares (J2 fix 2).
#[tokio::test]
async fn a_document_is_prepared_once() {
    let rig = Rig::new("prepares").await;
    let s = rig.session();
    for n in 0..3 {
        s.push_doc(super::fake::minimal_doc(-(n as f64)));
    }
    until(10, "the last document", || {
        projects(&rig)
            .iter()
            .any(|f| f["doc"]["masterVolume"] == json!(-2.0))
    })
    .await;
    assert_eq!(Stats::get(&rig.link.stats().prepares), 3);
}

// The dry run plays the document that was sent, so an armed take's hole is in every `project` frame of the load (J2 fix 3).
#[tokio::test]
async fn the_dry_run_plays_the_document_with_the_take_hole() {
    let rig = Rig::new("dryhole").await;
    let s = rig.session();
    s.set_take_arm(Some(TakeArm {
        track: "t1".into(),
        event_beats: 0.0,
        from: 0.0,
        to: Some(4.0),
    }));
    s.push_doc(super::fake::minimal_doc(-3.0));
    until(10, "the dry run's transport", || {
        rig.engine
            .kinds(Rig::SESSION)
            .iter()
            .any(|k| k == "transport play")
    })
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let docs = projects(&rig);
    assert!(docs.len() >= 2, "the document and the dry run's: {docs:?}");
    for d in &docs {
        assert_eq!(d["doc"]["recordMute"], json!(false), "{d}");
    }
}

pub(super) fn silent_wav(path: &std::path::Path) {
    let mut b = vec![0u8; 44];
    b[..4].copy_from_slice(b"RIFF");
    b[4..8].copy_from_slice(&36u32.to_le_bytes());
    b[8..16].copy_from_slice(b"WAVEfmt ");
    b[16..20].copy_from_slice(&16u32.to_le_bytes());
    b[20..22].copy_from_slice(&1u16.to_le_bytes());
    b[22..24].copy_from_slice(&2u16.to_le_bytes());
    b[24..28].copy_from_slice(&48000u32.to_le_bytes());
    b[28..32].copy_from_slice(&192000u32.to_le_bytes());
    b[32..34].copy_from_slice(&4u16.to_le_bytes());
    b[34..36].copy_from_slice(&16u16.to_le_bytes());
    b[36..40].copy_from_slice(b"data");
    std::fs::write(path, b).unwrap();
}

// A take whose filing runs long in the real host (its push compiles slowly there) holds neither the reader nor the next event (B3, D6).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_slow_filing_holds_neither_the_reader_nor_what_comes_after_it() {
    let a = super::tests_real::account_with_lane("filing").await;
    a.open("p.k7");
    a.start().await;
    until(30, "the document", || {
        a.kinds().iter().any(|k| k == "project")
    })
    .await;
    let engine = a.engine.as_ref().unwrap();
    let wav = std::path::Path::new(&a.ws).join("p.k7/clips/take-tmp.wav");
    silent_wav(&wav);
    engine
        .host
        .slow_ms
        .store(3000, std::sync::atomic::Ordering::Relaxed);
    let filed = std::time::Instant::now();
    a.fake.send_all(json!({
        "type": "recorded", "track": "gtr", "file": wav.display().to_string(), "start": 0, "length": 2,
    }));
    a.fake.send_all(json!({ "type": "state", "playing": true }));
    let s = engine.link.session(Some(&a.ws));
    until(2, "the state after the take, while it is filed", || {
        s.playing()
    })
    .await;
    assert!(filed.elapsed() < Duration::from_secs(2));
    until(20, "the filing's answer", || {
        engine
            .bridge
            .last_take(super::tests_real::SESSION)
            .is_some()
    })
    .await;
    assert!(
        filed.elapsed() >= Duration::from_secs(3),
        "the filing ran slow"
    );
}

// A take is filed with the journal's gate to itself: while a record holds the gate, the take waits, so no step absorbs it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_take_waits_for_the_journals_gate() {
    let a = super::tests_real::account_with_lane("filing-gate").await;
    a.open("p.k7");
    a.start().await;
    until(30, "the document", || {
        a.kinds().iter().any(|k| k == "project")
    })
    .await;
    let engine = a.engine.as_ref().unwrap();
    let wav = std::path::Path::new(&a.ws).join("p.k7/clips/take-tmp.wav");
    silent_wav(&wav);
    let root = k7_file_service::paths::resolve_all(&[a.ws.as_str()]);
    let record = k7_file_service::journal::Journals::global()
        .enter(&root)
        .await;
    a.fake.send_all(json!({
        "type": "recorded", "track": "gtr", "file": wav.display().to_string(), "start": 0, "length": 2,
    }));
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let last = || engine.bridge.last_take(super::tests_real::SESSION);
    assert!(last().is_none(), "the take landed inside an open record");
    drop(record);
    until(20, "the filing after the gate is free", || last().is_some()).await;
}

// A served folder's unload drops the session the engine holds for it (J4 B2).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_unload_drops_the_session_of_the_served_folder() {
    let a = super::tests_real::account("letgo").await;
    a.open("p.k7");
    a.start().await;
    until(30, "the document", || {
        a.kinds().iter().any(|k| k == "project")
    })
    .await;
    a.unload().await;
    until(10, "the drop", || {
        a.kinds().iter().any(|k| k == "drop-session")
    })
    .await;
}

// The open of a second project answers a load phase, never null, though its start still waits in the session's queue (J4 B1).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_open_of_a_second_project_answers_a_load_phase() {
    let a = super::tests_real::account("second").await;
    let engine = a.engine.as_ref().unwrap();
    let s = engine.link.session(Some(&a.ws));
    a.open("p.k7");
    a.start().await;
    until(30, "the first project loaded", || {
        s.serving().is_some() && s.load_phase().is_none()
    })
    .await;
    let q = std::path::Path::new(&a.ws).join("q.k7");
    std::fs::create_dir_all(q.join("nodes")).unwrap();
    std::fs::write(q.join("nodes/gtr.k7x"), "extends(\"@meta/track\")\n").unwrap();
    std::fs::write(q.join("graph.k7x"), "modules({ gtr: [\"MIX\"] })\n").unwrap();
    std::fs::write(q.join("project.k7x"), "project({})\n").unwrap();
    // The engine answers the second project loaded only far past the check, so its phase is held.
    a.fake.loaded_after(Duration::from_secs(120));
    // A job ahead keeps the second start waiting, as a busy queue does.
    s.enqueue(|_| async { tokio::time::sleep(Duration::from_millis(400)).await });
    a.open("q.k7");
    a.start().await;
    let answer = super::super::projects_open::link_load(engine, &a.ws).await;
    assert!(
        ["copying", "scanning", "applying", "warming"]
            .iter()
            .any(|p| answer == json!(p)),
        "{answer}"
    );
}

// A poll's prewarm is a job in the session's queue: a start of another project queued before it is decided first, and the prewarm then finds another project served (J4 B3).
#[tokio::test]
async fn a_poll_s_prewarm_never_overtakes_a_start_queued_before_it() {
    let rig = Rig::new("prewarm").await;
    let s = rig.session();
    let (a, b) = (format!("{}/p.k7", rig.ws), format!("{}/q.k7", rig.ws));
    std::fs::create_dir_all(&b).unwrap();
    let (root, project) = (rig.ws.clone(), b.clone());
    s.enqueue(|_| async { tokio::time::sleep(Duration::from_millis(300)).await });
    s.enqueue(move |s| async move { s.start(&root, &project).await });
    s.prewarm_soon(&rig.ws, &a);
    until(10, "the start's document", || {
        s.serving().as_deref() == Some(b.as_str())
    })
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(s.serving().as_deref(), Some(b.as_str()));
    let compiled = rig.host.compiled.lock().unwrap().clone();
    assert!(
        !compiled.contains(&a),
        "the prewarm of {a} compiled: {compiled:?}"
    );
}

// A frame asked while a restored document compiles leaves after it (J4 B3).
#[tokio::test]
async fn a_frame_asked_while_a_restore_compiles_leaves_after_the_restored_document() {
    let rig = Rig::new("restore").await;
    let s = rig.session();
    s.start(&rig.ws, &rig.project).await;
    until(20, "the load done", || {
        !projects(&rig).is_empty() && s.load_phase().is_none()
    })
    .await;
    let before = rig.engine.kinds(Rig::SESSION).len();
    rig.host
        .slow_compile
        .store(400, std::sync::atomic::Ordering::Relaxed);
    s.after(vec![super::queue::After::Restore]);
    s.frame(json!({ "type": "preview", "file": "x.wav", "start": 0, "length": 1 }));
    until(10, "the preview", || {
        rig.engine.kinds(Rig::SESSION)[before..]
            .iter()
            .any(|k| k == "preview")
    })
    .await;
    let after: Vec<String> = rig.engine.kinds(Rig::SESSION)[before..].to_vec();
    let at = |w: &str| after.iter().position(|k| k == w);
    assert!(
        at("project").is_some() && at("project") < at("preview"),
        "{after:?}"
    );
}

// A project compiling holds the frames asked meanwhile, whoever called `push_project` (J4 B3).
#[tokio::test]
async fn a_frame_asked_while_a_project_compiles_waits_for_its_document() {
    let rig = Rig::new("compiling").await;
    let s = rig.session();
    rig.host
        .slow_compile
        .store(400, std::sync::atomic::Ordering::Relaxed);
    let served = super::queue::Served {
        root: rig.ws.clone(),
        project: rig.project.clone(),
    };
    let task = {
        let s = s.clone();
        tokio::spawn(async move { s.push_project(&served).await })
    };
    tokio::time::sleep(Duration::from_millis(100)).await;
    s.frame(json!({ "type": "preview", "file": "x.wav", "start": 0, "length": 1 }));
    let _ = task.await;
    until(10, "the preview", || {
        rig.engine
            .kinds(Rig::SESSION)
            .iter()
            .any(|k| k == "preview")
    })
    .await;
    let kinds = rig.engine.kinds(Rig::SESSION);
    let at = |w: &str| kinds.iter().position(|k| k == w);
    assert!(
        at("project").is_some() && at("project") < at("preview"),
        "{kinds:?}"
    );
}

// The poll that starts a load already answers `copying`, as the TypeScript's synchronous start did (2271 no longer departs).
#[tokio::test]
async fn the_poll_that_starts_a_load_answers_copying() {
    let rig = Rig::new("pollcopy").await;
    let s = rig.session();
    s.enqueue(|_| async { tokio::time::sleep(Duration::from_millis(300)).await });
    s.prewarm_soon(&rig.ws, &rig.project);
    assert_eq!(
        s.engine_load_of(Some(&rig.project)),
        Some(k7_file_service::engine::Phase::Copying)
    );
}

fn devices(a: &super::tests_real::Account) -> Vec<String> {
    a.fake
        .of(super::tests_real::SESSION)
        .iter()
        .filter(|f| f["type"] == "set-audio-device")
        .map(|f| f["name"].as_str().unwrap_or("").to_string())
        .collect()
}

// A socket's open replays the preferences as the file holds them then: a device changed since the last open is the one sent (J4 fix 1).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_redial_replays_the_preferences_as_they_stand() {
    let a = super::tests_real::account("prefs").await;
    let file = a.dir.join("home/preferences.k7x");
    let home = k7_file_service::libraries::Home::of(&std::collections::BTreeMap::from([(
        "STUDIO_HOME".to_string(),
        crate::routes::files::service::native::tests::slashed(&a.dir.join("home")),
    )]));
    let file = std::path::Path::new(&home.studio).join(file.file_name().unwrap());
    std::fs::create_dir_all(&home.studio).unwrap();
    std::fs::write(&file, "audioDevice({ name: \"X\" })\n").unwrap();
    a.open("p.k7");
    a.start().await;
    until(30, "the first device", || devices(&a) == ["X"]).await;
    std::fs::write(&file, "audioDevice({ name: \"Y\" })\n").unwrap();
    a.fake.close_all();
    let s = a.engine.as_ref().unwrap().link.session(Some(&a.ws));
    let end = std::time::Instant::now() + Duration::from_secs(20);
    while !devices(&a).contains(&"Y".to_string()) {
        assert!(std::time::Instant::now() < end, "{:?}", devices(&a));
        s.prewarm(&a.ws, &format!("{}/p.k7", a.ws)).await;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

// A document a push compiled never overruns a browser's document that arrived while it compiled (J4 fix 2).
#[tokio::test]
async fn a_compiled_push_is_dropped_when_a_browsers_document_arrived_meanwhile() {
    let rig = Rig::new("overrun").await;
    let s = rig.session();
    let client = super::fake::minimal_doc(-9.0);
    s.enqueue(|_| async { tokio::time::sleep(Duration::from_millis(300)).await });
    s.push_compiled(super::fake::minimal_doc(-1.0));
    s.apply_client_now(client);
    until(10, "the client's document", || {
        projects(&rig)
            .iter()
            .any(|f| f["doc"]["masterVolume"] == json!(-9.0))
    })
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !projects(&rig)
            .iter()
            .any(|f| f["doc"]["masterVolume"] == json!(-1.0)),
        "the push's document was sent"
    );
}

// The take's arm is the one held when the take was reported: a disarm that runs before the filing does not move the take (K3 fix 4).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_take_is_filed_under_the_arm_held_when_it_was_reported() {
    let a = super::tests_real::account_with_lane("armcap").await;
    let project = std::path::Path::new(&a.ws).join("p.k7");
    let gtr = project.join("nodes/gtr.k7x");
    silent_wav(&project.join("clips/a.wav"));
    std::fs::write(project.join("clips/a1.k7x"), "source(\"clips/a.wav\")\n").unwrap();
    std::fs::write(
        &gtr,
        "extends(\"@meta/track\")\nevents([{ ref: \"a1\", start: \"1:1\" }])\n",
    )
    .unwrap();
    a.open("p.k7");
    a.start().await;
    until(30, "the document", || {
        a.kinds().iter().any(|k| k == "project")
    })
    .await;
    let engine = a.engine.as_ref().unwrap();
    let session = engine.link.session(Some(&a.ws));
    session.set_take_arm(Some(TakeArm {
        track: "gtr".into(),
        event_beats: 0.0,
        from: 0.0,
        to: Some(8.0),
    }));
    let take = |n: u32, start: u32| {
        let wav = std::path::Path::new(&a.ws).join(format!("p.k7/clips/take-{n}.wav"));
        silent_wav(&wav);
        a.fake.send_all(json!({
            "type": "recorded", "track": "gtr", "file": wav.display().to_string(), "start": start, "length": 2,
        }));
    };
    // The first take's filing is slow: the second is reported while the arm stands, and the disarm lands before its filing runs.
    engine
        .host
        .slow_ms
        .store(3000, std::sync::atomic::Ordering::Relaxed);
    take(1, 0);
    tokio::time::sleep(Duration::from_millis(300)).await;
    take(2, 4);
    tokio::time::sleep(Duration::from_millis(300)).await;
    session.set_take_arm(None);
    engine
        .host
        .slow_ms
        .store(0, std::sync::atomic::Ordering::Relaxed);
    let in_event = || {
        let text = std::fs::read_to_string(&gtr).unwrap();
        let list = text.split("takes: [").nth(1).unwrap_or_default();
        list.split(']')
            .next()
            .unwrap_or_default()
            .matches("ref(")
            .count()
    };
    // Both went INTO the event it was armed on, though the second one's arm was gone when its filing ran.
    until(30, "both takes filed into the event", || in_event() == 2).await;
}
