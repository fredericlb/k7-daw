//! server_plugins.rs — the plugin verbs over a real socket and the fake engine: every reply byte for byte against
//! the fixture, the one scan and its progress, the probe cap of four, pending queries and superseded
//! parameter writes under concurrency, and the message-thread calls each verb makes.

use crate::harness::{Server, Wire, eventually};
use k7_runtime_server::engine::fake::with_state;
use k7_runtime_server::engine::fake_plugins::{self, with};
use k7_runtime_server::engine::fake_state::set_param_infos;
use k7_runtime_server::engine::plugins::{PluginInfo, PluginInstance, Preset};
use k7_runtime_server::engine::state::ParamInfo;
use k7_runtime_server::server::ServeConfig;
use k7_runtime_server::server::plugin_scan::status_json;
use k7_runtime_server::server::scan_board::Status;
use serde_json::{Value, json};
use std::future::Future;

/// A test of the process-wide plugins fake: alone, on a fresh fake, with its own runtime.
fn scripted<F: Future>(body: impl FnOnce() -> F) {
    let _alone = fake_plugins::serial();
    fake_plugins::reset();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(body());
    fake_plugins::release();
}

fn started() -> Server {
    Server::start(ServeConfig::new(0))
}

fn val(frame: &str) -> Value {
    serde_json::from_str(frame).unwrap()
}

/// The next frame of this type, the others skipped.
async fn of_type(w: &mut Wire, ty: &str) -> String {
    loop {
        let frame = w.text().await;
        if val(&frame)["type"] == ty {
            return frame;
        }
    }
}

/// The types of the next `n` frames.
async fn types(w: &mut Wire, n: usize) -> Vec<String> {
    let mut out = Vec::new();
    for _ in 0..n {
        out.push(val(&w.text().await)["type"].as_str().unwrap().to_string());
    }
    out
}

fn plugin_of(v: &Value) -> PluginInfo {
    let s = |k: &str| v[k].as_str().unwrap().to_string();
    PluginInfo {
        name: s("name"),
        format: s("format"),
        manufacturer: s("manufacturer"),
        instrument: v["instrument"].as_bool().unwrap(),
        role: s("role"),
        category: s("category"),
        id: s("id"),
    }
}

