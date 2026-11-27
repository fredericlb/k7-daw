// What a right-click offers: the list is DERIVED from the catalogue, an entry PRE-WRITES a command line, and a hand-named row carries a message key.
// These are the TypeScript suite's cases for `contextMenu`, over a slice of the real commands.yaml kept to the shapes that decide something.

use serde_json::{Value, json};
use verbs::palette::CommandInfo;
use verbs::ui::menu::{Item, MenuCtx, context_menu};
use verbs::ui::prefs::Prefs;

fn arg(name: &str, kind: &str, provider: Option<&str>) -> Value {
    let mut a = json!({ "name": name, "kind": kind, "hint": "" });
    if let Some(p) = provider {
        a["provider"] = json!(p);
    }
    a
}

fn cmd(command: &str, help: &str, args: Vec<Value>) -> Value {
    json!({ "command": command, "help": help, "args": args })
}

fn catalogue() -> Vec<CommandInfo> {
    let tracks = || arg("track", "track", Some("tracks"));
    let mut pan = cmd(
        "track pan",
        "set a track's pan on its mixer strip, -1 left ... +1 right",
        vec![tracks(), arg("-1..1", "number", None)],
    );
    pan["readable"] = json!({ "en": "Pan a track", "fr": "Panoramiquer une piste" });
    let list = vec![
        cmd("track mute", "toggle a track's (or group's) mute", vec![tracks()]),
        cmd(
            "track gain",
            "set a track's gain in dB on its mixer strip (e.g. track gain melody -6)",
            vec![tracks(), arg("dB", "number", None)],
        ),
        pan,
        cmd(
            "event remove",
            "remove a placement",
            vec![tracks(), arg("timecode", "timecode", None)],
        ),
        cmd(
            "event move",
            "move a placement",
            vec![
                tracks(),
                arg("timecode", "timecode", None),
                arg("newTimecode", "timecode", None),
                json!({ "name": "toTrack", "kind": "track", "provider": "tracks", "hint": "", "optional": true }),
            ],
        ),
        cmd(
            "selection consolidate",
            "write one new clip over a range",
            vec![
                arg("tracks", "track", Some("tracks")),
                arg("from", "timecode", None),
                arg("to", "timecode", None),
            ],
        ),
        cmd(
            "node bypass",
            "bypass a module",
            vec![arg("node", "module", Some("nodes")), arg("state", "choice", None)],
        ),
        cmd(
            "graph unlink",
            "take a node out of its chain",
            vec![arg("node", "module", Some("nodes"))],
        ),
        cmd(
            "node replace",
            "replace what a node is",
            vec![
                arg("node", "module", Some("nodes")),
                arg("extends-ref", "module", Some("baseModules")),
            ],
        ),
        cmd(
            "node param",
            "set a module's plugin param",
            vec![
                arg("node", "module", Some("nodes")),
                arg("param", "text", None),
                arg("value", "number", None),
            ],
        ),
        cmd(
            "node external-ui",
            "open the plugin's own window",
            vec![arg("module", "module", Some("nodes"))],
        ),
        cmd(
            "module clone",
            "clone a module folder",
            vec![
                arg("category/name", "module", Some("modules")),
                arg("newName", "text", None),
            ],
        ),
        cmd(
            "module new",
            "create a module",
            vec![arg("category/name", "module", Some("categories"))],
        ),
        cmd(
            "module own",
            "give an engine module a folder of its own",
            vec![arg("ref", "module", Some("engineModules"))],
        ),
        cmd(
            "project marker new",
            "drop a named marker at a timecode",
            vec![arg("timecode", "timecode", None), arg("name", "text", None)],
        ),
        cmd(
            "project marker delete",
            "remove a marker by name",
            vec![arg("name", "marker", Some("markerNames"))],
        ),
        cmd(
            "clip warp remove",
            "drop one stretch point",
            vec![arg("clip", "file", Some("clipTargets")), arg("sec", "number", None)],
        ),
        cmd(
            "sequence note remove",
            "remove one note",
            vec![
                arg("sequence", "file", None),
                arg("note", "note", None),
                arg("start", "number", None),
            ],
        ),
        cmd(
            "sequence note split",
            "cut one note in two",
            vec![
                arg("sequence", "file", None),
                arg("note", "note", None),
                arg("start", "number", None),
                arg("at", "number", None),
            ],
        ),
        cmd(
            "sequence notes transpose",
            "shift every note in a region",
            vec![
                arg("sequence", "file", None),
                arg("loNote", "note", None),
                arg("hiNote", "note", None),
                arg("from", "number", None),
                arg("to", "number", None),
                arg("semitones", "number", None),
            ],
        ),
        cmd(
            "sequence notes remove",
            "remove every note in a region",
            vec![
                arg("sequence", "file", None),
                arg("loNote", "note", None),
                arg("hiNote", "note", None),
                arg("from", "number", None),
                arg("to", "number", None),
            ],
        ),
    ];
    serde_json::from_value(Value::Array(list)).unwrap()
}

