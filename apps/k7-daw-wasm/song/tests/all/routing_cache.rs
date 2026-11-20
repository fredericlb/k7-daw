// routing_cache.rs — the routing is read per compile and never joins the vocabulary's read set (decision 111).
// A reader would get wrong the direction: a draft of a routing file moves the routing and NOT the vocabulary, a library draft rebuilds it.

use crate::support;

use song::routing::load_routing;
use song::{Cache, MemSource};
use support::{mem, mem_world_roots};

const CHAIN: &str = "start(true)\nsteps([id(\"a\").extends(\"@k7-core/audio/amp\"), to([chain(\"OUT\")])])\n";
const AMP: &str = "plugin({ type: \"internal\", name: \"A\" })\n";

fn world() -> MemSource {
    mem(&[
        ("ws/proj/chains/a.k7x", CHAIN),
        ("ws/proj/tracks/gtr.k7x", "extends(\"@meta/track\")\n"),
        ("ws/proj/layout.k7x", "positions({ a: { x: 1, y: 2 } })\n"),
        ("lib/k7-core/modules/audio/amp/module.k7x", AMP),
    ])
}

fn dests(src: &MemSource, cache: &mut Cache) -> Vec<String> {
    let r = load_routing(src, cache, &mem_world_roots()).expect("routing");
    r.segments.modules.expect("modules")["a"].clone()
}

#[test]
fn a_draft_of_a_chain_file_changes_the_routing_and_does_not_rebuild_the_vocabulary() {
    let mut src = world();
    let roots = mem_world_roots();
    let mut cache = Cache::new();
    let first = cache.vocabulary(&src, &roots).expect("vocabulary");
    assert_eq!(dests(&src, &mut cache), ["OUT"]);
    let hits = cache.vocabulary_hits();
    src.set("/p/ws/proj/chains/a.k7x", Some(CHAIN.replace("OUT", "MIX")))
        .expect("draft");
    assert_eq!(dests(&src, &mut cache), ["MIX"]);
    let second = cache.vocabulary(&src, &roots).expect("vocabulary");
    assert!(std::sync::Arc::ptr_eq(&first, &second));
    assert_eq!(cache.vocabulary_hits(), hits + 1);
}

#[test]
fn a_draft_of_a_tracks_file_or_the_layout_does_not_rebuild_it_either() {
    let mut src = world();
    let roots = mem_world_roots();
    let mut cache = Cache::new();
    let first = cache.vocabulary(&src, &roots).expect("vocabulary");
    src.set(
        "/p/ws/proj/tracks/gtr.k7x",
        Some("extends(\"@meta/picture-track\")\n".into()),
    )
    .expect("draft");
    src.set(
        "/p/ws/proj/layout.k7x",
        Some("positions({ a: { x: 9, y: 9 } })\n".into()),
    )
    .expect("draft");
    let r = load_routing(&src, &mut cache, &roots).expect("routing");
    assert_eq!(r.segments.positions.expect("positions")["a"].x, 9.0);
    assert!(std::sync::Arc::ptr_eq(
        &first,
        &cache.vocabulary(&src, &roots).expect("vocabulary")
    ));
}

#[test]
fn a_draft_of_a_library_module_rebuilds_the_vocabulary() {
    let mut src = world();
    let roots = mem_world_roots();
    let mut cache = Cache::new();
    let first = cache.vocabulary(&src, &roots).expect("vocabulary");
    src.set(
        "/p/lib/k7-core/modules/audio/amp/module.k7x",
        Some(AMP.replace("\"A\"", "\"B\"")),
    )
    .expect("draft");
    let second = cache.vocabulary(&src, &roots).expect("vocabulary");
    assert!(!std::sync::Arc::ptr_eq(&first, &second));
}

#[test]
fn the_same_routing_is_read_again_from_the_bundle_a_read_made() {
    // The read set: every routing file goes through the Recorder, and an absent layout answers NotFound from its listings.
    let src = world();
    let rec = song::record::Recorder::new(&src);
    let mut cache = Cache::new();
    let first = load_routing(&rec, &mut cache, &mem_world_roots()).expect("routing");
    let roots = mem_world_roots();
    let (mounts, files) = roots.mounts().expect("mounts");
    let again = MemSource::from_bundle(rec.into_bundle_with_files(&mounts, &files)).expect("bundle");
    let second = load_routing(&again, &mut Cache::new(), &roots).expect("routing from the bundle");
    assert_eq!(first.segments, second.segments);
    assert_eq!(first.levels, second.levels);
}

#[test]
fn a_young_disk_stamp_is_never_a_key_and_an_aged_one_is() {
    // Decision 111 with the two-second rule: those cases run over a disk tree aged by `Tmp::age`.
    let t = support::Tmp::new("rc-age");
    t.write("ws/proj/chains/a.k7x", CHAIN);
    let roots = support::roots(&t);
    let source = support::disk(&roots);
    let mut cache = Cache::new();
    load_routing(&source, &mut cache, &roots).expect("routing");
    assert_eq!(cache.len(), 0);
    t.age();
    load_routing(&source, &mut cache, &roots).expect("routing");
    assert!(!cache.is_empty());
}
