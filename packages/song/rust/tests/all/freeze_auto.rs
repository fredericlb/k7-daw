// freeze_auto.rs — the automatic freezes: where they are taken, the render's name, and which engine plays it (autoFreeze.test.ts, 339 to 348).
// A reader would get wrong that nothing is rendered or touched here: a render is a stat, and the host alone makes and marks one.

use crate::freeze_support;
use crate::support;

use freeze_support::{Scene, chain, chains, doc, plugin_module, scan_of, track, vst};
use song::clips::Budget;
use song::freeze::{AutoFreeze, BUDGET, FreezePlan, auto_render_path, is_vst_module, render_ready};
use song::modules::ModuleKind;
use song::{Cache, DiskSource, MemSource, SourceError};
use support::{Tmp, spell};

fn budget() -> Budget {
    Budget::new(BUDGET)
}

fn world(t: &Tmp) -> DiskSource {
    DiskSource::new(&[spell(&t.0)]).expect("a source")
}

fn freezes(s: &Scene, t: &Tmp) -> Vec<AutoFreeze> {
    s.run(&world(t), &mut Cache::new(), &mut budget()).freezes
}

fn scene(t: &Tmp, cab_vst: bool) -> Scene {
    Scene::standard(&t.at("proj"), Some(&t.at("renders")), cab_vst)
}

fn of<'a>(f: &'a [AutoFreeze], module: &str) -> &'a AutoFreeze {
    f.iter().find(|x| x.module == module).expect("a freeze at the module")
}

fn make(f: &AutoFreeze) {
    let path = std::path::Path::new(&f.render);
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("folder");
    std::fs::write(path, "").expect("render");
}

#[test]
fn a_point_is_taken_at_each_vst_when_nothing_downstream_of_it_is_one() {
    // autoFreeze.test.ts "at each VST, when nothing downstream of it is one"; decision 340
    let s = Scene::standard("/p", None, false);
    assert_eq!(s.points(&mut budget()), Some(vec!["amp".to_owned(), "verb".to_owned()]));
}

#[test]
fn a_point_is_taken_at_the_last_vst_of_a_run() {
    // autoFreeze.test.ts "at the LAST VST of a run"; decision 340
    let s = Scene::standard("/p", None, true);
    assert_eq!(s.points(&mut budget()), Some(vec!["cab".to_owned(), "verb".to_owned()]));
}

#[test]
fn no_point_where_the_lane_also_goes_out_on_its_own() {
    // autoFreeze.test.ts "not where the lane also goes out on its own"; decision 341
    let mut s = Scene::standard("/p", None, false);
    s.chains = chains(vec![
        ("gtr", chain(Some("gtr"), &[], &["amp", "OUT"])),
        ("amp", chain(None, &["amp", "cab"], &["OUT"])),
    ]);
    assert_eq!(s.points(&mut budget()), Some(vec![]));
}

#[test]
fn two_runs_into_one_bus_are_one_point_at_the_bus() {
    // decision 340: the bus's VST has both runs' VSTs upstream, so one render covers them
    let mut s = Scene::standard("/p", None, false);
    s.chains = chains(vec![
        ("gtr", chain(Some("gtr"), &["amp"], &["bus"])),
        ("keys", chain(Some("keys"), &["verb"], &["bus"])),
        ("bus", chain(None, &["bus"], &["OUT"])),
    ]);
    s.modules.insert("bus".into(), vst("Bus"));
    assert_eq!(s.points(&mut budget()), Some(vec!["bus".to_owned()]));
}

#[test]
fn the_last_of_three_is_the_only_point() {
    // decision 340
    let mut s = Scene::standard("/p", None, false);
    s.chains = chains(vec![("gtr", chain(Some("gtr"), &["a", "b", "c"], &["OUT"]))]);
    s.modules.clear();
    for m in ["a", "b", "c"] {
        s.modules.insert(m.into(), vst(m));
    }
    assert_eq!(s.points(&mut budget()), Some(vec!["c".to_owned()]));
}