fn open(ctx: Value, commands: &[CommandInfo], prefs: Value, locale: &str) -> Vec<Item> {
    let ctx: MenuCtx = serde_json::from_value(ctx).unwrap();
    let prefs: Prefs = prefs.as_object().cloned().unwrap_or_default();
    context_menu(&ctx, commands, &prefs, locale)
}

fn menu(ctx: Value) -> Vec<Item> {
    open(ctx, &catalogue(), json!({}), "en")
}

fn lines(items: &[Item]) -> Vec<&str> {
    items.iter().map(|i| i.command.as_str()).collect()
}

fn labels(items: &[Item]) -> Vec<String> {
    items
        .iter()
        .map(|i| i.label.text.clone().or_else(|| i.label.key.clone()).unwrap_or_default())
        .collect()
}

fn says<'a>(items: &'a [Item], label: &str) -> Option<&'a str> {
    items
        .iter()
        .find(|i| i.label.text.as_deref() == Some(label))
        .and_then(|i| i.description.text.as_deref())
}

#[test]
fn a_track_offers_every_command_it_can_answer_with_the_trailing_space_where_a_value_is_owed() {
    let m = menu(json!({ "kind": "track", "id": "gtr" }));
    assert_eq!(
        lines(&m),
        [
            "track mute gtr",
            "track gain gtr ",
            "track pan gtr ",
            "event remove gtr ",
            "event move gtr ",
            "selection consolidate gtr ",
            "ui file gtr"
        ]
    );
    // The word naming what was clicked is dropped, and a head naming something else stays.
    assert_eq!(
        labels(&m),
        [
            "mute",
            "gain",
            "pan",
            "event remove",
            "event move",
            "selection consolidate",
            "ui file"
        ]
    );
    assert_eq!(says(&m, "mute"), Some("toggle a track's (or group's) mute"));
    // The `(e.g. ...)` tail is dropped: the pre-written line already answers it.
    assert_eq!(says(&m, "gain"), Some("set a track's gain in dB on its mixer strip"));
    assert_eq!(says(&m, "pan"), Some("Pan a track"));
    let fr = open(json!({ "kind": "track", "id": "gtr" }), &catalogue(), json!({}), "fr");
    assert_eq!(says(&fr, "pan"), Some("Panoramiquer une piste"));
    assert_eq!(labels(&fr), labels(&m));
    assert_eq!(m[2].sentence.as_deref(), Some("Pan a track"));
    assert!(m[0].sentence.is_none());
}

#[test]
fn a_track_among_several_picked_offers_to_group_them_first() {
    let m = menu(json!({ "kind": "track", "id": "gtr", "picked": ["gtr", "bass"] }));
    assert_eq!(m[0].command, "track group gtr,bass");
    assert_eq!(m[0].label.key.as_deref(), Some("ctx.groupTracks"));
    assert_eq!(m[0].label.params.as_ref().unwrap()["n"], 2);
    assert!(lines(&m).contains(&"track mute gtr"));
}

