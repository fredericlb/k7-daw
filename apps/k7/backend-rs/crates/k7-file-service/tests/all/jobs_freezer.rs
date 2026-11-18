//! jobs_freezer.rs — the freezer machine's own rules, and the shapes B1 added (2312 to 2316); the TypeScript's traces replay in jobs_freezer_traces.rs.
//! A reader gets wrong that a stray answer changes nothing and that a pass ends with exactly one `Done`, whatever happened in it.

use k7_file_service::jobs::freezer::Freezer;
use k7_file_service::jobs::{AutoFreeze, FreezeEffect as Fx, FreezeEvent as Ev, THAW_IDLE};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

pub fn freeze(module: &str, ready: bool, hosted: bool, plugins: &[&str], dir: &str) -> AutoFreeze {
    let render = format!(
        "/scratch/r/{}-{module}.wav",
        dir.replace(|c: char| !c.is_ascii_alphanumeric(), "_")
    );
    let plugins: Vec<Value> = plugins
        .iter()
        .map(|n| json!({ "format": "VST3", "name": n }))
        .collect();
    serde_json::from_value(json!({
        "module": module, "render": render, "ready": ready, "hosted": hosted, "plugins": plugins,
        "plan": { "at": module, "render": render, "lanes": [], "modules": [] },
        "cut": serde_json::to_value(song::doc::ProjectDoc::empty()).unwrap(),
    }))
    .unwrap()
}

fn ev_tick(rev: &str, playing: bool, now: Instant) -> Ev {
    Ev::Tick {
        rev: rev.into(),
        playing,
        now,
    }
}

#[test]
fn a_render_failing_logs_its_sentence_and_a_pass_that_cannot_compile_ends_with_one() {
    let mut f = Freezer::new();
    let now = Instant::now();
    assert_eq!(f.on("/p", ev_tick("r1", false, now)), [Fx::Compile]);
    let a = freeze("a", false, true, &["a"], "/p");
    let fx = f.on(
        "/p",
        Ev::Compiled {
            freezes: vec![a.clone()],
            playing: false,
        },
    );
    assert_eq!(fx, [Fx::Render(a)]);
    let fx = f.on(
        "/p",
        Ev::Rendered {
            module: "a".into(),
            made: false,
            error: Some("engine render failed (exit 1)".into()),
        },
    );
    assert_eq!(
        fx,
        [
            Fx::Log("   ✗ auto-freeze a: engine render failed (exit 1)".into()),
            Fx::Done(vec![])
        ]
    );
    // The revision is looked at: a failed render is not tried again until the project changes (2316).
    assert_eq!(f.on("/p", ev_tick("r1", false, now)), [Fx::Done(vec![])]);
    assert_eq!(f.on("/p", ev_tick("r2", false, now)), [Fx::Compile]);
    assert_eq!(
        f.on("/p", Ev::CompileFailed("no such project".into())),
        [
            Fx::Log("   ✗ auto-freeze: no such project".into()),
            Fx::Done(vec![])
        ]
    );
    assert!(!f.running("/p"));
}

#[test]
fn the_second_read_of_playing_after_the_compile_decides_a_reload_now_or_at_the_stop() {
    let now = Instant::now();
    let second_pass = |playing: bool| {
        let mut f = Freezer::new();
        let a = freeze("a", true, false, &[], "/p");
        let b = freeze("b", true, false, &[], "/p");
        f.on("/p", ev_tick("r1", false, now));
        f.on(
            "/p",
            Ev::Compiled {
                freezes: vec![a.clone()],
                playing: false,
            },
        );
        // Another engine bound: one more render is played, which changes the set the compile applied.
        f.on("/p", ev_tick("r2", false, now));
        let fx = f.on(
            "/p",
            Ev::Compiled {
                freezes: vec![a, b],
                playing,
            },
        );
        (f, fx)
    };
    let (_, fx) = second_pass(false);
    assert_eq!(
        fx,
        [Fx::Reload, Fx::Done(vec![])],
        "stopped: the reload is now"
    );
    let (mut f, fx) = second_pass(true);
    assert_eq!(
        fx,
        [Fx::Done(vec![])],
        "playing: nothing under the playhead"
    );
    // The first tick after the stop swaps it in before it looks at the revision.
    assert_eq!(
        f.on("/p", ev_tick("r2", false, now)),
        [Fx::Reload, Fx::Compile]
    );
}