#[test]
fn a_vst_under_a_manual_freeze_is_left_live_and_the_next_one_still_freezes() {
    // decision 341
    let mut s = Scene::standard("/p", None, true);
    s.manual = vec![FreezePlan {
        at: "amp".into(),
        render: "renders/amp.wav".into(),
        lanes: ["gtr".to_owned()].into_iter().collect(),
        modules: ["amp".to_owned()].into_iter().collect(),
    }];
    assert_eq!(s.points(&mut budget()), Some(vec!["cab".to_owned(), "verb".to_owned()]));
}

#[test]
fn a_thawed_module_is_left_live_and_so_is_any_vst_with_it_upstream() {
    // autoFreeze.test.ts "leaves a thawed module live"; decision 341
    let mut s = Scene::standard("/p", None, true);
    s.thawed.insert("amp".into());
    assert_eq!(s.points(&mut budget()), Some(vec!["verb".to_owned()]));
}

#[test]
fn a_vst_fed_by_a_strudel_lane_is_left_live() {
    // decision 341
    let mut s = Scene::standard("/p", None, false);
    s.strudel.insert("keys".into());
    assert_eq!(s.points(&mut budget()), Some(vec!["amp".to_owned()]));
}

#[test]
fn a_module_on_no_chain_is_no_point() {
    // decision 339: the module must be a step of some run
    let mut s = Scene::standard("/p", None, false);
    s.modules.insert("orphan".into(), vst("Orphan"));
    assert_eq!(s.points(&mut budget()), Some(vec!["amp".to_owned(), "verb".to_owned()]));
}

#[test]
fn a_module_with_no_kind_is_a_plugin_to_the_vst_test() {
    // decision 339, as toModule
    let mut m = vst("Amp");
    m.kind = None;
    assert!(is_vst_module(Some(&m)));
    for kind in [ModuleKind::Io, ModuleKind::Doc, ModuleKind::Track] {
        let mut m = vst("Amp");
        m.kind = Some(kind);
        assert!(!is_vst_module(Some(&m)));
    }
    assert!(!is_vst_module(None));
}

#[test]
fn only_an_external_format_with_a_name_in_a_hosting_category_is_a_vst() {
    // decision 339
    assert!(is_vst_module(Some(&plugin_module("VST3", "Amp"))));
    assert!(is_vst_module(Some(&plugin_module("clap", "Amp"))));
    assert!(!is_vst_module(Some(&plugin_module("internal", "Amp"))));
    assert!(!is_vst_module(Some(&plugin_module("vst3", ""))));
    let mut shader = vst("Amp");
    shader.category = Some("shaders".into());
    assert!(!is_vst_module(Some(&shader)));
}

#[test]
fn a_freeze_is_listed_with_its_plan_its_cut_and_its_plugins() {
    // autoFreeze.test.ts "plays live until the render exists"; decisions 342, 346
    let t = Tmp::new("auto-list");
    let f = freezes(&scene(&t, false), &t);
    assert_eq!(f.iter().map(|x| x.module.as_str()).collect::<Vec<_>>(), ["amp", "verb"]);
    let amp = of(&f, "amp");
    assert_eq!(amp.plan.at, "amp");
    assert_eq!(amp.plan.render, amp.render);
    assert_eq!(amp.plan.lanes.iter().collect::<Vec<_>>(), ["gtr"]);
    assert_eq!(amp.plan.modules.iter().collect::<Vec<_>>(), ["amp"]);
    assert_eq!(amp.cut.tracks.len(), 1);
    assert_eq!(amp.cut.tracks[0].plugins.len(), 1);
    assert_eq!(
        amp.plugins,
        [song::freeze::FreezePlugin {
            format: "vst3".into(),
            name: "Amp".into()
        }]
    );
    assert!(f.iter().all(|x| !x.ready));
}

#[test]
fn a_render_is_kept_in_the_renders_folder_never_inside_the_project() {
    // autoFreeze.test.ts "in the machine's cache, never inside the project"; decision 346
    let t = Tmp::new("auto-where");
    let f = freezes(&scene(&t, false), &t);
    let r = &of(&f, "amp").render;
    assert!(r.starts_with(&t.at("renders/amp-")) && r.ends_with(".wav"), "{r}");
    assert!(!r.starts_with(&t.at("proj")));
}