#[test]
fn a_group_answers_only_two_verbs() {
    let m = menu(json!({ "kind": "group", "id": "drums" }));
    assert_eq!(lines(&m), ["track mute drums", "track solo drums"]);
    assert_eq!(labels(&m), ["mute", "solo"]);
}

#[test]
fn a_node_offers_what_takes_a_module_and_not_what_takes_a_category() {
    let m = menu(json!({ "kind": "module", "id": "lead" }));
    assert_eq!(
        lines(&m),
        [
            "node bypass lead ",
            "graph unlink lead",
            "node replace lead ",
            "node param lead ",
            "module clone lead ",
            "ui file lead"
        ]
    );
    assert_eq!(
        labels(&m),
        ["bypass", "graph unlink", "replace", "param", "module clone", "ui file"]
    );
}

#[test]
fn a_node_offers_the_models_download_first_the_plugin_window_and_the_engines_own() {
    let m = menu(json!({ "kind": "module", "id": "split", "model": true }));
    assert_eq!(m[0].command, "ui model split");
    assert!(m[0].complete);
    assert_eq!(m[0].label.key.as_deref(), Some("ctx.downloadModel"));
    assert!(!lines(&menu(json!({ "kind": "module", "id": "lead" }))).contains(&"node external-ui lead"));
    assert!(lines(&menu(json!({ "kind": "module", "id": "lead", "native": true }))).contains(&"node external-ui lead"));
    let engine = menu(json!({ "kind": "module", "id": "@engine/audio/verb", "ambient": true }));
    assert!(lines(&engine).contains(&"module own @engine/audio/verb"));
    assert!(!lines(&engine).join(" ").contains("ui file"));
}

#[test]
fn a_dangling_node_shrinks_to_the_verbs_that_survive_it() {
    let m = menu(json!({ "kind": "module", "id": "gone", "missing": true }));
    assert_eq!(lines(&m), ["node bypass gone ", "graph unlink gone"]);
}

#[test]
fn a_clip_offers_the_event_family_once_it_knows_which_placement() {
    let at = json!({ "kind": "clip", "id": "riff", "track": "gtr", "tc": "2:1:1" });
    let m = menu(at.clone());
    assert_eq!(
        lines(&m),
        [
            "ui file riff",
            "event remove gtr 2:1:1",
            "event move gtr 2:1:1 ",
            "selection consolidate gtr 2:1:1 "
        ]
    );
    assert_eq!(labels(&m), ["ui file", "remove", "move", "selection consolidate"]);
    assert_eq!(lines(&menu(json!({ "kind": "clip", "id": "riff" }))), ["ui file riff"]);
}

#[test]
fn a_clip_fills_a_range_verb_from_the_rectangle_and_once() {
    let m = menu(json!({
        "kind": "clip", "id": "riff", "track": "gtr", "tc": "2:1:1",
        "sel": { "tracks": "gtr", "from": "2:1:1", "to": "6:1:1" }
    }));
    let l = lines(&m);
    assert!(l.contains(&"selection consolidate gtr 2:1:1 6:1:1"));
    assert_eq!(l.iter().filter(|x| x.starts_with("selection consolidate")).count(), 1);
}

#[test]
fn a_param_quotes_a_name_with_spaces_and_is_not_fooled_by_a_shape() {
    let base = json!({ "kind": "param", "id": "amp", "param": "Filter Freq", "label": "Filter Freq" });
    let m = menu(base.clone());
    assert!(lines(&m).contains(&"node param amp \"Filter Freq\" "));
    let joined = lines(&m).join(" ");
    assert!(!joined.contains("bypass") && !joined.contains("module clone") && !joined.contains("none"));
    assert_eq!(labels(&m), ["ui midi learn", "ui automate", "node param"]);
    let mut bound = base.clone();
    bound["cc"] = json!({ "ch": 1, "cc": 74 });
    bound["param"] = json!("Cutoff");
    let b = menu(bound);
    assert!(lines(&b).contains(&"module cc amp Cutoff none"));
    assert!(b.iter().any(|i| i.label.key.as_deref() == Some("ctx.removeMidi")));
    let mut driven = base;
    driven["automated"] = json!(true);
    let d = menu(driven);
    let unbind = d
        .iter()
        .find(|i| i.label.key.as_deref() == Some("ctx.removeAutomation"))
        .unwrap();
    assert_eq!(unbind.command, "node automate amp Filter Freq none");
}

