// graph_nodes.rs — M5's node and graph verbs on a graph project (note-lane) and a chain project (chain-lanes): files, doc, undo.
// A reader would get wrong what a node new changes in the doc: nothing, since a node wired to nothing plays nowhere.

mod support;

use song_edit::Fs;
use support::Project;

fn plugins(p: &mut Project, lane: &str) -> usize {
    p.track(lane).plugins.len()
}

#[test]
fn a_new_node_is_written_and_pinned_wired_to_nothing() {
    let mut p = Project::open("note-lane");
    let doc = p.doc();
    let ran = p.round_trip("node new audio/compressor comp");
    assert_eq!(
        ran.answer.log,
        ["created nodes/comp.k7x — in the graph, wired to nothing yet"]
    );
    assert_eq!(ran.changed(&p.dir), ["graph.k7x", "nodes/comp.k7x"]);
    p.ok("node new audio/compressor comp 40,50");
    assert!(
        p.text("nodes/comp.k7x")
            .contains("extends(\"@workspace/audio/compressor\")")
    );
    assert!(p.text("graph.k7x").contains("comp"), "the graph names the new node");
    assert_eq!(p.doc().tracks, doc.tracks, "a node wired to nothing plays nowhere");
    p.ok("node new instruments/FourOsc");
    assert!(
        p.exists("nodes/fourosc_1.k7x"),
        "the case of a typed module is the folder's"
    );
    p.refused("node new audio/compressor comp", "already exists");
    p.refused(
        "node new audio/nothing",
        "is neither in the workspace nor one the engine offers",
    );
    p.refused("node new nothing", "is not <category>/<name>");
    p.refused("node new @nowhere/at-all x", "is not a module this project can see");
}

#[test]
fn a_node_on_a_ref_extends_it_with_no_module_between() {
    let mut p = Project::open("note-lane");
    let ran = p.round_trip("node new @workspace/audio/compressor squash");
    assert_eq!(
        ran.answer.log,
        ["created nodes/squash.k7x on @workspace/audio/compressor — in the graph, wired to nothing yet"]
    );
    assert!(!ran.changed(&p.dir).iter().any(|f| f.contains("_modules")));
}

#[test]
fn a_new_node_in_a_chain_project_is_a_loose_chain() {
    let mut p = Project::open_crate("chain-lanes");
    let ran = p.round_trip("node new audio/compressor squash");
    let changed = ran.changed(&p.dir);
    assert!(changed.contains(&"chains/squash.k7x".to_owned()), "{changed:?}");
    assert!(
        ran.answer.log[0].starts_with("created @workspace/audio/compressor · squash · step 1 (id-"),
        "{:?}",
        ran.answer.log
    );
}

#[test]
fn an_insert_splices_an_existing_node_into_an_arrow() {
    let mut p = Project::open("note-lane");
    p.ok("node new audio/compressor comp");
    assert_eq!(plugins(&mut p, "melody"), 1);
    let ran = p.round_trip("node insert lead MIX comp");
    assert_eq!(ran.answer.log, ["inserted comp between lead and MIX"]);
    p.ok("node insert lead MIX comp");
    assert_eq!(plugins(&mut p, "melody"), 2, "the compressor plays after the synth");
    p.refused("node insert lead OUT comp", "'lead' does not feed 'OUT'");
    p.refused("node insert lead comp ghost", "no node \"ghost\" in this project");
    p.refused("node insert lead", "usage: node insert");
    let ran = p.round_trip("graph unlink comp");
    assert_eq!(
        ran.answer.log,
        ["unlinked comp and rejoined what fed it: lead; it stays in the graph, wired to nothing"]
    );
    p.ok("graph unlink comp");
    assert_eq!(plugins(&mut p, "melody"), 1);
    assert!(p.exists("nodes/comp.k7x"), "an unlink leaves the file");
}

#[test]
fn the_master_run_drawn_but_unwritten_is_spliced_into() {
    let mut p = Project::open_crate("chain-lanes");
    p.ok("node new audio/compressor glue");
    let ran = p.round_trip("node insert MIX OUT glue");
    assert_eq!(
        ran.answer.log,
        ["inserted id-1 between MIX and OUT"],
        "a chain project's node is named by its id"
    );
    p.ok("node insert MIX OUT glue");
    assert_eq!(p.doc().master_plugins.len(), 1, "the master run plays the new node");
}

#[test]
fn a_delete_splices_the_node_out_unless_it_starves_one() {
    let mut p = Project::open("note-lane");
    p.refused(
        "node delete melody",
        "is the only signal reaching lead, whose input is mandatory",
    );
    let ran = p.round_trip("node delete lead");
    assert_eq!(
        ran.answer.log,
        ["deleted nodes/lead.k7x — rejoined what fed it: melody"]
    );
    assert_eq!(ran.changed(&p.dir), ["graph.k7x", "nodes/lead.k7x"]);
    p.ok("node delete lead");
    assert_eq!(plugins(&mut p, "melody"), 0);
    p.refused("node delete lead", "node delete failed");
}

