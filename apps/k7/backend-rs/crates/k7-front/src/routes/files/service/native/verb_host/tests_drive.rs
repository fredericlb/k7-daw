//! The render family's drivers behind the frame (3f, M5): a doc mapped then rendered, a peak, a cut, a playlist's stems.
//! What a reader gets wrong: the machine is a fake that writes the files a real one would, so what is judged is the route.

use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};

use crate::routes::files::service::native::callbacks::Reply;
use serde_json::{Value, json};
use song::doc::ProjectDoc;

use super::tests::{Bed, bed};
use k7_file_service::verb_host::jobs::Table;

use super::{Member, drive};
use crate::routes::files::service::native::jobs::{self, Gone, Jobs};

/// A machine that renders silence and cuts into the folder it is told, noting what it was asked.
#[derive(Default)]
pub(super) struct Fake {
    pub bound: bool,
    renders: Mutex<Vec<Value>>,
    cuts: Mutex<Vec<Value>>,
    jobs: OnceLock<Weak<Jobs>>,
    /// The file a cut's record names beyond its folder, to prove a machine's word is not trusted.
    stray: Option<String>,
    /// A render that wrote its mix and then failed, as an engine killed late does.
    failing: bool,
}

pub(super) fn wav(path: &Path, frames: u32, sample: i16) {
    let data = frames * 2;
    let mut b = b"RIFF".to_vec();
    b.extend((36 + data).to_le_bytes());
    b.extend(b"WAVEfmt ");
    b.extend(16u32.to_le_bytes());
    b.extend([1, 0, 1, 0]);
    b.extend(8000u32.to_le_bytes());
    b.extend(16000u32.to_le_bytes());
    b.extend([2, 0, 16, 0]);
    b.extend(b"data");
    b.extend(data.to_le_bytes());
    for _ in 0..frames {
        b.extend(sample.to_le_bytes());
    }
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b).unwrap();
}

impl jobs::Driver for Fake {
    fn renderer_available(&self) -> bool {
        self.bound
    }
    fn bound(&self) -> bool {
        self.bound
    }
    fn render_run_until(&self, args: Value, reply: Reply, _gone: Gone) {
        self.renders.lock().unwrap().push(args.clone());
        let out = args["outDir"].as_str().unwrap();
        wav(&Path::new(out).join("mix.wav"), 800, 1000);
        let mut stems = vec![];
        if args["stems"] == true {
            wav(&Path::new(out).join("stem-a.wav"), 400, 500);
            stems.push(json!({ "name": "a", "file": "stem-a.wav" }));
        }
        if self.failing {
            return reply(Err("engine render failed (exit 1)".into()));
        }
        reply(Ok(json!({ "value": { "mix": "mix.wav", "stems": stems } })));
    }
    fn render_all(&self, _dir: String, reply: Reply) {
        reply(Ok(json!({ "value": [] })));
    }
    fn separate(&self, id: &str, generation: u64, args: Value) {
        self.cuts.lock().unwrap().push(args.clone());
        let out = args["outDir"].as_str().unwrap();
        wav(&Path::new(out).join("drums.wav"), 100, 100);
        let file = match &self.stray {
            Some(s) => s.replace("{out}", out),
            None => format!("{out}/drums.wav"),
        };
        let record = json!({
            "id": id, "done": true, "failed": false, "error": null, "phase": "done",
            "percent": 100, "lines": [], "files": [file]
        });
        let jobs = self.jobs.get().and_then(Weak::upgrade).unwrap();
        let Value::Object(record) = record else {
            unreachable!()
        };
        jobs.registry()
            .progress(Instant::now(), id, generation, record);
    }
    fn attach(&self, jobs: Weak<Jobs>) {
        let _ = self.jobs.set(jobs);
    }
}

pub(super) struct Rig {
    pub b: Bed,
    machine: Arc<Fake>,
    jobs: Arc<Jobs>,
}

pub(super) async fn rig(name: &str, machine: Fake) -> Rig {
    let machine = Arc::new(machine);
    let jobs = Jobs::new(Some(machine.clone() as Arc<dyn jobs::Driver>));
    Rig {
        b: bed(name).await,
        machine,
        jobs,
    }
}

impl Rig {
    pub(super) fn jobs(&self) -> Arc<Jobs> {
        self.jobs.clone()
    }

    /// The workspace the rig's requests are in: a record is a path below it.
    pub(super) fn space(&self) -> std::path::PathBuf {
        self.b.dir.join("home/workspace/songs")
    }

