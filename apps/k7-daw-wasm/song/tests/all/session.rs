// session.rs — one workspace's cache and answers (decisions 703 to 708, 751): what is held, what keys it, and when it stands.
// A reader would get wrong that `rev` keys an answer: it only spares the stat of a read set, and a different request is a different slot.

use crate::doc_support;
use crate::support;

use doc_support::{AMP, laid, roots};
use serde_json::{Value, json};
use song::doc::Vsts;
use song::graph::Folder;
use song::session::{Ask, MAX_ANSWERS, Session, View};
use song::{Entry, MemSource, Source, SourceError, Span, Stat};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

const MODULES: &str = "modules({ gtr: [\"amp\"], amp: [\"MIX\"] })\n";

fn files(lane_text: &str) -> Vec<(String, String)> {
    laid(&[
        ("nodes/gtr.k7x", lane_text),
        ("nodes/amp.k7x", AMP),
        ("graph.k7x", MODULES),
    ])
}

fn world(lane_text: &str) -> MemSource {
    let owned = files(lane_text);
    let refs: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    support::mem(&refs)
}

fn ask() -> Ask {
    Ask {
        roots: roots(),
        rig: json!({}),
        vsts: Vsts::Scan,
        thawed: Vec::new(),
        rev: None,
        folder: Folder::Open,
    }
}

/// A world that can change under a session, and counts the stats it is asked.
struct Live {
    now: Mutex<MemSource>,
    stats: AtomicUsize,
    settled: bool,
    /// A path whose stamp reads as moved, though its text is the same.
    bumped: Mutex<Option<String>>,
}

impl Live {
    fn new(lane_text: &str) -> Live {
        Live {
            now: Mutex::new(world(lane_text)),
            stats: AtomicUsize::new(0),
            settled: true,
            bumped: Mutex::new(None),
        }
    }

    fn set(&self, lane_text: &str) {
        *self.now.lock().expect("lock") = world(lane_text);
    }

    fn set_files(&self, owned: &[(String, String)]) {
        let refs: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        *self.now.lock().expect("lock") = support::mem(&refs);
    }

    fn with_files(owned: &[(String, String)]) -> Live {
        let live = Live::new("");
        live.set_files(owned);
        live
    }

    fn stats(&self) -> usize {
        self.stats.swap(0, Ordering::SeqCst)
    }
}

impl Source for Live {
    fn stat(&self, path: &str) -> Result<Stat, SourceError> {
        self.stats.fetch_add(1, Ordering::SeqCst);
        let mut st = self.now.lock().expect("lock").stat(path)?;
        st.stamp.settled = self.settled;
        if self.bumped.lock().expect("lock").as_deref() == Some(path) {
            st.stamp.mtime_ns ^= 0x5eed;
        }
        Ok(st)
    }

    fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError> {
        self.now.lock().expect("lock").list(dir)
    }

    fn read(&self, path: &str, span: Span) -> Result<Vec<u8>, SourceError> {
        self.now.lock().expect("lock").read(path, span)
    }
}

fn text(reply: &song::session::Reply, view: View) -> &str {
    reply
        .views
        .iter()
        .find(|(v, _)| *v == view)
        .map(|(_, t)| &**t)
        .unwrap_or_else(|| {
            panic!(
                "no {view:?} in {:?}",
                reply.views.iter().map(|v| v.0).collect::<Vec<_>>()
            )
        })
}

fn json_of(reply: &song::session::Reply, view: View) -> Value {
    serde_json::from_str(text(reply, view)).expect("json")
}

const CORE: [View; 3] = [View::Engine, View::Web, View::Errors];