#[test]
fn a_curve_card_opens_the_editor_on_its_param_or_on_the_free_curve() {
    let m = menu(json!({ "kind": "curve-card", "curve": "wob", "module": "amp", "param": "Filter Freq" }));
    assert_eq!(
        lines(&m),
        ["ui automate amp \"Filter Freq\"", "node automate amp Filter Freq none"]
    );
    assert_eq!(
        lines(&menu(json!({ "kind": "curve-card", "curve": "wob" }))),
        ["ui curve wob"]
    );
}

#[test]
fn a_chain_and_a_sink_open_no_menu() {
    assert!(menu(json!({ "kind": "chain", "id": "x" })).is_empty());
    assert!(menu(json!({ "kind": "sink", "id": "OUT" })).is_empty());
}

#[test]
fn the_ruler_derives_the_marker_verbs_and_names_the_loop_for_what_it_does() {
    let m = menu(json!({ "kind": "marker-add", "tc": "5:1:1" }));
    assert_eq!(lines(&m), ["project marker new 5:1:1 "]);
    assert_eq!(labels(&m), ["marker new"]);
    let set = menu(json!({ "kind": "marker-add", "tc": "5:1:1", "loop": "set" }));
    assert_eq!(set.last().unwrap().label.key.as_deref(), Some("ctx.loopSet"));
    assert_eq!(set.last().unwrap().command, "transport loop");
    let clear = menu(json!({ "kind": "marker-add", "tc": "5:1:1", "loop": "clear" }));
    assert_eq!(
        clear.last().unwrap().description.key.as_deref(),
        Some("ctx.loopClearHelp")
    );
    assert_eq!(
        lines(&menu(json!({ "kind": "marker", "name": "Chorus" }))),
        ["project marker delete Chorus"]
    );
}

#[test]
fn a_warp_point_offers_only_its_removal() {
    let m = menu(json!({ "kind": "warp-point", "clip": "break", "src": "1.94" }));
    assert_eq!(lines(&m), ["clip warp remove break 1.94"]);
    assert_eq!(labels(&m), ["warp remove"]);
}

#[test]
fn a_roll_note_offers_its_removal_a_split_only_inside_it_and_the_rectangles_verbs() {
    let note = |extra: Value| {
        let mut c = json!({ "kind": "note", "sequence": "melody", "note": 60, "start": 4, "end": 6 });
        for (k, v) in extra.as_object().unwrap() {
            c[k] = v.clone();
        }
        menu(c)
    };
    assert_eq!(lines(&note(json!({}))), ["sequence note remove melody 60 4"]);
    assert!(note(json!({})).iter().all(|i| i.complete));
    assert!(lines(&note(json!({ "at": 5 }))).contains(&"sequence note split melody 60 4 5"));
    assert_eq!(note(json!({ "at": 4 })).len(), 1);
    assert_eq!(note(json!({ "at": 6 })).len(), 1);
    let boxed = note(json!({ "region": { "lo": 60, "hi": 72, "from": 0, "to": 8 } }));
    let l = lines(&boxed);
    assert!(l.contains(&"sequence notes remove melody 60 72 0 8"));
    assert!(l.contains(&"sequence notes transpose melody 60 72 0 8 "));
    let by = |c: &str| boxed.iter().find(|i| i.command.starts_with(c)).unwrap();
    assert!(by("sequence notes remove").complete);
    assert!(!by("sequence notes transpose").complete);
}