#[test]
fn a_replace_repoints_the_extends_and_a_made_name_follows() {
    let mut p = Project::open("note-lane");
    let ran = p.round_trip("node replace lead @workspace/instruments/synth");
    assert_eq!(
        ran.answer.log,
        ["lead: extends @workspace/instruments/synth (nodes/lead.k7x)"]
    );
    p.ok("node new instruments/fourosc");
    p.ok("node insert melody lead fourosc_1");
    let ran = p.round_trip("node replace fourosc_1 @workspace/instruments/synth");
    assert_eq!(
        ran.answer.log[1],
        "fourosc_1 → synth_1, following the module it now extends (rewired melody)"
    );
    p.ok("node replace fourosc_1 @workspace/instruments/synth");
    assert!(p.exists("nodes/synth_1.k7x") && !p.exists("nodes/fourosc_1.k7x"));
    p.refused("node replace lead @nowhere/x", "node replace failed: unknown module");
    p.refused("node replace lead", "usage: node replace");
}

#[test]
fn a_stack_is_a_subchain_of_a_chain_project() {
    let mut p = Project::open_crate("chain-lanes");
    p.ok("node new audio/compressor squash");
    let comp = "00000000-0000-4000-8000-0000000000d2";
    let ran = p.round_trip(&format!("node stack {comp} squash"));
    assert!(
        ran.answer.log[0].starts_with(&format!("stacked id-1 under {comp} (")),
        "{:?}",
        ran.answer.log
    );
    p.ok(&format!("node stack {comp} squash"));
    assert!(p.text("chains/drums.k7x").contains("subchains"));
    assert!(!p.exists("chains/squash.k7x"), "the loose chain is folded in");
    assert_eq!(plugins(&mut p, "drums"), 2);
    let mut g = Project::open("note-lane");
    g.refused("node stack lead lead", "only a chain project has them");
}