fn param_of(v: &Value) -> ParamInfo {
    let f = |k: &str| v[k].as_f64().unwrap();
    ParamInfo {
        name: v["name"].as_str().unwrap().to_string(),
        value: v["value"].as_f64().unwrap_or(0.0),
        min: f("min"),
        max: f("max"),
        default: v.get("def").map(|d| d.as_f64().unwrap()),
        states: v["states"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|s| (s["label"].as_str().unwrap().to_string(), f_of(&s["value"])))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn f_of(v: &Value) -> f64 {
    v.as_f64().unwrap()
}

/// Each fixture line of one kind: its spec and the reply's bytes.
fn cases(kind: &str) -> Vec<(Value, String)> {
    crate::repo_file("apps/k7-daw-backend/packages/k7-runtime-server/fixtures/plugin_replies.txt")
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .map(|(spec, want)| (val(spec), want.to_string()))
        .filter(|(spec, _)| spec["kind"] == kind)
        .collect()
}

#[test]
fn the_catalogue_is_the_c_plus_plus_catalogue_byte_for_byte() {
    scripted(|| async {
        let s = started();
        let mut w = s.connect().await;
        for (spec, want) in cases("plugins") {
            let list: Vec<PluginInfo> = spec["plugins"]
                .as_array()
                .unwrap()
                .iter()
                .map(plugin_of)
                .collect();
            with(|f| {
                f.cache = list.clone();
                f.warm = true;
                f.hold_scans = spec["scanning"] == true;
            });
            // A scan running is what `scanning` says: a held rescan, whose begin frame is drained first.
            if spec["scanning"] == true {
                w.send(r#"{"type":"rescan-plugins"}"#).await;
                assert_eq!(val(&of_type(&mut w, "plugin-scan").await)["scanning"], true);
            }
            let ask = json!({"type": "list-plugins", "filter": spec["filter"]});
            w.send(&ask.to_string()).await;
            assert_eq!(of_type(&mut w, "plugins").await, want, "{spec}");
            fake_plugins::release();
            if spec["scanning"] == true {
                of_type(&mut w, "plugins").await;
                of_type(&mut w, "plugin-scan").await;
            }
        }
    });
}

#[test]
fn the_scan_status_is_the_c_plus_plus_status_byte_for_byte() {
    for (spec, want) in cases("scan") {
        let s = Status {
            scanning: spec["scanning"].as_bool().unwrap(),
            done: spec["done"].as_i64().unwrap() as i32,
            total: spec["total"].as_i64().unwrap() as i32,
            current: spec["current"].as_str().unwrap().to_string(),
            version: spec["version"].as_i64().unwrap() as i32,
        };
        assert_eq!(k7_runtime_server::wire::json::dump(&status_json(&s)), want);
    }
}

#[test]
fn a_plugin_types_parameters_are_the_c_plus_plus_reply_byte_for_byte() {
    scripted(|| async {
        let s = started();
        let mut w = s.connect().await;
        for (spec, want) in cases("param-list") {
            let (format, id) = (
                spec["format"].as_str().unwrap(),
                spec["id"].as_str().unwrap(),
            );
            if spec["found"] == true {
                let infos = spec["params"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(param_of)
                    .collect();
                with(|f| f.specs.insert((format.into(), id.into()), infos));
            } else {
                with(|f| f.specs.clear());
            }
            let ask = json!({"type": "list-plugin-params", "format": format, "id": id});
            assert_eq!(w.ask(&ask.to_string()).await, want, "{spec}");
        }
    });
}

#[test]
fn the_engine_wide_plugin_list_is_the_c_plus_plus_reply_byte_for_byte() {
    scripted(|| async {
        let s = started();
        let mut w = s.connect().await;
        for (spec, want) in cases("edit-plugins") {
            let mut sessions: Vec<String> = Vec::new();
            with(|f| f.instances.clear());
            for p in spec["plugins"].as_array().unwrap() {
                let session = p["session"].as_str().unwrap().to_string();
                if !sessions.contains(&session) {
                    sessions.push(session.clone());
                }
                with(|f| {
                    f.instances
                        .entry(session)
                        .or_default()
                        .push(PluginInstance {
                            id: p["id"].as_str().unwrap().into(),
                            name: p["name"].as_str().unwrap().into(),
                            has_native_ui: p["hasNativePluginUI"].as_bool().unwrap(),
                        })
                });
            }
            with_state(&s.engine, |st| st.sessions = sessions);
            assert_eq!(
                w.ask(r#"{"type":"list-edit-plugins"}"#).await,
                want,
                "{spec}"
            );
        }
        assert_eq!(with(|f| f.off_thread), 0);
    });
}

#[test]
fn a_modules_presets_are_the_c_plus_plus_reply_byte_for_byte() {
    scripted(|| async {
        let s = started();
        let mut w = s.connect().await;
        for (spec, want) in cases("presets") {
            let (session, module) = (
                spec["session"].as_str().unwrap(),
                spec["module"].as_str().unwrap(),
            );
            let rows = spec["presets"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| Preset {
                    name: p["name"].as_str().unwrap().into(),
                    source: p["source"].as_str().unwrap().into(),
                    index: p["index"].as_i64().unwrap() as i32,
                    path: p["path"].as_str().unwrap().into(),
                })
                .collect();
            with(|f| f.presets.insert((session.into(), module.into()), rows));
            let ask = json!({"type": "list-presets", "session": session, "module": module});
            assert_eq!(w.ask(&ask.to_string()).await, want, "{spec}");
        }
    });
}

#[test]
fn a_live_plugins_parameters_are_the_c_plus_plus_reply_byte_for_byte() {
    scripted(|| async {
        let s = started();
        let mut w = s.connect().await;
        for (spec, want) in cases("get-params") {
            let (session, id) = (
                spec["session"].as_str().unwrap(),
                spec["id"].as_str().unwrap(),
            );
            let infos = spec["infos"]
                .as_array()
                .unwrap()
                .iter()
                .map(param_of)
                .collect();
            set_param_infos(&s.engine, session, id, infos);
            let ask = json!({"type": "get-plugin-params", "session": session, "id": id});
            assert_eq!(w.ask(&ask.to_string()).await, want, "{spec}");
        }
    });
}

#[test]
fn a_native_window_and_a_base_are_answered_either_way() {
    scripted(|| async {
        let s = started();
        let mut w = s.connect().await;
        let (opened, saved) = (cases("opened"), cases("base-saved"));
        let ask = r#"{"type":"show-native-plugin-ui-of","format":"VST3","id":"/p.vst3"}"#;
        with(|f| {
            f.loadable.push(("VST3".into(), "/p.vst3".into()));
            f.opens_ok = true;
        });
        assert_eq!(w.ask(ask).await, opened[0].1);
        // The load failed: the same answer with `ok` false, since a caller told nothing cannot tell slow from dead.
        with(|f| f.loadable.clear());
        assert_eq!(w.ask(ask).await, opened[1].1);
        with(|f| f.snapshot_ok = true);
        let snap = r#"{"type":"snapshot-plugin-base","format":"VST3","id":"/p.vst3","path":"/h/base.vstpreset"}"#;
        assert_eq!(w.ask(snap).await, saved[0].1);
        assert_eq!(with(|f| f.off_thread), 0);
    });
}

#[test]
fn a_verb_missing_what_it_needs_says_so_to_the_asker_alone() {
    scripted(|| async {
        let s = started();
        let (mut w, mut other) = (s.connect().await, s.connect().await);
        let want = cases("error");
        let asks = [
            r#"{"type":"list-plugin-params","format":"VST3"}"#,
            r#"{"type":"show-native-plugin-ui-of","id":"/p.vst3"}"#,
            r#"{"type":"snapshot-plugin-base","format":"VST3","id":"/p.vst3"}"#,
        ];
        for (ask, (_, want)) in asks.iter().zip(&want[1..]) {
            assert_eq!(&w.ask(ask).await, want);
        }
        // Nothing reached the other socket: a sentinel is its next frame.
        other.send(r#"{"type":"plugin-scan"}"#).await;
        assert_eq!(val(&other.text().await)["type"], "plugin-scan");
    });
}

#[test]
fn a_field_of_the_wrong_type_is_answered_malformed() {
    scripted(|| async {
        let s = started();
        let mut w = s.connect().await;
        let got = w.ask(r#"{"type":"list-plugins","filter":5}"#).await;
        assert_eq!(
            got,
            r#"{"message":"malformed message: [json.exception.type_error.302] type must be string, but is number","type":"error"}"#
        );
        let got = w
            .ask(r#"{"type":"list-plugin-params","format":"a","id":[]}"#)
            .await;
        assert!(got.contains("type must be string, but is array"));
    });
}

#[test]
fn a_warm_cache_is_listed_with_no_scan_and_a_cold_one_scans_once() {
    scripted(|| async {
        let s = started();
        let mut w = s.connect().await;
        let one = PluginInfo {
            name: "A".into(),
            format: "internal".into(),
            id: "builtin:a".into(),
            ..PluginInfo::default()
        };
        with(|f| {
            f.cache = vec![one.clone()];
            f.warm = true;
        });
        w.send(r#"{"type":"list-plugins"}"#).await;
        assert!(of_type(&mut w, "plugins").await.contains("builtin:a"));
        assert!(with(|f| f.scans.is_empty()));
        // Cold: the ask scans, once, and every socket hears the begin, the catalogue, then the end.
        let found = PluginInfo {
            name: "B".into(),
            format: "VST3".into(),
            id: "/b.vst3".into(),
            ..PluginInfo::default()
        };
        with(|f| {
            f.warm = false;
            f.scan_result = Some(Ok(vec![found]));
        });
        w.send(r#"{"type":"list-plugins"}"#).await;
        assert_eq!(
            types(&mut w, 3).await,
            ["plugin-scan", "plugins", "plugin-scan"]
        );
        assert_eq!(with(|f| f.scans.clone()), [false]);
        w.send(r#"{"type":"plugin-scan"}"#).await;
        let status = val(&w.text().await);
        assert_eq!(
            (status["scanning"].clone(), status["version"].clone()),
            (json!(false), json!(1))
        );
    });
}

#[test]
fn a_scan_says_where_it_stands_without_a_frame_per_file() {
    scripted(|| async {
        let s = started();
        let mut w = s.connect().await;
        with(|f| {
            f.scan_steps = vec![
                (1, 100, String::new()),
                (2, 100, String::new()),
                (3, 100, "A.vst3".into()),
                (4, 100, "A.vst3".into()),
                (5, 100, "B.vst3".into()),
            ];
            f.scan_result = Some(Ok(Vec::new()));
        });
        w.send(r#"{"type":"rescan-plugins","force":true}"#).await;
        let begin = val(&w.text().await);
        assert_eq!(
            (begin["scanning"].clone(), begin["total"].clone()),
            (json!(true), json!(0))
        );
        // Only the two loads starting are said; the stat-only steps and the repeat are not.
        let a = val(&w.text().await);
        assert_eq!(
            (a["current"].clone(), a["done"].clone(), a["total"].clone()),
            (json!("A.vst3"), json!(3), json!(100))
        );
        let b = val(&w.text().await);
        assert_eq!(b["current"], "B.vst3");
        assert_eq!(types(&mut w, 2).await, ["plugins", "plugin-scan"]);
        assert_eq!(with(|f| f.scans.clone()), [true]);
    });
}

#[test]
fn two_scans_asked_together_run_one_after_the_other() {
    scripted(|| async {
        let s = started();
        let mut w = s.connect().await;
        with(|f| {
            f.hold_scans = true;
            f.scan_result = Some(Ok(Vec::new()));
        });
        for _ in 0..3 {
            w.send(r#"{"type":"rescan-plugins"}"#).await;
        }
        eventually("a scan began", || with(|f| f.scans_running == 1)).await;
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert_eq!(with(|f| (f.scans.len(), f.scans_running)), (1, 1));
        fake_plugins::release();
        eventually("every scan ran", || with(|f| f.scans.len() == 3)).await;
        assert_eq!(with(|f| f.scans_peak), 1);
        // Three scans, three ends: the version counted them.
        w.send(r#"{"type":"plugin-scan"}"#).await;
        eventually("the count", || with(|f| f.scans_running == 0)).await;
        let mut seen = 0;
        while seen < 3 {
            let status = val(&of_type(&mut w, "plugin-scan").await);
            if status["scanning"] == false {
                seen += 1;
            }
        }
    });
}

#[test]
fn a_scan_that_throws_still_ends_and_is_warned() {
    scripted(|| async {
        let s = started();
        let mut w = s.connect().await;
        let cached = PluginInfo {
            name: "Kept".into(),
            id: "builtin:kept".into(),
            ..PluginInfo::default()
        };
        with(|f| {
            f.cache = vec![cached];
            f.scan_result = Some(Err("a plugin took the scan down".into()));
        });
        w.send(r#"{"type":"rescan-plugins"}"#).await;
        assert_eq!(
            types(&mut w, 3).await,
            ["plugin-scan", "plugins", "plugin-scan"]
        );
        let warning = val(&w.text().await);
        assert_eq!(warning["type"], "engine-warning");
        assert_eq!(
            warning["message"],
            "background work failed: a plugin took the scan down"
        );
        assert!(warning.get("session").is_none());
        w.send(r#"{"type":"plugin-scan"}"#).await;
        let status = val(&of_type(&mut w, "plugin-scan").await);
        assert_eq!(
            (status["scanning"].clone(), status["version"].clone()),
            (json!(false), json!(1))
        );
    });
}

#[test]
fn the_incremental_scan_at_every_start_tells_each_socket() {
    scripted(|| async {
        with(|f| {
            f.hold_scans = true;
            f.scan_result = Some(Ok(vec![PluginInfo {
                name: "N".into(),
                id: "builtin:n".into(),
                ..PluginInfo::default()
            }]));
        });
        let mut cfg = ServeConfig::new(0);
        cfg.startup_scan = true;
        let s = Server::start(cfg);
        let mut w = s.connect().await;
        eventually("the scan began", || with(|f| f.scans_running == 1)).await;
        // A socket is trusted from its first message, so it asks where the scan stands before it hears the rest.
        assert_eq!(
            val(&w.ask(r#"{"type":"plugin-scan"}"#).await)["scanning"],
            true
        );
        fake_plugins::release();
        // The begin was said before this socket came: it hears the catalogue and the end.
        assert_eq!(types(&mut w, 2).await, ["plugins", "plugin-scan"]);
        assert_eq!(with(|f| f.scans.clone()), [false]);
    });
}

#[test]
fn only_four_probes_load_at_once_and_a_slot_comes_back() {
    scripted(|| async {
        let s = started();
        let mut w = s.connect().await;
        with(|f| {
            f.hold_probes = true;
            for n in 0..6 {
                f.specs
                    .insert(("VST3".into(), format!("/{n}.vst3")), Vec::new());
            }
        });
        for n in 0..4 {
            w.send(&json!({"type": "list-plugin-params", "format": "VST3", "id": format!("/{n}.vst3")}).to_string()).await;
        }
        eventually("four probes load", || with(|f| f.probes_running == 4)).await;
        // The fifth and sixth are refused, to the asker, and never queued.
        let want = cases("error");
        for n in 4..6 {
            let ask =
                json!({"type": "list-plugin-params", "format": "VST3", "id": format!("/{n}.vst3")});
            assert_eq!(w.ask(&ask.to_string()).await, want[0].1);
        }
        assert_eq!(with(|f| f.probes.len()), 4);
        fake_plugins::release();
        let mut replies = 0;
        while replies < 4 {
            of_type(&mut w, "plugin-param-list").await;
            replies += 1;
        }
        assert_eq!(with(|f| f.probes_peak), 4);
        eventually("every slot is back", || s.ctx.plugins.probes_in_use() == 0).await;
        let again = json!({"type": "list-plugin-params", "format": "VST3", "id": "/5.vst3"});
        assert!(
            w.ask(&again.to_string())
                .await
                .contains("plugin-param-list")
        );
    });
}

#[test]
fn a_window_load_takes_a_slot_too_and_gives_it_back_when_the_load_fails() {
    scripted(|| async {
        let s = started();
        let mut w = s.connect().await;
        with(|f| f.hold_probes = true);
        for n in 0..4 {
            w.send(&json!({"type": "show-native-plugin-ui-of", "format": "VST3", "id": format!("/{n}.vst3")}).to_string()).await;
        }
        eventually("four loads", || with(|f| f.probes_running == 4)).await;
        let fifth = w
            .ask(r#"{"type":"show-native-plugin-ui-of","format":"VST3","id":"/4.vst3"}"#)
            .await;
        assert!(fifth.contains("too many plugins loading at once"));
        fake_plugins::release();
        for _ in 0..4 {
            let reply = val(&of_type(&mut w, "native-plugin-ui-opened").await);
            assert_eq!(reply["ok"], false);
        }
        eventually("slots back", || s.ctx.plugins.probes_in_use() == 0).await;
    });
}

#[test]
fn identical_queries_waiting_on_the_message_thread_are_answered_once() {
    scripted(|| async {
        let mut s = Server::held(ServeConfig::new(0));
        let mut w = s.connect().await;
        set_param_infos(
            &s.engine,
            "k7:s",
            "c:m",
            vec![ParamInfo {
                name: "Mix".into(),
                value: 0.5,
                min: 0.0,
                max: 1.0,
                default: None,
                states: vec![],
            }],
        );
        let ask = r#"{"type":"get-plugin-params","session":"k7:s","id":"c:m"}"#;
        for _ in 0..5 {
            w.send(ask).await;
            w.send(r#"{"type":"list-edit-plugins"}"#).await;
            w.send(r#"{"type":"list-presets","session":"k7:s","module":"m"}"#)
                .await;
        }
        eventually("all fifteen were read", || s.ctx.pending.size() == 3).await;
        s.release();
        // One of each, in the order the first of each was asked.
        assert_eq!(
            types(&mut w, 3).await,
            ["plugin-params", "edit-plugins", "presets"]
        );
        // Asked again once the job ran, the query is queued again; a sentinel shows nothing else was queued.
        w.send(ask).await;
        w.send(r#"{"type":"list-edit-plugins"}"#).await;
        assert_eq!(types(&mut w, 2).await, ["plugin-params", "edit-plugins"]);
        assert_eq!(s.ctx.pending.size(), 0);
    });
}

#[test]
fn a_parameter_write_skips_the_older_ones_still_queued() {
    scripted(|| async {
        let mut s = Server::held(ServeConfig::new(0));
        let mut w = s.connect().await;
        let set = |plugin: &str, name: &str, v: f64| {
            json!({"type": "set-plugin-param", "session": "k7:s", "id": plugin, "name": name, "value": v}).to_string()
        };
        for v in [0.1, 0.2, 0.3] {
            w.send(&set("c:m", "Cutoff", v)).await;
            w.send(&set("c:m", "Mix", v)).await;
        }
        w.send(&set("c:n", "Cutoff", 0.9)).await;
        // A sentinel behind them all, so the log is read once every write has had its turn.
        w.send(r#"{"type":"list-edit-plugins"}"#).await;
        w.send(r#"{"type":"plugin-scan"}"#).await;
        assert_eq!(types(&mut w, 1).await, ["plugin-scan"]);
        s.release();
        of_type(&mut w, "edit-plugins").await;
        let calls = with(|f| f.calls.clone());
        assert_eq!(
            calls
                .iter()
                .filter(|c| c.starts_with("set_plugin_param"))
                .collect::<Vec<_>>(),
            [
                "set_plugin_param k7:s c:m Cutoff 0.3",
                "set_plugin_param k7:s c:m Mix 0.3",
                "set_plugin_param k7:s c:n Cutoff 0.9"
            ]
        );
        assert_eq!(with(|f| f.off_thread), 0);
    });
}

#[test]
fn the_other_message_thread_verbs_make_their_calls_in_order() {
    scripted(|| async {
        let s = started();
        let mut w = s.connect().await;
        for msg in [
            r#"{"type":"show-native-plugin-ui","session":"k7:s","id":"c:m"}"#,
            r#"{"type":"set-preset","session":"k7:s","module":"lead","index":3}"#,
            r#"{"type":"set-preset","session":"k7:s","module":"lead","index":3,"path":"/p/Warm.vstpreset"}"#,
            r#"{"type":"set-preset","session":"k7:s","module":"lead"}"#,
            r#"{"type":"subscribe-waveform","session":"k7:s","id":"c:m"}"#,
            r#"{"type":"subscribe-waveform","session":"k7:s","id":"c:m","side":"input"}"#,
            r#"{"type":"unsubscribe-waveform","session":"k7:s","id":"c:m","side":"input"}"#,
            r#"{"type":"subscribe-waveform","session":"k7:s","id":"c:m","side":"sideways"}"#,
            r#"{"type":"subscribe-waveform","session":"k7:s","id":""}"#,
        ] {
            w.send(msg).await;
        }
        // The jobs run in order on the one thread, so the last answer comes after every call above.
        w.ask(r#"{"type":"list-edit-plugins"}"#).await;
        assert_eq!(
            with(|f| f.calls.clone()),
            [
                "show_native_ui k7:s c:m",
                "set_preset k7:s lead 3 ",
                "set_preset k7:s lead 3 /p/Warm.vstpreset",
                "set_preset k7:s lead -1 ",
                "tap_on k7:s c:m output",
                "tap_on k7:s c:m input",
                "tap_off k7:s c:m input",
            ]
        );
        assert_eq!(with(|f| f.off_thread), 0);
    });
}