// 705: the engine, the web project and the errors come from one compile, always together, in the order asked.
#[test]
fn the_core_views_come_from_one_compile_in_the_order_asked() {
    let live = Live::new("extends(\"@meta/track\")\n");
    let mut s = Session::new(song::PathRule::HOST);
    let r = s
        .answer(&live, &ask(), &[View::Errors, View::Engine])
        .expect("an answer");
    assert_eq!(
        r.views.iter().map(|v| v.0).collect::<Vec<_>>(),
        [View::Errors, View::Engine]
    );
    assert!(!r.reused);
    assert!(json_of(&r, View::Engine)["doc"]["tracks"].is_array());
    // The web project was made with them: asking for it now holds.
    let again = s.answer(&live, &ask(), &[View::Web]).expect("an answer");
    assert!(again.reused, "the web project was made by the first compile");
    assert!(json_of(&again, View::Web)["tracks"].is_array());
}

// 706: an equal token answers with no stat at all; without one, a read set that is settled and valid answers.
#[test]
fn an_equal_rev_costs_no_stat_and_a_valid_read_set_costs_stats_but_no_compile() {
    let live = Live::new("extends(\"@meta/track\")\n");
    let mut s = Session::new(song::PathRule::HOST);
    let mut a = ask();
    a.rev = Some("r1".to_owned());
    let first = s.answer(&live, &a, &CORE).expect("an answer");
    live.stats();
    let second = s.answer(&live, &a, &CORE).expect("an answer");
    assert!(second.reused);
    assert_eq!(live.stats(), 0, "an equal rev stats nothing");
    assert_eq!(text(&second, View::Engine), text(&first, View::Engine));
    a.rev = Some("r2".to_owned());
    let third = s.answer(&live, &a, &CORE).expect("an answer");
    assert!(third.reused, "nothing moved");
    assert!(live.stats() > 0, "a new rev checks the read set");
    a.rev = None;
    assert!(s.answer(&live, &a, &CORE).expect("an answer").reused);
}

// 706: a file that moved remakes the answer, whatever the order things were asked in.
#[test]
fn a_changed_file_remakes_the_answer_and_a_moved_rev_alone_does_not_stand() {
    let live = Live::new("extends(\"@meta/track\")\n");
    let mut s = Session::new(song::PathRule::HOST);
    let before = s.answer(&live, &ask(), &CORE).expect("an answer");
    live.set("extends(\"@meta/track\")\ncolor(\"#ff0000\")\n");
    let after = s.answer(&live, &ask(), &CORE).expect("an answer");
    assert!(!after.reused);
    assert_ne!(text(&after, View::Engine), text(&before, View::Engine));
    // The same token over a changed world would be the host's lie; the read set is what is checked without one.
    let again = s.answer(&live, &ask(), &CORE).expect("an answer");
    assert!(again.reused);
}

// 706: a read set with an unsettled stamp is never reused without a token.
#[test]
fn an_unsettled_file_is_not_reused_without_a_rev() {
    let mut live = Live::new("extends(\"@meta/track\")\n");
    live.settled = false;
    let mut s = Session::new(song::PathRule::HOST);
    s.answer(&live, &ask(), &CORE).expect("an answer");
    assert!(!s.answer(&live, &ask(), &CORE).expect("an answer").reused);
    let mut a = ask();
    a.rev = Some("same".to_owned());
    s.answer(&live, &a, &CORE).expect("an answer");
    assert!(
        s.answer(&live, &a, &CORE).expect("an answer").reused,
        "the host's token stands"
    );
}

// 707: the roots, the rig, the VSTs, the thawed modules and the folder key an answer; the token never does.
#[test]
fn a_request_that_differs_is_not_a_hit_whatever_the_rev() {
    let live = Live::new("extends(\"@meta/track\")\n");
    let mut s = Session::new(song::PathRule::HOST);
    let mut base = ask();
    base.rev = Some("r".to_owned());
    s.answer(&live, &base, &CORE).expect("an answer");
    let mut variants: Vec<Ask> = Vec::new();
    let mut a = base.clone();
    a.roots.project = "/p/ws/other".to_owned();
    variants.push(a);
    let mut a = base.clone();
    a.rig = json!({"channels": {"in": []}});
    variants.push(a);
    let mut a = base.clone();
    a.vsts = Vsts::None;
    variants.push(a);
    let mut a = base.clone();
    a.thawed = vec!["amp".to_owned()];
    variants.push(a);
    let mut a = base.clone();
    a.folder = Folder::Other;
    variants.push(a);
    for v in &variants {
        // A project that is not there answers a failure, which is still a made answer and not the first one's.
        let r = s.answer(&live, v, &[View::Engine]).expect("an answer");
        assert!(!r.reused, "{:?}", v.roots.project);
        let back = s.answer(&live, &base, &CORE).expect("an answer");
        assert!(back.reused, "the first is still held after {:?}", v.roots.project);
    }
}