#[test]
fn a_render_plays_once_it_exists() {
    // autoFreeze.test.ts "plays live until the render exists, then the render"; decisions 346, 347
    let t = Tmp::new("auto-ready");
    let s = scene(&t, false);
    let before = freezes(&s, &t);
    assert!(before.iter().all(|f| !f.plays_render()));
    make(of(&before, "amp"));
    let after = freezes(&s, &t);
    assert!(of(&after, "amp").ready && of(&after, "amp").plays_render());
    assert!(!of(&after, "verb").ready);
}

#[test]
fn a_render_that_is_a_folder_counts_as_ready() {
    // decision 335, 346: `existsSync` says yes to a folder
    let t = Tmp::new("auto-folder");
    let s = scene(&t, false);
    let f = freezes(&s, &t);
    std::fs::create_dir_all(&of(&f, "amp").render).expect("folder");
    assert!(of(&freezes(&s, &t), "amp").ready);
}

#[cfg(unix)]
#[test]
fn a_render_that_is_a_link_out_of_the_mounts_is_not_ready() {
    // decision 335: Outside is not there
    let t = Tmp::new("auto-link");
    let out = Tmp::new("auto-link-out");
    out.write("x.wav", "");
    let s = scene(&t, false);
    let f = freezes(&s, &t);
    let r = &of(&f, "amp").render;
    std::fs::create_dir_all(std::path::Path::new(r).parent().expect("parent")).expect("folder");
    std::os::unix::fs::symlink(out.0.join("x.wav"), r).expect("link");
    assert!(!of(&freezes(&s, &t), "amp").ready);
}

#[test]
fn an_engine_hosting_every_vst_of_the_run_ignores_the_render() {
    // autoFreeze.test.ts "an engine hosting every VST of the run ignores the render"; decision 347
    let t = Tmp::new("auto-hosted");
    let mut s = scene(&t, false);
    let first = freezes(&s, &t);
    make(of(&first, "amp"));
    s.scan = scan_of(&[("VST3", "Amp")]);
    let f = freezes(&s, &t);
    assert!(of(&f, "amp").hosted && of(&f, "amp").ready && !of(&f, "amp").plays_render());
}

#[test]
fn an_engine_missing_one_plays_the_render_instead() {
    // autoFreeze.test.ts "an engine missing one plays the render instead"; decision 347
    let t = Tmp::new("auto-lacking");
    let mut s = scene(&t, false);
    let first = freezes(&s, &t);
    make(of(&first, "amp"));
    s.scan = scan_of(&[("vst3", "Verb")]);
    let f = freezes(&s, &t);
    assert!(!of(&f, "amp").hosted && of(&f, "amp").plays_render());
}

#[test]
fn an_engine_with_no_vsts_hosts_none_whatever_the_scan_says() {
    // decision 347: `vsts: none`
    let t = Tmp::new("auto-novsts");
    let mut s = scene(&t, false);
    s.scan = scan_of(&[("VST3", "Amp"), ("VST3", "Verb")]);
    assert!(freezes(&s, &t).iter().all(|f| f.hosted));
    s.no_vsts = true;
    assert!(freezes(&s, &t).iter().all(|f| !f.hosted));
}

#[test]
fn a_scan_that_cannot_say_hosts_nothing() {
    // decision 347: an empty scan is "cannot say", so the render plays
    let t = Tmp::new("auto-emptyscan");
    assert!(freezes(&scene(&t, false), &t).iter().all(|f| !f.hosted));
}

#[test]
fn a_vst_param_change_is_another_render() {
    // autoFreeze.test.ts "plays live again once a VST param changes"; decision 343
    let t = Tmp::new("auto-param");
    let mut s = scene(&t, false);
    let before = of(&freezes(&s, &t), "amp").render.clone();
    s.live.tracks[0].plugins[0].params.insert("Gain".into(), 2.0);
    assert_ne!(of(&freezes(&s, &t), "amp").render, before);
}