#[test]
fn a_rack_pad_offers_its_edits_by_name_in_the_order_a_kit_is_edited() {
    let m = menu(json!({ "kind": "tap", "id": "kit", "symbol": "Hi Hat" }));
    let keys: Vec<_> = m.iter().map(|i| i.label.key.clone().unwrap()).collect();
    assert_eq!(
        keys,
        [
            "ctx.tapNote",
            "ctx.tapSample",
            "ctx.tapRename",
            "ctx.tapRemove",
            "ctx.tapBefore",
            "ctx.tapAfter",
            "ctx.tapChain"
        ]
    );
    assert_eq!(m[0].command, "node tap note kit \"Hi Hat\" ");
    assert!(!m[0].complete);
    assert!(m[3].complete);
    assert_eq!(m[4].command, "node tap insert kit before \"Hi Hat\"");
}

#[test]
fn a_picture_lane_offers_a_song_lane_its_own_rows_and_a_timelines_lane_the_shared_ones() {
    let list = json!([
        { "command": "timeline lane copy", "help": "copy a lane", "menu": ["track", "picture"],
          "args": [arg("lane", "track", Some("tracks")), arg("to", "choice", Some("laneTargets"))] },
        { "command": "picture mute", "help": "hide a lane", "args": [arg("lane", "file", Some("pictureLanes"))] },
        { "command": "picture lane delete", "help": "delete a lane", "menu": ["picture"], "confirm": true,
          "args": [arg("lane", "file", Some("pictureLanes"))] },
    ]);
    let commands: Vec<CommandInfo> = serde_json::from_value(list).unwrap();
    let song = open(
        json!({ "kind": "picture-lane", "lane": "cam" }),
        &commands,
        json!({}),
        "en",
    );
    assert_eq!(
        lines(&song),
        [
            "picture lane order cam ",
            "timeline lane copy cam main",
            "picture lane rename cam ",
            "timeline lane copy cam ",
            "picture mute cam",
            "picture solo cam",
            "picture lane delete cam",
            "picture add cam ",
            "ui picture arm cam",
            "ui file cam"
        ]
    );
    assert_eq!(song[1].label.key.as_deref(), Some("ctx.duplicate"));
    assert_eq!(
        song.iter().find(|i| i.verb == "picture lane delete").unwrap().confirm,
        Some(true)
    );
    let tl = open(
        json!({ "kind": "picture-lane", "lane": "verse>cam" }),
        &commands,
        json!({}),
        "en",
    );
    assert_eq!(
        lines(&tl),
        [
            "picture lane rename verse>cam ",
            "timeline lane copy verse>cam ",
            "picture lane delete verse>cam",
            "picture add verse>cam ",
            "ui file timelines/verse.k7x"
        ]
    );
}

#[test]
fn a_picture_shot_is_addressed_by_its_lane_and_start_in_seconds() {
    let m = menu(json!({ "kind": "picture-shot", "lane": "main", "at": "12.5", "name": "shot_a" }));
    assert_eq!(
        lines(&m),
        [
            "picture move main 12.5 ",
            "picture head main 12.5 ",
            "picture length main 12.5 ",
            "picture silence main 12.5",
            "picture delete main 12.5"
        ]
    );
    assert_eq!(labels(&m), ["move", "head", "length", "silence", "delete"]);
}

#[test]
fn a_rack_node_is_offered_the_racks_verbs_and_another_node_is_not() {
    let rack_verb = json!([{ "command": "node tap add", "help": "add a sound", "menu": ["rack"],
        "args": [arg("node", "module", Some("nodes")), arg("sound", "text", None)] }]);
    let commands: Vec<CommandInfo> = serde_json::from_value(rack_verb).unwrap();
    let on = |ctx: Value| open(ctx, &commands, json!({}), "en");
    assert!(
        on(json!({ "kind": "module", "id": "kit", "rack": true }))
            .iter()
            .any(|i| i.verb == "node tap add")
    );
    assert!(
        !on(json!({ "kind": "module", "id": "kit" }))
            .iter()
            .any(|i| i.verb == "node tap add")
    );
}