// 707: at most MAX_ANSWERS are held, the least recently asked dropped; a hundred rigs are held to four.
#[test]
fn a_hundred_requests_of_different_rigs_are_held_to_four() {
    let live = Live::new("extends(\"@meta/track\")\n");
    let mut s = Session::new(song::PathRule::HOST);
    let rig = |i: usize| json!({ "channels": { "in": [format!("in{i}")] } });
    let mut a = ask();
    for i in 0..100 {
        a.rig = rig(i);
        s.answer(&live, &a, &[View::Engine]).expect("an answer");
    }
    assert_eq!(s.held(), MAX_ANSWERS);
    a.rig = rig(0);
    assert!(
        !s.answer(&live, &a, &[View::Engine]).expect("an answer").reused,
        "the first went"
    );
    a.rig = rig(99);
    assert!(
        s.answer(&live, &a, &[View::Engine]).expect("an answer").reused,
        "the last stayed"
    );
}

// 708: a session weighs its cache and its answers, and a shed or a forget lets them go.
#[test]
fn a_session_weighs_what_it_holds_and_lets_go_of_it() {
    let live = Live::new("extends(\"@meta/track\")\n");
    let mut s = Session::new(song::PathRule::HOST);
    assert_eq!(s.bytes(), 0);
    s.answer(&live, &ask(), &CORE).expect("an answer");
    let held = s.bytes();
    assert!(held > 1_000, "{held}");
    s.shed();
    assert!(s.bytes() < held);
    let r = s.answer(&live, &ask(), &CORE).expect("an answer after a shed");
    assert!(!r.reused);
    s.forget();
    assert_eq!(s.held(), 0);
}

// 705, 726: the graph asks the compile, then draws over it; a project that does not compile still draws.
#[test]
fn the_graph_draws_over_the_compile_and_a_failed_compile_draws_the_plans_it_has() {
    let live = Live::new("extends(\"@meta/track\")\n");
    let mut s = Session::new(song::PathRule::HOST);
    let r = s.answer(&live, &ask(), &[View::Graph]).expect("an answer");
    let g = json_of(&r, View::Graph);
    assert!(g["chains"].is_object(), "{g}");
    assert!(!r.reused);
    let both = s
        .answer(&live, &ask(), &[View::Graph, View::Engine])
        .expect("an answer");
    assert!(both.reused, "the core was made for the graph");
    // A lane that wants a clip it does not have stops the compile, and the graph is still drawn.
    live.set("extends(\"@meta/track\")\nevents([{ ref: \"ghost\", start: \"1:1\" }])\n");
    let broken = s
        .answer(&live, &ask(), &[View::Engine, View::Graph])
        .expect("an answer");
    assert!(json_of(&broken, View::Engine)["failure"]["diagnostics"].is_array());
    assert!(json_of(&broken, View::Graph)["chains"].is_object());
}

// 705: the other views are one call each, and each is the view the route answers.
#[test]
fn each_other_view_is_the_route_answer() {
    let live = Live::new("extends(\"@meta/track\")\n");
    let mut s = Session::new(song::PathRule::HOST);
    let r = s
        .answer(
            &live,
            &ask(),
            &[
                View::Hints,
                View::Surfaces,
                View::IoPorts,
                View::Catalogue,
                View::BaseModules,
            ],
        )
        .expect("an answer");
    assert!(json_of(&r, View::Hints)["modules"].is_array());
    assert!(json_of(&r, View::Surfaces)["surfaces"].is_array());
    assert!(json_of(&r, View::IoPorts).is_array());
    assert!(json_of(&r, View::Catalogue).is_array());
    assert!(json_of(&r, View::BaseModules).is_array());
    assert!(s.answer(&live, &ask(), &[View::Catalogue]).expect("an answer").reused);
}

