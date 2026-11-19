// jsdsp_host.rs — what the compile says of a DSP module: its `dsp.k7x` decoded, or a legacy jsdsp.json that plays and is never rebuilt.
// A reader would get wrong that the compile builds anything: a graph that cannot play is one ERROR on the module and a pass-through.

use crate::doc_support;

use doc_support::{laid, world_request};
use song::doc::{Compiled, Vsts, compile};
use song::files::Severity;
use song::views::answer::{Want, answer};
use song::{Cache, MemSource};

const PASS: &str = "{\"params\":[],\"nodes\":[]}\n";
const MODULE: &str = "lib/k7-core/modules/audio/fx/module.k7x";
const LABEL: &str = "nodes/fx.k7x";

fn mem(files: &[(&str, &str)]) -> MemSource {
    let owned = laid(files);
    let refs: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    crate::support::mem(&refs)
}

fn world(extra: &[(&str, &str)]) -> MemSource {
    let mut files = vec![
        ("nodes/gtr.k7x", "extends(\"@meta/track\")\n"),
        ("nodes/fx.k7x", "extends(\"@k7-core/audio/fx\")\n"),
        ("graph.k7x", "modules({ gtr: [\"fx\"], fx: [\"MIX\"] })\n"),
        (MODULE, "plugin({ type: \"dsp\", name: \"Fx\" })\n"),
    ];
    files.extend_from_slice(extra);
    mem(&files)
}

fn run(extra: &[(&str, &str)]) -> Compiled {
    let req = world_request(&serde_json::json!({}), Vsts::Scan, &[]);
    compile(&world(extra), &mut Cache::new(), &req).expect("a compile")
}

fn graph_of(c: &Compiled) -> Option<String> {
    let doc = c.engine.as_ref().expect("a document");
    doc.tracks[0].plugins[0].dsp_graph.clone()
}

fn errors(c: &Compiled) -> Vec<String> {
    c.diagnostics
        .iter()
        .filter(|f| f.severity == Severity::Error && f.file == LABEL)
        .map(|f| f.message.clone())
        .collect()
}

#[test]
fn a_dsp_k7x_beside_its_module_plays_as_the_graph_it_decodes_with_no_word() {
    let c = run(&[(
        "lib/k7-core/modules/audio/fx/dsp.k7x",
        "params({ Mix: { min: 0, max: 1, default: 0.5 } })\ngraph([mix(\"Mix\")])\n",
    )]);
    let graph = graph_of(&c).expect("a graph");
    assert!(graph.contains("\"type\": \"mix\""), "{graph}");
    assert!(c.diagnostics.is_empty(), "{:?}", c.diagnostics);
}

#[test]
fn a_dsp_k7x_that_does_not_decode_is_one_error_with_its_line_and_the_sound_passes_through() {
    let c = run(&[(
        "lib/k7-core/modules/audio/fx/dsp.k7x",
        "params({})\ngraph([\n  output(amul(\"dyr\", 1)),\n])\n",
    )]);
    assert_eq!(graph_of(&c).as_deref(), Some(PASS));
    let said = errors(&c);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].contains("(line 3: ") && said[0].contains("`dyr` names nothing"),
        "{said:?}"
    );
}

#[test]
fn a_module_with_no_dsp_k7x_passes_the_sound_through_and_says_so() {
    let c = run(&[]);
    assert_eq!(graph_of(&c).as_deref(), Some(PASS));
    assert_eq!(errors(&c).len(), 1);
}

#[test]
fn the_error_list_carries_a_bad_dsp_k7x_once() {
    let src = world(&[("lib/k7-core/modules/audio/fx/dsp.k7x", "graph([nope()])\n")]);
    let req = world_request(&serde_json::json!({}), Vsts::Scan, &[]);
    let want = Want {
        errors: true,
        ..Want::default()
    };
    let views = answer(&src, &mut Cache::new(), &req, &want).expect("views");
    let errors = views.errors.expect("errors");
    let said = errors
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("does not build"))
        .count();
    assert_eq!(said, 1, "{:?}", errors.diagnostics);
}
