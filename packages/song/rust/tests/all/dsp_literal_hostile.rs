// dsp_literal_hostile.rs — what a `dsp.k7x` cannot make the decoder do: read nothing, loop, blow up, recurse, or invent a node.
// A reader would get wrong that each refusal is generic: every one names the file, the line and the key path it is about.

use song::dsp_literal::{MAX_NESTING, MAX_NODES, MAX_REPEAT, decode};
use song::files::Finding;
use std::time::{Duration, Instant};

const HEAD: &str = "params({ Mix: { min: 0, max: 1, default: 0.5 } })\nwires(\"fb\")\n";

fn refused(graph: &str) -> Finding {
    let text = format!("{HEAD}graph([\n{graph}\n])\n");
    match decode("lib/m/dsp.k7x", &text) {
        Ok(json) => panic!("decoded:\n{json}"),
        Err(f) => f,
    }
}

fn says(f: &Finding, line: u32, words: &[&str]) {
    assert_eq!(f.file, "lib/m/dsp.k7x");
    assert_eq!(f.line, line, "{}", f.message);
    for w in words {
        assert!(f.message.contains(w), "{:?} lacks {w:?}", f.message);
    }
}

#[test]
fn a_reference_to_nothing_names_it() {
    let f = refused("  input().as(\"dry\"),\n  output(amul(\"dyr\", \"Mix\")),");
    says(&f, 5, &["graph[1].output[0].amul[0]", "`dyr` names nothing"]);
    assert_eq!(f.col, 15);
}

#[test]
fn a_cycle_without_a_wire_is_a_read_of_a_later_node() {
    let f = refused("  add(\"b\", 1).as(\"a\"),\n  add(\"a\", 1).as(\"b\"),");
    says(&f, 4, &["graph[0].add[0]", "`b` is named further down", "wires("]);
    let own = refused("  add(\"a\", 1).as(\"a\"),");
    says(&own, 4, &["`a` is named further down"]);
}

#[test]
fn a_wire_has_one_writer_and_is_written() {
    let twice = refused(
        "  input().as(\"x\"),\n  delay(\"x\", 1, 10).into(\"fb\"),\n  delay(\"x\", 1, 10).into(\"fb\"),\n  output(\"fb\"),",
    );
    says(&twice, 6, &["graph[2].delay", "already written"]);
    let never = refused("  output(input()),");
    says(&never, 2, &["wires", "`fb` is never written"]);
    let undeclared = refused("  input().as(\"x\"),\n  delay(\"x\", 1, 10).into(\"wet\"),");
    says(&undeclared, 5, &["`wet` is not a wire"]);
    let not_writer = refused("  amul(input(), 1).into(\"fb\"),");
    says(&not_writer, 4, &["only allpass and delay write a wire"]);
}

#[test]
fn a_huge_repeat_is_refused_before_it_runs() {
    let start = Instant::now();
    let f = refused("  input().as(\"x\"),\n  allpass(\"x\", 100).as(\"x\").repeat(1000000000),");
    assert!(start.elapsed() < Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 }));
    says(&f, 5, &["graph[1]", &format!("from 1 to {MAX_REPEAT}")]);
    let fraction = refused("  input().as(\"x\"),\n  allpass(\"x\", 100).as(\"x\").repeat(2.5),");
    says(&fraction, 5, &["whole count"]);
}

#[test]
fn many_repeats_stop_at_the_node_cap() {
    let item = format!("  allpass(\"x\", 100).as(\"x\").repeat({MAX_REPEAT}),\n");
    let graph = format!("  input().as(\"x\"),\n{}", item.repeat(MAX_NODES / MAX_REPEAT + 1));
    let start = Instant::now();
    let f = refused(&graph);
    assert!(start.elapsed() < Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 }));
    assert!(
        f.message.contains(&format!("more than {MAX_NODES} nodes")),
        "{}",
        f.message
    );
}

#[test]
fn a_repeat_names_only_its_own_node() {
    let f = refused("  input().as(\"x\"),\n  allpass(mul(\"x\", 1).as(\"y\"), 100).as(\"x\").repeat(2),");
    says(&f, 5, &["only the repeated node itself is named"]);
}

#[test]
fn deep_nesting_is_refused_by_name_and_by_the_parser() {
    let nest = |n: usize| format!("{}\"Mix\"{}", "todb(".repeat(n), ")".repeat(n));
    let fine = format!(
        "params({{ Mix: {{ min: 0, max: 1, default: 0 }} }})\ngraph([gain({})])",
        nest(MAX_NESTING - 1)
    );
    assert!(decode("m/dsp.k7x", &fine).is_ok());
    let f = refused(&format!("  gain({}),", nest(MAX_NESTING)));
    assert!(
        f.message.contains(&format!("nest deeper than {MAX_NESTING}")),
        "{}",
        f.message
    );
    let start = Instant::now();
    let parser = refused(&format!("  gain({}),", nest(100_000)));
    assert!(start.elapsed() < Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 }));
    says(&parser, 4, &["nested deeper than"]);
}

#[test]
fn an_unknown_node_type_lists_the_vocabulary() {
    let f = refused("  output(sqrt(input())),");
    says(&f, 4, &["graph[0].output[0]", "unknown node type `sqrt`", "allpass"]);
    let two = refused("  todb(4).glide(2),");
    says(&two, 4, &["`todb` and `glide` are two nodes"]);
    let top = decode("m/dsp.k7x", "graph([])\neval(\"x\")").expect_err("refused");
    assert!(top.message.contains("unknown key"), "{}", top.message);
}

#[test]
fn a_rate_mismatch_is_refused_at_its_input() {
    let audio_in_scalar = refused("  input().as(\"x\"),\n  gain(\"x\"),");
    says(&audio_in_scalar, 5, &["graph[1].gain[0]", "not an audio wire"]);
    let scalar_in_audio = refused("  output(\"Mix\"),");
    says(&scalar_in_audio, 4, &["expected an audio wire"]);
    let sink_read = refused("  output(amul(input(), gain(1))),");
    says(&sink_read, 4, &["leaves no value"]);
}

#[test]
fn names_cannot_collide_with_what_the_json_spells() {
    let id = refused("  input().as(\"n0\"),");
    says(&id, 4, &["`n0` cannot be a name"]);
    let sr = refused("  input().as(\"sr\"),");
    says(&sr, 4, &["`sr` cannot be a name"]);
    let param = refused("  input().as(\"Mix\"),");
    says(&param, 4, &["already a param"]);
    let twice = refused("  input().as(\"x\"),\n  input().as(\"x\"),");
    says(&twice, 5, &["names an earlier node"]);
}

#[test]
fn literals_are_checked_where_the_builder_checked_them() {
    let infinite = refused("  gain(1e999),");
    says(&infinite, 4, &["finite number"]);
    let descending = refused("  gain(geolerp(2000, 20, \"Mix\")),");
    says(&descending, 4, &["descending", "sub(1, t)"]);
    let arity = refused("  gain(lerp(1, 2)),");
    says(&arity, 4, &["`lerp` takes 3 inputs, not 2"]);
    let mode = refused("  biquad(\"comb\", 100, 1),");
    says(&mode, 4, &["expected one of bandpass"]);
    let shelf = refused("  biquad(\"peak\", 100, 1),");
    says(&shelf, 4, &["a peak biquad takes 4 inputs"]);
    let syntax = decode("m/dsp.k7x", "graph([input(dry)])").expect_err("refused");
    assert_eq!((syntax.line, syntax.file.as_str()), (1, "m/dsp.k7x"));
}