// 751: a bundle of the compile replays to the same engine document and errors; the web project is drawn through its table (611).
#[test]
fn a_bundle_replays_to_the_same_core() {
    let live = Live::new("extends(\"@meta/track\")\n");
    let mut s = Session::new(song::PathRule::HOST);
    let first = s.answer(&live, &ask(), &CORE).expect("an answer");
    let bundle = s.bundle(&live, &ask()).expect("a bundle");
    assert!(bundle.request.is_some());
    let replay = MemSource::from_bundle(bundle).expect("a source");
    let mut other = Session::new(song::PathRule::HOST);
    let again = other
        .answer(&replay, &ask(), &[View::Engine, View::Errors])
        .expect("an answer over the bundle");
    assert_eq!(text(&again, View::Engine), text(&first, View::Engine));
    assert_eq!(text(&again, View::Errors), text(&first, View::Errors));
}

// 717: a difference is not logged when something it read moved after the time the TypeScript started.
#[test]
fn a_read_set_that_moved_after_a_time_is_said_to_have_moved() {
    struct Timed<'a>(&'a Live, u64);
    impl Source for Timed<'_> {
        fn stat(&self, path: &str) -> Result<Stat, SourceError> {
            let mut st = self.0.stat(path)?;
            st.stamp.mtime_ns = self.1;
            Ok(st)
        }
        fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError> {
            self.0.list(dir)
        }
        fn read(&self, path: &str, span: Span) -> Result<Vec<u8>, SourceError> {
            self.0.read(path, span)
        }
        fn stamps_are_times(&self) -> bool {
            true
        }
    }
    let live = Live::new("extends(\"@meta/track\")\n");
    let mut s = Session::new(song::PathRule::HOST);
    let at = Timed(&live, 5_000_000_000);
    s.answer(&at, &ask(), &CORE).expect("an answer");
    assert!(
        !s.moved_since(&at, &ask(), View::Engine, 5_000_000_000),
        "not after the time"
    );
    assert!(s.moved_since(&at, &ask(), View::Engine, 4_000_000_000), "after it");
    let mut other = ask();
    other.rig = json!({"x": 1});
    assert!(
        !s.moved_since(&at, &other, View::Engine, 0),
        "an answer never held moved nothing"
    );
}

// 703: the session names no host: any Source serves it, and a Source that cannot say a path is the Err, not an answer.
#[test]
fn a_bundle_that_lacks_a_file_is_missing_and_not_a_wrong_answer() {
    let bare = song::SourceBundle {
        format: 1,
        mounts: vec!["/p".to_owned()],
        ..Default::default()
    };
    let src = MemSource::from_bundle(bare).expect("a source");
    let mut s = Session::new(song::PathRule::HOST);
    assert_eq!(s.answer(&src, &ask(), &CORE).map(|_| ()), Err(SourceError::Missing));
}

fn jsdsp_files(cache: Option<&str>) -> Vec<(String, String)> {
    let mut all = vec![
        ("nodes/gtr.k7x", "extends(\"@meta/track\")\n"),
        ("nodes/dsp.k7x", "plugin({ type: \"jsdsp\", name: \"dsp\" })\n"),
        ("graph.k7x", "modules({ gtr: [\"dsp\"], dsp: [\"MIX\"] })\n"),
        ("nodes/jsdsp.ts", "export {}\n"),
    ];
    if let Some(text) = cache {
        all.push(("nodes/jsdsp.json", text));
    }
    laid(&all)
}