#[test]
fn the_catalogues_menu_list_narrows_and_a_persons_list_overrules_it() {
    let list = json!([{ "command": "event remove", "help": "x", "menu": ["event"],
        "args": [arg("track", "track", Some("tracks")), arg("timecode", "timecode", None)] }]);
    let commands: Vec<CommandInfo> = serde_json::from_value(list).unwrap();
    let on = |ctx: Value, prefs: Value| open(ctx, &commands, prefs, "en");
    let track = json!({ "kind": "track", "id": "gtr" });
    let clip = json!({ "kind": "clip", "id": "riff", "track": "gtr", "tc": "2:1:1" });
    // Kept off the lane that carries the event, left where the click is one.
    assert!(!on(track.clone(), json!({})).iter().any(|i| i.verb == "event remove"));
    assert!(on(clip.clone(), json!({})).iter().any(|i| i.verb == "event remove"));
    // A person's own list replaces it, and the plain off switch still wins.
    assert!(
        on(track.clone(), json!({ "event remove": { "menu": ["track"] } }))
            .iter()
            .any(|i| i.verb == "event remove")
    );
    assert!(
        !on(clip, json!({ "event remove": { "menu": false } }))
            .iter()
            .any(|i| i.verb == "event remove")
    );
}

#[test]
fn what_a_person_turned_off_goes_and_an_absent_entry_or_a_true_stays() {
    let off = |prefs: Value| menu_with(prefs);
    fn menu_with(prefs: Value) -> Vec<String> {
        open(json!({ "kind": "track", "id": "gtr" }), &catalogue(), prefs, "en")
            .into_iter()
            .map(|i| i.verb)
            .collect()
    }
    assert!(!off(json!({ "track mute": { "menu": false } })).contains(&"track mute".to_owned()));
    // The hand-written entries obey too.
    assert!(!off(json!({ "ui file": { "menu": false } })).contains(&"ui file".to_owned()));
    assert!(off(json!({})).contains(&"track mute".to_owned()));
    assert!(off(json!({ "track mute": { "menu": true } })).contains(&"track mute".to_owned()));
}

#[test]
fn two_rows_that_would_read_alike_keep_every_head() {
    let list = json!([
        { "command": "track mute", "help": "a", "args": [arg("track", "track", Some("tracks"))] },
        { "command": "mute", "help": "b", "args": [arg("track", "track", Some("tracks"))] },
    ]);
    let commands: Vec<CommandInfo> = serde_json::from_value(list).unwrap();
    let m = open(json!({ "kind": "track", "id": "gtr" }), &commands, json!({}), "en");
    assert_eq!(labels(&m), ["track mute", "mute", "ui file"]);
}

#[test]
fn a_row_asks_first_only_when_the_catalogue_says_so() {
    let list = json!([
        { "command": "track delete", "help": "x", "confirm": true, "args": [arg("track", "track", Some("tracks"))] },
        { "command": "track mute", "help": "y", "args": [arg("track", "track", Some("tracks"))] },
    ]);
    let commands: Vec<CommandInfo> = serde_json::from_value(list).unwrap();
    let m = open(json!({ "kind": "track", "id": "gtr" }), &commands, json!({}), "en");
    assert_eq!(m[0].confirm, Some(true));
    assert_eq!(m[1].confirm, None);
}

#[test]
fn a_row_carries_a_name_a_sentence_a_line_the_verb_and_whether_it_is_finished_and_nothing_else() {
    let m = menu(json!({ "kind": "track", "id": "gtr" }));
    let v = serde_json::to_value(&m[2]).unwrap();
    let mut keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(
        keys,
        ["command", "complete", "description", "label", "sentence", "verb"]
    );
    // A line is finished exactly when it does not end in a space.
    assert!(m.iter().all(|i| i.complete == !i.command.ends_with(' ')));
}

#[test]
fn an_automation_lane_offers_to_remove_its_automation_when_it_names_its_module() {
    let with =
        menu(json!({"kind": "curve-point", "curve": "sweep", "param": "Filter Freq", "tc": "1.1.1", "module": "amp"}));
    assert!(lines(&with).contains(&"node automate amp Filter Freq none"));
    let without = menu(json!({"kind": "curve-point", "curve": "sweep", "param": "Filter Freq", "tc": "1.1.1"}));
    assert!(!lines(&without).iter().any(|l| l.starts_with("node automate")));
}