#[test]
fn pins_moves_and_organize_write_the_positions() {
    let mut p = Project::open("note-lane");
    let ran = p.round_trip(r#"graph pin {"lead":{"x":420,"y":180},"melody":{"x":"10","y":-5},"bad":{"x":"no"}}"#);
    assert_eq!(ran.answer.log, ["pinned 2 node(s) (graph.k7x)"]);
    p.ok(r#"graph pin {"lead":{"x":420,"y":180}}"#);
    assert!(p.text("graph.k7x").contains("420"));
    p.refused("graph pin {nope", "bad positions JSON");
    p.refused("graph pin [1]", "must be a JSON object");
    p.refused(r#"graph pin {"a":{}}"#, "no usable {x,y}");
    let ran = p.round_trip("graph move lead -20.4 33.6");
    assert_eq!(ran.answer.log, ["lead: pinned at -20,34"]);
    p.ok("graph move OUT 5 5");
    p.ok("graph move lead auto");
    p.refused("graph move ghost 1 1", "no node \"ghost\" in this project (modules: ");
    p.refused("graph move lead 1", "usage: graph move");
    let ran = p.round_trip("graph organize");
    assert_eq!(
        ran.answer.log,
        ["graph reorganized — every node handed back to the layout"]
    );
    p.ok("graph organize");
    assert!(!p.text("graph.k7x").contains("positions"));
}

#[test]
fn cables_connect_disconnect_and_replug() {
    let mut p = Project::open("note-lane");
    let ran = p.round_trip("graph chain disconnect lead MIX");
    assert_eq!(ran.answer.log, ["disconnected lead → MIX"]);
    p.ok("graph chain disconnect lead MIX");
    p.ok("node new audio/compressor comp");
    let ran = p.round_trip("graph chain connect lead comp");
    assert_eq!(ran.answer.log, ["connected lead → comp"]);
    p.ok("graph chain connect lead comp");
    p.ok("graph chain connect comp MIX");
    assert_eq!(plugins(&mut p, "melody"), 2);
    let ran = p.round_trip("graph chain replug comp MIX comp OUT");
    assert_eq!(ran.answer.log, ["replugged: comp → MIX is now comp → OUT"]);
    p.refused("graph chain connect lead lead", "can't connect 'lead' to itself");
    p.refused("graph chain connect OUT comp", "OUT is the sink");
    p.refused(
        "graph chain connect comp melody",
        "'melody' is a track — only a live input's signal lands on a lane",
    );
    p.refused("graph chain connect comp ghost", "no node \"ghost\"");
    p.refused("graph chain replug comp MIX comp MIX", "nothing moved");
    p.refused(
        "graph chain replug comp MIX comp melody",
        "'melody' is a track — a source",
    );
    p.refused("graph chain disconnect lead MIX", "graph chain disconnect failed");
}

#[test]
fn a_module_is_created_owned_and_cloned() {
    let mut p = Project::open("note-lane");
    let ran = p.round_trip("module new audio/gate");
    assert_eq!(
        ran.answer.log[0],
        "created ../_modules/audio/gate/module.k7x — add an inline plugin/rack (or an `extends`) to give it a sound"
    );
    assert_eq!(ran.changed(&p.dir), ["/w/ws/_modules/audio/gate/module.k7x"]);
    p.ok("module new audio/squash @workspace/audio/compressor");
    assert!(
        p.text("../_modules/audio/squash/module.k7x")
            .contains("extends(\"@workspace/audio/compressor\")")
    );
    p.ok("node new audio/squash sq");
    p.refused("module new audio/squash", "audio/squash is already here");
    p.refused(
        "module new audio/squash @workspace/audio/gate",
        "extends '@workspace/audio/compressor', not '@workspace/audio/gate'",
    );
    p.refused("module new io/x", "an io module is made with its instance");
    p.refused("module new audio/x @nowhere/y", "unknown module '@nowhere/y'");
    let ran = p.round_trip("module clone audio/compressor squeeze");
    assert_eq!(
        ran.answer.log,
        ["cloned audio/compressor → ../_modules/audio/squeeze/module.k7x"]
    );
    p.refused("module clone audio/ghost ghost2", "module 'audio/ghost' not found");
    let ran = p.round_trip("module own @engine/compressor");
    assert_eq!(
        ran.answer.log,
        ["@engine/compressor is now ../_modules/audio/compressor_2/module.k7x"],
        "the workspace's own name is taken"
    );
    p.refused("module own @engine/no-such", "module own failed");
    p.refused("module fork @workspace/audio/compressor", "module fork: ");
}

#[test]
fn an_assembly_instances_its_template_and_goes_whole() {
    let mut p = Project::open("note-lane");
    let lib = "/home-none/library/assemblies/duo";
    let files = [
        ("assembly.k7x", "label(\"Duo\")\n"),
        ("graph.k7x", "modules({ a: [\"b\"], b: [\"MIX\"] })\n"),
        ("nodes/a.k7x", "extends(\"@workspace/audio/compressor\")\n"),
        ("nodes/b.k7x", "extends(\"@workspace/audio/compressor\")\n"),
    ];
    for (rel, text) in files {
        let path = format!("{lib}/{rel}");
        p.fs.mkdir_all(path.rsplit_once('/').map_or("", |(d, _)| d))
            .expect("a template folder");
        p.fs.write_atomic(&format!("{lib}/{rel}"), text.as_bytes())
            .expect("a template file");
    }
    let ran = p.round_trip("assembly new @account/duo pair 10,20");
    assert!(
        ran.answer.log[0].ends_with("— 2 nodes, drawn as one box"),
        "{:?}",
        ran.answer.log
    );
    p.ok("assembly new @account/duo pair");
    p.ok("graph chain replug lead MIX lead pair_a");
    assert_eq!(plugins(&mut p, "melody"), 3, "the box's two nodes play after the synth");
    p.refused("assembly new @account/duo pair", "assembly new failed");
    let ran = p.round_trip("assembly dissolve pair");
    assert!(
        ran.answer.log[0].ends_with("its 2 nodes are on the graph, untouched"),
        "{:?}",
        ran.answer.log
    );
    let ran = p.round_trip("assembly delete pair");
    assert!(
        ran.answer.log[0].ends_with("and 2 of its files"),
        "{:?}",
        ran.answer.log
    );
    p.refused("assembly delete ghost", "assembly delete failed");
    p.refused("assembly new @account/duo", "usage: assembly new");
}

#[test]
fn a_chain_save_is_the_bench_route_s() {
    use serde_json::json;
    let mut p = Project::open("note-lane");
    let ran = p.ok_answered("chain save warm", |c| {
        assert_eq!(c["route"], "POST /bench/save");
        assert_eq!(c["args"], json!({ "name": "warm" }));
        json!({ "ref": "@account/chains/warm", "steps": 2, "outside": ["meter"] })
    });
    assert_eq!(ran.before, ran.after);
    assert_eq!(
        ran.answer.log,
        [
            "saved @account/chains/warm (2 steps) — clip process <clip> @account/chains/warm",
            "not in it: meter — only what sits between IN and fader is the chain"
        ]
    );
    let bare = p.ok_answered("chain save", |c| {
        assert_eq!(c["args"], json!({}));
        json!({ "ref": "@account/chains/bench", "steps": 0, "outside": [] })
    });
    assert_eq!(bare.answer.log, ["saved @account/chains/bench: its input alone"]);
    let failed = p.answered("chain save x", |_| json!({ "error": "no bench is open" }));
    assert_eq!(failed.answer.refuse, ["chain save: no bench is open"]);
}