// 705: a graph is drawn from the compile, so it is held only over the compile it was drawn over.
#[test]
fn a_graph_is_remade_when_the_compile_it_was_drawn_over_is() {
    let live = Live::with_files(&jsdsp_files(Some("{\"params\":[],\"nodes\":[]}\n")));
    let mut s = Session::new(song::PathRule::HOST);
    let mut a = ask();
    a.rev = Some("r1".to_owned());
    s.answer(&live, &a, &[View::Graph]).expect("an answer");
    // Only the compile reads the graph file's stamp: it moves, and the compile is made again.
    *live.bumped.lock().expect("lock") = Some("/p/ws/proj/nodes/jsdsp.json".to_owned());
    a.rev = Some("r2".to_owned());
    let core = s.answer(&live, &a, &[View::Engine]).expect("an answer");
    assert!(!core.reused, "the compile read a file that moved");
    let graph = s.answer(&live, &a, &[View::Graph]).expect("an answer");
    assert!(!graph.reused, "the graph was drawn over the compile that has gone");
    assert!(s.answer(&live, &a, &[View::Graph]).expect("an answer").reused);
}

// 708: the compile a session keeps weighs about three times its text, which is what a real project's resident size measured.
#[test]
fn a_held_compile_weighs_three_times_its_text() {
    let live = Live::new("extends(\"@meta/track\")\n");
    let mut s = Session::new(song::PathRule::HOST);
    let r = s.answer(&live, &ask(), &CORE).expect("an answer");
    let texts: usize = CORE.iter().map(|v| text(&r, *v).len()).sum();
    assert!(
        s.bytes() >= texts + 2 * text(&r, View::Engine).len(),
        "{} against {texts}",
        s.bytes()
    );
}

// 259, 271: a placed sequence that did not load is named by its own file, with the cause the file gives.
#[test]
fn a_broken_sequence_is_named_by_its_file_and_its_cause() {
    let owned = laid(&[
        (
            "nodes/melody.k7x",
            "extends(\"@meta/track\")\nevents([ref(\"melody\").start(\"1:1:1\")])\n",
        ),
        ("sequences/melody.k7x", "notes([\n  t(0).d(1).k(60).v(100),\n])\n"),
        ("graph.k7x", "modules({ melody: [\"MIX\"] })\n"),
    ]);
    let live = Live::with_files(&owned);
    let mut s = Session::new(song::PathRule::HOST);
    let r = s.answer(&live, &ask(), &[View::Engine]).expect("an answer");
    let d = &json_of(&r, View::Engine)["failure"]["diagnostics"];
    let first = &d[0];
    assert_eq!(first["file"], "sequences/melody.k7x", "{d}");
    let message = first["message"].as_str().unwrap_or("");
    assert!(
        message.contains("sequence 'melody' failed to load — sequences/melody.k7x: "),
        "{message}"
    );
    assert!(
        message.len() > "sequence 'melody' failed to load — sequences/melody.k7x: ".len() + 20,
        "{message}"
    );
}

// 2446: a node's model need is asked of the session and read fresh: a model that arrives is seen at the next ask.
#[test]
fn a_nodes_model_need_is_read_fresh_at_every_ask() {
    use song::graph::ModelNeed;
    let net = "plugin({ type: \"offline\", name: \"Denoise\" })\nparams({ model: \"net.onnx\" })\nmodel_url(\"https://example.com/net.onnx\")\n";
    let base = [("nodes/n.k7x", net), ("graph.k7x", "modules({ n: [\"OUT\"] })\n")];
    let laid_in = |extra: &[(&str, &str)]| {
        let mut all = base.to_vec();
        all.extend_from_slice(extra);
        support::mem(
            &laid(&all)
                .iter()
                .map(|(a, b)| (a.as_str(), b.as_str()))
                .collect::<Vec<_>>(),
        )
    };
    let mut session = Session::new(song::PathRule::HOST);
    let lacking = session.model_need(&laid_in(&[]), &ask(), "n").expect("graph");
    assert!(matches!(lacking, ModelNeed::Needs { url: Some(_), .. }), "{lacking:?}");
    let here = laid_in(&[("nodes/assets/net.onnx", "x")]);
    assert_eq!(session.model_need(&here, &ask(), "n").expect("graph"), ModelNeed::Have);
    assert_eq!(
        session.model_need(&here, &ask(), "ghost").expect("graph"),
        ModelNeed::NoNode
    );
}