    pub(super) async fn go(&self, m: Member, account: &str, body: Value) -> (u16, Value) {
        let mut ctx = self.b.ctx(m, false);
        ctx.account = account.into();
        ctx.verbs = drive::production(Some(self.jobs.clone()));
        self.b.send(m, "", body, Some(ctx)).await
    }

    /// One poll of the job, whatever its state.
    pub(super) async fn polled(&self, account: &str, job: &str) -> Value {
        let mut ctx = self.b.ctx(Member::Job, false);
        ctx.account = account.into();
        let q = format!("id={job}");
        let (s, a) = self.b.send(Member::Job, &q, json!({}), Some(ctx)).await;
        assert_eq!(s, 200, "{a}");
        a
    }

    /// Polls the job to its end, a minute at most.
    pub(super) async fn ended(&self, account: &str, job: &str) -> Value {
        let q = format!("id={job}");
        for _ in 0..600 {
            let mut ctx = self.b.ctx(Member::Job, false);
            ctx.account = account.into();
            let (s, a) = self.b.send(Member::Job, &q, json!({}), Some(ctx)).await;
            assert_eq!(s, 200, "{a}");
            if a["state"] != "running" {
                return a;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        panic!("the job never ended");
    }

    pub(super) fn put(&self, rel: &str) -> std::path::PathBuf {
        let p = self.b.dir.join("home/workspace/songs").join(rel);
        wav(&p, 800, 1000);
        p
    }
}

fn doc_with(source: &str) -> Value {
    let mut d = serde_json::to_value(ProjectDoc::empty()).unwrap();
    d["tracks"] = json!([{
        "id": "t", "name": "t", "role": "audio", "contentType": "audio", "group": "", "generated": false,
        "color": "", "mute": false, "solo": false, "disabled": false, "inputChannels": [], "outputChannels": [],
        "monitor": "off", "inputGain": 0.0, "plugins": [], "sends": [], "automation": [],
        "clips": [{
            "id": "c", "type": "audio", "name": "c", "source": source, "start": 0.0, "length": 1.0, "offset": 0.0,
            "fadeIn": 0.0, "fadeOut": 0.0, "loopStart": 0.0, "loopEnd": 0.0, "notes": []
        }]
    }]);
    d
}

pub(super) fn bound() -> Fake {
    Fake {
        bound: true,
        ..Fake::default()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_doc_is_mapped_rendered_and_its_files_answered_by_name_length_and_seconds() {
    let r = rig("vh-render", bound()).await;
    let real = r.put("p.k7/a.wav");
    let body = json!({ "at": "p.k7", "doc": doc_with("/w/p.k7/a.wav"), "stems": true });
    let (s, a) = r.go(Member::Render, "ra", body).await;
    assert_eq!(s, 202, "{a}");
    let job = a["job"].as_str().unwrap().to_string();
    let end = r.ended("ra", &job).await;
    assert_eq!(end["state"], "done", "{end}");
    let files = end["value"]["files"].as_array().unwrap();
    let names: Vec<&str> = files.iter().map(|f| f["out"].as_str().unwrap()).collect();
    assert_eq!(names, ["mix.wav", "stem-a.wav"]);
    assert_eq!(files[0]["len"], 44 + 1600);
    assert!((files[0]["seconds"].as_f64().unwrap() - 0.1).abs() < 1e-9);
    assert_eq!(end["value"]["manifest"]["mix"], "mix.wav");
    // The engine got the REAL path, in the job's own folder, and the answer repeats no folder of the server.
    let seen = r.machine.renders.lock().unwrap()[0].clone();
    assert_eq!(
        seen["doc"]["tracks"][0]["clips"][0]["source"],
        super::super::tests::slashed(&real)
    );
    assert!(
        seen["outDir"]
            .as_str()
            .unwrap()
            .contains(&format!(".job-out/{job}"))
    );
    assert!(
        !end.to_string()
            .contains(&super::super::tests::slashed(&r.b.dir)),
        "{end}"
    );
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

// 3f-5: a doc naming a path outside the roots starts nothing.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_doc_naming_a_path_outside_is_a_403_and_no_machine_is_asked() {
    let r = rig("vh-render-403", bound()).await;
    for bad in ["/etc/passwd", "/w/../../etc/passwd", "p.k7/a.wav"] {
        let body = json!({ "at": "p.k7", "doc": doc_with(bad) });
        let (s, a) = r.go(Member::Render, "rb", body).await;
        assert_eq!(s, 403, "{bad}: {a}");
    }
    let body = json!({ "at": "p.k7", "doc": doc_with(""), "base": "/etc" });
    assert_eq!(r.go(Member::Render, "rb", body).await.0, 403);
    assert!(r.machine.renders.lock().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_render_asks_for_a_doc_a_known_shape_and_a_machine() {
    let r = rig("vh-render-400", Fake::default()).await;
    let (s, a) = r.go(Member::Render, "rc", json!({ "at": "p.k7" })).await;
    assert_eq!(s, 400, "{a}");
    let body = json!({ "at": "p.k7", "doc": { "tempo": "fast" } });
    assert_eq!(r.go(Member::Render, "rc", body).await.0, 400);
    let body = json!({ "at": "p.k7", "doc": doc_with("") });
    let (s, a) = r.go(Member::Render, "rc", body).await;
    assert_eq!((s, a["code"].as_str()), (503, Some("no-machine")), "{a}");
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_stopped_job_resolves_the_machines_gone() {
    let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let waiting = tokio::spawn(drive::gone(flag.clone()));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!waiting.is_finished());
    flag.store(true, std::sync::atomic::Ordering::Relaxed);
    tokio::time::timeout(Duration::from_secs(2), waiting)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_peak_is_the_files_loudest_sample_in_dbfs_and_a_bad_file_is_said() {
    let r = rig("vh-peak", bound()).await;
    wav(
        &r.b.dir.join("home/workspace/songs/p.k7/loud.wav"),
        100,
        16384,
    );
    let (s, a) = r
        .go(Member::Peak, "pa", json!({ "file": "/w/p.k7/loud.wav" }))
        .await;
    assert_eq!(s, 200, "{a}");
    assert!((a["db"].as_f64().unwrap() + 6.0206).abs() < 1e-3, "{a}");
    wav(&r.b.dir.join("home/workspace/songs/p.k7/quiet.wav"), 100, 0);
    let (_, a) = r
        .go(Member::Peak, "pa", json!({ "file": "/w/p.k7/quiet.wav" }))
        .await;
    assert_eq!(a["db"], -100.0);
    let (s, _) = r
        .go(Member::Peak, "pa", json!({ "file": "/w/p.k7/none.wav" }))
        .await;
    assert_eq!(s, 404);
    std::fs::write(r.b.dir.join("home/workspace/songs/p.k7/empty.wav"), b"").unwrap();
    let (s, a) = r
        .go(Member::Peak, "pa", json!({ "file": "/w/p.k7/empty.wav" }))
        .await;
    assert_eq!(s, 500, "{a}");
    assert!(
        a["error"]
            .as_str()
            .unwrap()
            .starts_with("could not measure the peak of /w/p.k7/empty.wav")
    );
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

fn cut(source: &str) -> Value {
    json!({ "at": "p.k7", "source": source, "model": "/s/models/m.onnx", "outputs": ["drums"] })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_cut_answers_the_files_its_machine_wrote_into_the_jobs_own_folder() {
    let r = rig("vh-cut", bound()).await;
    r.put("p.k7/mix.wav");
    std::fs::create_dir_all(r.b.dir.join("home/models")).unwrap();
    std::fs::write(r.b.dir.join("home/models/m.onnx"), "m").unwrap();
    let (s, a) = r.go(Member::Separate, "sa", cut("/w/p.k7/mix.wav")).await;
    assert_eq!(s, 202, "{a}");
    let job = a["job"].as_str().unwrap().to_string();
    let end = r.ended("sa", &job).await;
    assert_eq!(end["state"], "done", "{end}");
    assert_eq!(end["value"]["files"][0]["out"], "drums.wav");
    assert_eq!(end["value"]["files"][0]["len"], 44 + 200);
    let seen = r.machine.cuts.lock().unwrap()[0].clone();
    assert!(
        seen["outDir"]
            .as_str()
            .unwrap()
            .ends_with(&format!(".job-out/{job}"))
    );
    assert!(seen["source"].as_str().unwrap().ends_with("p.k7/mix.wav"));
    assert!(seen["model"].as_str().unwrap().ends_with("models/m.onnx"));
    // Two asks of the same work are two jobs: neither reads the other's folder.
    let (_, b) = r.go(Member::Separate, "sa", cut("/w/p.k7/mix.wav")).await;
    assert_ne!(b["job"], a["job"]);
    assert_eq!(
        r.ended("sa", b["job"].as_str().unwrap()).await["state"],
        "done"
    );
    assert_eq!(r.machine.cuts.lock().unwrap().len(), 2);
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_cut_is_refused_before_a_job_for_a_path_an_output_or_a_missing_machine() {
    let r = rig("vh-cut-bad", bound()).await;
    for bad in ["/etc/passwd", "/w/../../etc/hosts"] {
        assert_eq!(r.go(Member::Separate, "sb", cut(bad)).await.0, 403, "{bad}");
    }
    let mut body = cut("/w/p.k7/project.k7x");
    body["model"] = json!("/etc/passwd");
    assert_eq!(r.go(Member::Separate, "sb", body).await.0, 403);
    for outputs in [
        json!(["../x"]),
        json!([]),
        json!("drums"),
        json!([1]),
        json!(["drums", "bass", "other", "vocals", "instrumental", "drums"]),
    ] {
        let mut body = cut("/w/p.k7/project.k7x");
        body["outputs"] = outputs.clone();
        assert_eq!(r.go(Member::Separate, "sb", body).await.0, 400, "{outputs}");
    }
    assert!(r.machine.cuts.lock().unwrap().is_empty());
    let none = rig("vh-cut-none", Fake::default()).await;
    let (s, a) = none
        .go(Member::Separate, "sb", cut("/w/p.k7/project.k7x"))
        .await;
    assert_eq!((s, a["code"].as_str()), (503, Some("no-machine")), "{a}");
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_cut_whose_machine_names_a_file_outside_its_folder_fails_the_job() {
    for (n, stray) in ["/etc/passwd", "{out}/sub/x.wav", "{out}/../x.wav"]
        .into_iter()
        .enumerate()
    {
        let machine = Fake {
            bound: true,
            stray: Some(stray.into()),
            ..Fake::default()
        };
        let r = rig(&format!("vh-cut-stray{n}"), machine).await;
        r.put("p.k7/mix.wav");
        std::fs::create_dir_all(r.b.dir.join("home/models")).unwrap();
        std::fs::write(r.b.dir.join("home/models/m.onnx"), "m").unwrap();
        let (_, a) = r.go(Member::Separate, "sc", cut("/w/p.k7/mix.wav")).await;
        let end = r.ended("sc", a["job"].as_str().unwrap()).await;
        assert_eq!(end["state"], "failed", "{stray}: {end}");
        assert!(
            end["error"].as_str().unwrap().contains("outside its cut"),
            "{stray}: {end}"
        );
        let _ = std::fs::remove_dir_all(&r.b.dir);
    }
}

fn stems_body(songs: Value) -> Value {
    json!({ "songs": songs, "steps": [{ "step": "split", "model": "/s/models/m.onnx", "outputs": ["drums"] }] })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_playlist_is_cut_record_by_record_with_one_state_each_inside_the_workspace() {
    let r = rig("vh-stems", bound()).await;
    std::fs::create_dir_all(r.b.dir.join("home/models")).unwrap();
    std::fs::write(r.b.dir.join("home/models/m.onnx"), "m").unwrap();
    let space = r.space();
    wav(&space.join("fresh/take.wav"), 100, 100);
    wav(&space.join("held/take.wav"), 100, 100);
    wav(&space.join("held/stems/drums.wav"), 100, 100);
    std::fs::create_dir_all(space.join("quiet")).unwrap();
    std::fs::write(space.join("quiet/notes.txt"), "x").unwrap();
    wav(&space.join("../outside/take.wav"), 100, 100);
    #[cfg(unix)]
    std::os::unix::fs::symlink(space.join("../outside"), space.join("linked")).unwrap();
    let names = json!([
        "fresh/take.wav",
        "held/take.wav",
        "quiet/notes.txt",
        "nowhere.wav",
        "../outside/take.wav",
        "/etc/take.wav",
        "fresh/../fresh/take.wav",
        "linked/take.wav",
        "fresh"
    ]);
    let (s, a) = r.go(Member::PlaylistStems, "pa", stems_body(names)).await;
    assert_eq!(s, 202, "{a}");
    let end = r.ended("pa", a["job"].as_str().unwrap()).await;
    assert_eq!(end["state"], "done", "{end}");
    let got: Vec<(&str, &str)> = end["value"]["songs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| (s["song"].as_str().unwrap(), s["state"].as_str().unwrap()))
        .collect();
    assert_eq!(
        got,
        [
            ("fresh/take.wav", "done"),
            ("held/take.wav", "held"),
            ("quiet/notes.txt", "absent"),
            ("nowhere.wav", "absent"),
            ("../outside/take.wav", "absent"),
            ("/etc/take.wav", "absent"),
            ("fresh/../fresh/take.wav", "absent"),
            ("linked/take.wav", "absent"),
            ("fresh", "absent"),
        ],
        "{end} {:?}",
        r.machine.cuts.lock().unwrap()
    );
    assert_eq!(end["value"]["songs"][0]["files"], json!(["drums.wav"]));
    // Only the fresh record reached the machine, and its stems went beside it.
    let cuts = r.machine.cuts.lock().unwrap().clone();
    assert_eq!(cuts.len(), 1);
    assert!(cuts[0]["outDir"].as_str().unwrap().ends_with("fresh/stems"));
    assert!(
        !end.to_string().contains(&r.b.dir.display().to_string()),
        "{end}"
    );
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_playlist_is_bounded_and_needs_a_machine_a_library_and_known_steps() {
    let r = rig("vh-stems-bad", bound()).await;
    let many: Vec<String> = (0..101).map(|i| format!("s{i}")).collect();
    assert_eq!(
        r.go(Member::PlaylistStems, "pb", stems_body(json!(many)))
            .await
            .0,
        400
    );
    assert_eq!(
        r.go(Member::PlaylistStems, "pb", stems_body(json!([1])))
            .await
            .0,
        400
    );
    assert_eq!(
        r.go(Member::PlaylistStems, "pb", json!({ "songs": ["a"] }))
            .await
            .0,
        400
    );
    let mut body = stems_body(json!(["a"]));
    body["steps"][0]["model"] = json!("/etc/passwd");
    assert_eq!(r.go(Member::PlaylistStems, "pb", body).await.0, 403);
    let mut body = stems_body(json!(["a"]));
    body["steps"][0]["outputs"] = json!(["nope"]);
    assert_eq!(r.go(Member::PlaylistStems, "pb", body).await.0, 400);
    let none = rig("vh-stems-none", Fake::default()).await;
    let (s, a) = none
        .go(Member::PlaylistStems, "pb", stems_body(json!(["a"])))
        .await;
    assert_eq!((s, a["code"].as_str()), (503, Some("no-machine")), "{a}");
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_member_whose_piece_is_off_and_a_front_without_jobs_answer_503_off() {
    let r = rig("vh-off-members", bound()).await;
    // The engine's members need the engine link, which this front has none of.
    let mut ctx = r.b.ctx(Member::Plugins, false);
    ctx.verbs = super::account::with(drive::production(Some(r.jobs())), None);
    let (s, a) = r.b.send(Member::Plugins, "", json!({}), Some(ctx)).await;
    assert_eq!((s, a["code"].as_str()), (503, Some("off")), "{a}");
    assert!(drive::production(None).is_none());
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_job_that_failed_leaves_no_files_and_frees_its_place_but_a_done_one_keeps_them() {
    let failing = Fake {
        bound: true,
        failing: true,
        ..Fake::default()
    };
    let r = rig("vh-clean", failing).await;
    let body = json!({ "at": "p.k7", "doc": doc_with("") });
    let (_, a) = r.go(Member::Render, "ca", body.clone()).await;
    let job = a["job"].as_str().unwrap().to_string();
    let end = r.ended("ca", &job).await;
    assert_eq!(end["state"], "failed", "{end}");
    let out = r.b.dir.join("home/.job-out").join(&job);
    // The record ends before the driver's hold is let go and its files removed: wait for both.
    for _ in 0..1500 {
        if Table::global().running("ca") == 0 && !out.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(Table::global().running("ca"), 0);
    assert!(!out.exists(), "a failed job's files are nobody's");
    let ok = rig("vh-clean-ok", bound()).await;
    let (_, a) = ok.go(Member::Render, "cb", body).await;
    let job = a["job"].as_str().unwrap().to_string();
    assert_eq!(ok.ended("cb", &job).await["state"], "done");
    assert!(
        ok.b.dir
            .join("home/.job-out")
            .join(&job)
            .join("mix.wav")
            .exists()
    );
    let _ = std::fs::remove_dir_all(&r.b.dir);
    let _ = std::fs::remove_dir_all(&ok.b.dir);
}