#[test]
fn renderall_reports_progress_the_impossible_and_a_failed_compile() {
    let mut f = Freezer::new();
    let (a, c, d) = (
        freeze("a", false, true, &["a"], "/p"),
        freeze("c", false, false, &["P1", "P2"], "/p"),
        freeze("d", true, true, &["d"], "/p"),
    );
    assert_eq!(f.on("/p", Ev::RenderAll), [Fx::Compile]);
    let fx = f.on(
        "/p",
        Ev::Compiled {
            freezes: vec![a.clone(), c, d],
            playing: false,
        },
    );
    assert_eq!(
        fx,
        [
            Fx::Progress {
                done: 0,
                total: 1,
                module: "a".into()
            },
            Fx::Render(a)
        ]
    );
    let fx = f.on(
        "/p",
        Ev::Rendered {
            module: "a".into(),
            made: false,
            error: Some("boom".into()),
        },
    );
    assert_eq!(fx[0], Fx::Log("   ✗ auto-freeze a: boom".into()));
    assert_eq!(
        fx[1],
        Fx::Progress {
            done: 1,
            total: 1,
            module: String::new()
        }
    );
    assert_eq!(
        fx[2],
        Fx::Impossible(vec![
            ("c".into(), vec!["P1".into(), "P2".into()]),
            ("a".into(), vec!["a".into()])
        ])
    );
    // The share's compile failing is its error, and the folder is free again.
    f.on("/p", Ev::RenderAll);
    assert_eq!(
        f.on("/p", Ev::CompileFailed("no such project".into())),
        [Fx::RenderAllFailed("no such project".into())]
    );
    assert!(!f.running("/p"));
}

#[test]
fn a_renderall_during_a_pass_waits_for_it_and_two_of_them_run_one_after_the_other() {
    let mut f = Freezer::new();
    let now = Instant::now();
    f.on("/p", ev_tick("r1", false, now));
    let a = freeze("a", false, true, &["a"], "/p");
    f.on(
        "/p",
        Ev::Compiled {
            freezes: vec![a],
            playing: false,
        },
    );
    assert!(f.running("/p"));
    assert!(f.on("/p", Ev::RenderAll).is_empty());
    assert!(f.on("/p", Ev::RenderAll).is_empty());
    let fx = f.on(
        "/p",
        Ev::Rendered {
            module: "a".into(),
            made: true,
            error: None,
        },
    );
    assert_eq!(
        &fx[..3],
        &[
            Fx::Log("   ❄ auto-froze a".into()),
            Fx::Done(vec!["a".into()]),
            Fx::Compile
        ]
    );
    // The first renderAll answers, then the second starts: never two renders of one freeze at once.
    let fx = f.on(
        "/p",
        Ev::Compiled {
            freezes: vec![],
            playing: false,
        },
    );
    assert_eq!(fx.last(), Some(&Fx::Compile));
    assert!(f.running("/p"));
}

#[test]
fn a_stray_answer_changes_nothing() {
    let mut f = Freezer::new();
    let a = freeze("a", false, true, &["a"], "/p");
    assert!(
        f.on(
            "/p",
            Ev::Compiled {
                freezes: vec![a.clone()],
                playing: false
            }
        )
        .is_empty()
    );
    assert!(
        f.on(
            "/p",
            Ev::Rendered {
                module: "a".into(),
                made: true,
                error: None
            }
        )
        .is_empty()
    );
    assert!(f.on("/p", Ev::CompileFailed("x".into())).is_empty());
    f.on("/p", ev_tick("r1", false, Instant::now()));
    f.on(
        "/p",
        Ev::Compiled {
            freezes: vec![a.clone()],
            playing: false,
        },
    );
    // An answer for a module no render was asked for is not the answer that is waited for.
    assert!(
        f.on(
            "/p",
            Ev::Rendered {
                module: "zz".into(),
                made: true,
                error: None
            }
        )
        .is_empty()
    );
    assert!(f.running("/p"));
    assert_eq!(
        f.on(
            "/p",
            Ev::Rendered {
                module: "a".into(),
                made: true,
                error: None
            }
        )
        .len(),
        2
    );
}

