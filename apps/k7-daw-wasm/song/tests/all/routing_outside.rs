// routing_outside.rs — what the Source refuses inside a chain project is skipped file by file, and a file has one spelling.
// A reader would get wrong that a link out of the mounts is the whole family's loss: only that file is a warning (decisions 24, 103).

use crate::support;

use song::Severity;
use song::routing::load_routing;
use song::{Cache, Finding};
use support::{Tmp, disk, mem, mem_world_roots, roots, skip};

const CHAIN: &str = "start(true)\nsteps([id(\"a\").extends(\"@k7-core/audio/amp\"), to([chain(\"OUT\")])])\n";

fn named(findings: &[Finding], severity: Severity) -> Vec<&str> {
    findings
        .iter()
        .filter(|f| f.severity == severity)
        .map(|f| f.file.as_str())
        .collect()
}

#[cfg(unix)]
fn link(t: &Tmp, from: &str, to: &str) -> bool {
    std::os::unix::fs::symlink(t.0.join(from), t.0.join(to)).is_ok()
}

#[cfg(not(unix))]
fn link(_: &Tmp, _: &str, _: &str) -> bool {
    false
}

#[test]
fn one_chain_file_linked_out_is_a_warning_and_the_others_still_read() {
    // Decision 24 over `chains/`: the good file stays, the refused one is absent and named.
    let t = Tmp::new("route-out-chain");
    t.write("ws/proj/chains/a.k7x", CHAIN);
    t.write("elsewhere/z.k7x", CHAIN);
    if !link(&t, "elsewhere/z.k7x", "ws/proj/chains/z.k7x") {
        skip("one_chain_file_linked_out", "this file system refuses links");
        return;
    }
    let r = load_routing(&disk(&roots(&t)), &mut Cache::new(), &roots(&t)).expect("routing");
    assert!(r.chain_project);
    assert_eq!(named(&r.findings, Severity::Warning), ["chains/z.k7x"]);
    assert!(named(&r.findings, Severity::Error).is_empty());
    assert_eq!(r.project.expect("project").chains.keys().collect::<Vec<_>>(), ["a"]);
    assert_eq!(r.segments.modules.expect("modules")["a"], ["OUT"]);
}

#[test]
fn a_layout_linked_out_is_a_warning_named_layout_and_no_positions() {
    // Decision 103: the pins are left out; the routing still reads, as it does for a `graph.k7x` linked out.
    let t = Tmp::new("route-out-layout");
    t.write("ws/proj/chains/a.k7x", CHAIN);
    t.write("elsewhere/layout.k7x", "positions({ a: { x: 1, y: 2 } })\n");
    if !link(&t, "elsewhere/layout.k7x", "ws/proj/layout.k7x") {
        skip("a_layout_linked_out", "this file system refuses links");
        return;
    }
    let r = load_routing(&disk(&roots(&t)), &mut Cache::new(), &roots(&t)).expect("routing");
    assert_eq!(named(&r.findings, Severity::Warning), ["layout.k7x"]);
    assert!(r.segments.positions.is_none());
    assert_eq!(r.segments.modules.expect("modules")["a"], ["OUT"]);
}

#[test]
fn a_file_has_one_spelling_in_every_finding_whatever_its_extension_case() {
    // Decision 95: `b.K7X` reads, and the duplicate-name error, the decode error and the node's label all name `chains/b.k7x`.
    let src = mem(&[
        ("ws/proj/chains/a.k7x", CHAIN),
        ("ws/proj/chains/b.K7X", CHAIN.replace("OUT", "MIX").as_str()),
        ("ws/proj/chains/c.K7X", "bogus(1)\n"),
    ]);
    let r = load_routing(&src, &mut Cache::new(), &mem_world_roots()).expect("routing");
    let mut files = named(&r.findings, Severity::Error);
    files.sort_unstable();
    assert_eq!(files, ["chains/b.k7x", "chains/c.k7x"]);
}