#[test]
fn a_change_to_the_tracks_content_is_another_render() {
    // autoFreeze.test.ts "plays live again once the track's content changes"; decision 343
    let t = Tmp::new("auto-content");
    let mut s = scene(&t, false);
    let before = of(&freezes(&s, &t), "amp").render.clone();
    s.live.tracks[0].name = "other".into();
    assert_ne!(of(&freezes(&s, &t), "amp").render, before);
}

#[test]
fn a_vst_preset_rewritten_in_place_is_another_render() {
    // autoFreeze.test.ts "plays live again once the VST's preset file is rewritten"; decision 343, 345
    let t = Tmp::new("auto-preset");
    let path = t.at("proj/nodes/amp.vstpreset");
    t.write("proj/nodes/amp.vstpreset", "state-1");
    let mut s = scene(&t, false);
    s.live.tracks[0].plugins[0].surge_patch = Some(path);
    let before = of(&freezes(&s, &t), "amp").render.clone();
    t.write("proj/nodes/amp.vstpreset", "state-2 (longer)");
    assert_ne!(of(&freezes(&s, &t), "amp").render, before);
}

#[test]
fn the_render_is_named_the_same_wherever_the_project_lives() {
    // autoFreeze.test.ts "names the render the same wherever the project lives"; decision 343
    let (a, b) = (Tmp::new("auto-here"), Tmp::new("auto-there"));
    let names = |t: &Tmp| {
        let mut s = Scene::standard(&t.at("proj"), Some("/renders"), false);
        s.live.tracks[0].plugins[0].surge_patch = Some(t.at("proj/nodes/amp.vstpreset"));
        t.write("proj/nodes/amp.vstpreset", "state");
        let src = MemSource::from_bundle(song::SourceBundle {
            format: 1,
            mounts: vec![spell(&t.0), "/renders".into()],
            files: std::collections::BTreeMap::from([(
                t.at("proj/nodes/amp.vstpreset"),
                song::BundleFile {
                    text: Some("state".into()),
                    len: None,
                    ..Default::default()
                },
            )]),
            lists: std::collections::BTreeMap::from([(spell(&t.0), vec![]), ("/renders".to_owned(), vec![])]),
            ..Default::default()
        })
        .expect("bundle");
        s.run(&src, &mut Cache::new(), &mut budget())
            .freezes
            .iter()
            .map(|f| f.render.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&a), names(&b));
}

#[test]
fn a_module_that_is_not_in_the_live_document_is_no_freeze() {
    // decision 341: only a module the compiler turned into a plugin has a signal to render
    let t = Tmp::new("auto-notcompiled");
    let mut s = scene(&t, false);
    s.live = doc(vec![track("gtr", &["cab"]), track("keys", &["verb"])]);
    assert_eq!(
        freezes(&s, &t).iter().map(|f| f.module.as_str()).collect::<Vec<_>>(),
        ["verb"]
    );
}

#[test]
fn no_renders_folder_is_no_automatic_freeze_at_all() {
    // decision 346
    let t = Tmp::new("auto-norenders");
    let mut s = scene(&t, false);
    s.renders = None;
    let out = s.run(&world(&t), &mut Cache::new(), &mut budget());
    assert!(out.freezes.is_empty() && !out.capped);
}

#[test]
fn a_module_name_with_a_colon_a_slash_and_a_backslash_stays_one_file_name() {
    // freeze_auto hostile row "a VST named a:b/c\d"; decision 346: file_safe on every OS
    let t = Tmp::new("auto-name");
    let mut s = scene(&t, false);
    s.modules.clear();
    s.modules.insert("a:b/c\\d".into(), vst("Odd"));
    s.chains = chains(vec![("gtr", chain(Some("gtr"), &["a:b/c\\d"], &["OUT"]))]);
    s.live = doc(vec![track("gtr", &["a:b/c\\d"])]);
    let f = freezes(&s, &t);
    assert_eq!(f.len(), 1);
    let name = f[0]
        .render
        .strip_prefix(&t.at("renders/"))
        .expect("under the renders folder");
    assert!(
        name.starts_with("a-b-c-d-") && !name.contains('/') && !name.contains('\\'),
        "{name}"
    );
}