#[test]
fn the_idle_clock_frees_a_thaw_at_twenty_seconds_and_not_a_tick_before() {
    let mut f = Freezer::new();
    let t0 = Instant::now();
    f.on(
        "/p",
        Ev::Thaw {
            module: "a".into(),
            on: true,
            now: t0,
        },
    );
    f.on("/p", ev_tick("r1", false, t0));
    f.on(
        "/p",
        Ev::Compiled {
            freezes: vec![],
            playing: false,
        },
    );
    let early = t0 + THAW_IDLE - Duration::from_millis(1);
    assert_eq!(f.on("/p", ev_tick("r1", false, early)), [Fx::Done(vec![])]);
    assert_eq!(f.thawed("/p"), ["a"]);
    let fx = f.on("/p", ev_tick("r1", false, t0 + THAW_IDLE));
    assert_eq!(&fx[..2], &[Fx::SetThawed(vec![]), Fx::Reload]);
    assert!(f.thawed("/p").is_empty());
}

#[test]
fn a_thawed_list_of_two_thousand_keeps_its_order_and_a_thousand_freezes_render_one_by_one() {
    let mut f = Freezer::new();
    let now = Instant::now();
    for i in 0..2000 {
        f.on(
            "/p",
            Ev::Thaw {
                module: format!("m{i}"),
                on: true,
                now,
            },
        );
    }
    for i in (0..2000).step_by(2) {
        f.on(
            "/p",
            Ev::Thaw {
                module: format!("m{i}"),
                on: false,
                now,
            },
        );
    }
    let want: Vec<String> = (1..2000).step_by(2).map(|i| format!("m{i}")).collect();
    assert_eq!(f.thawed("/p"), want);
    let mut g = Freezer::new();
    g.on("/q", ev_tick("r1", false, now));
    let all: Vec<AutoFreeze> = (0..1000)
        .map(|i| freeze(&format!("f{i}"), false, true, &["p"], "/q"))
        .collect();
    let mut fx = g.on(
        "/q",
        Ev::Compiled {
            freezes: all,
            playing: false,
        },
    );
    let mut rendered = 0;
    while let Some(Fx::Render(r)) = fx.first().cloned() {
        assert_eq!(fx.len(), 1, "one render at a time");
        rendered += 1;
        fx = g.on(
            "/q",
            Ev::Rendered {
                module: r.module,
                made: true,
                error: None,
            },
        );
    }
    assert_eq!(rendered, 1000);
    assert!(matches!(fx.last(), Some(Fx::Done(done)) if done.len() == 1000));
}

#[test]
fn a_thaw_that_lands_while_the_pass_compiles_leaves_the_revision_unlooked_at() {
    let mut f = Freezer::new();
    let now = Instant::now();
    assert_eq!(f.on("/p", ev_tick("r1", false, now)), [Fx::Compile]);
    // The compile runs on a blocking thread, so a route's thaw comes between the tick and its answer.
    let fx = f.on(
        "/p",
        Ev::Thaw {
            module: "a".into(),
            on: false,
            now,
        },
    );
    assert_eq!(fx, [Fx::SetThawed(vec![])]);
    let a = freeze("a", true, true, &["a"], "/p");
    f.on(
        "/p",
        Ev::Compiled {
            freezes: vec![a],
            playing: false,
        },
    );
    // The next tick, on the same revision, compiles again: the thaw changed what freezes.
    assert_eq!(f.on("/p", ev_tick("r1", false, now)), [Fx::Compile]);
    f.on(
        "/p",
        Ev::Compiled {
            freezes: vec![],
            playing: false,
        },
    );
    // With nothing in between, the revision stays looked at.
    assert_eq!(f.on("/p", ev_tick("r1", false, now)), [Fx::Done(vec![])]);
}