#[test]
fn the_render_path_is_the_safe_module_name_dash_the_fingerprint() {
    // decision 346
    assert_eq!(
        auto_render_path("/r", "amp", "0123456789abcdef"),
        "/r/amp-0123456789abcdef.wav"
    );
    assert_eq!(auto_render_path("/r", "a:b/c\\d", "ff"), "/r/a-b-c-d-ff.wav");
}

#[test]
fn a_freeze_serializes_its_sets_as_arrays() {
    // decision 359
    let t = Tmp::new("auto-json");
    let f = freezes(&scene(&t, false), &t);
    let v = serde_json::to_value(of(&f, "amp")).expect("json");
    assert_eq!(v["plan"]["lanes"], serde_json::json!(["gtr"]));
    assert_eq!(v["plan"]["modules"], serde_json::json!(["amp"]));
    let back: AutoFreeze = serde_json::from_value(v).expect("a freeze");
    assert_eq!(&back, of(&f, "amp"));
}

#[test]
fn the_render_stat_counts_a_file_and_refuses_the_rest() {
    // decision 335: Ok of any kind; NotFound, Outside and a NUL are not there; Missing is the error
    let t = Tmp::new("auto-stat");
    t.write("renders/a.wav", "");
    t.mkdir("renders/dir");
    let src = world(&t);
    assert_eq!(
        render_ready(&src, &t.at("renders/a.wav"), song::PathRule::HOST),
        Ok(true)
    );
    assert_eq!(render_ready(&src, &t.at("renders/dir"), song::PathRule::HOST), Ok(true));
    assert_eq!(
        render_ready(&src, &t.at("renders/none.wav"), song::PathRule::HOST),
        Ok(false)
    );
    assert_eq!(
        render_ready(&src, "/definitely/outside.wav", song::PathRule::HOST),
        Ok(false)
    );
    assert_eq!(
        render_ready(&src, &format!("{}/a\0.wav", t.at("renders")), song::PathRule::HOST),
        Ok(false)
    );
    assert_eq!(render_ready(&src, "relative.wav", song::PathRule::HOST), Ok(false));
    let bundle = song::SourceBundle {
        format: 1,
        mounts: vec!["/p".into()],
        files: Default::default(),
        lists: std::collections::BTreeMap::from([(
            "/p".to_owned(),
            vec![song::Entry {
                name: "deep".into(),
                kind: song::Kind::Dir,
            }],
        )]),
        ..Default::default()
    };
    let mem = MemSource::from_bundle(bundle).expect("bundle");
    assert_eq!(
        render_ready(&mem, "/p/deep/x.wav", song::PathRule::HOST),
        Err(SourceError::Missing)
    );
}

#[test]
fn a_budget_spent_before_the_points_are_found_is_no_automatic_freeze() {
    // decision 348: capped, none, and the document plays live
    let t = Tmp::new("auto-spent");
    let s = scene(&t, false);
    let out = s.run(&world(&t), &mut Cache::new(), &mut Budget::new(3));
    assert!(out.freezes.is_empty() && out.capped);
    assert_eq!(s.points(&mut Budget::new(3)), None);
    let mut spent = Budget::new(0);
    assert!(!spent.charge(1));
    assert!(s.run(&world(&t), &mut Cache::new(), &mut spent).capped);
}

#[test]
fn the_plugins_of_a_freeze_are_the_vsts_inside_its_plan_with_the_format_lowercased() {
    // decision 342: `{format, name}` of each VST module of the plan
    let t = Tmp::new("auto-plugins");
    let mut s = scene(&t, true);
    s.modules.insert("amp".into(), plugin_module("VST3", "Amp"));
    let f = freezes(&s, &t);
    let cab = of(&f, "cab");
    let got: Vec<(&str, &str)> = cab
        .plugins
        .iter()
        .map(|p| (p.format.as_str(), p.name.as_str()))
        .collect();
    assert_eq!(got, [("vst3", "Cab"), ("vst3", "Amp")]);
}
