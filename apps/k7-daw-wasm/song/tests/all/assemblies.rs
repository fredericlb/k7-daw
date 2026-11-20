// assemblies.rs — a project's `assemblies/` against the library's templates (decisions 273 to 278): assemblies.test.ts and its siblings.
// A reader would get wrong that an assembly changes nothing about the chains: it is a DRAWING, and a test proves it (278).

use crate::lanes_support;
use crate::support;

use lanes_support::{fast, inputs};
use song::assemblies::{
    ASSEMBLY_FILE, BUDGET, Loaded, assembly_folder, assembly_issues, decode_assembly, decode_assembly_template,
    load_template, project_assemblies, resolve_assemblies,
};
use song::lanes::ownership::chain_ownership;
use song::modules::assets::Places;
use song::{Cache, Severity};
use support::{Tmp, disk, mem, mem_world_roots, roots};

const TEMPLATE: &str = "label(\"Turntable\")\noutputs([\n  name(\"mix\").type(\"audio\").node(\"fade\"),\n  name(\"cue\").type(\"audio\").node(\"cue\"),\n])\nparams({\n  Level: { node: \"level\", param: \"Gain\" },\n  Fade: { node: \"fade\", param: \"Level\" },\n})\n";
const INSTANCE: &str = "uses(\"@k7-core/turntable\")\nmembers({ level: \"levelA\", fade: \"fadeA\", cue: \"cueA\" })\n";
const LIB: &str = "lib/k7-core/assemblies/turntable/assembly.k7x";

/// The project's files (below the project) and library files (`lib/...`), read through memory with the world's roots.
fn resolved(files: &[(&str, &str)]) -> song::assemblies::ResolvedAssemblies {
    let owned = lanes_support::laid(files);
    let refs: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    resolve_assemblies(&mem(&refs), &mut Cache::new(), &mem_world_roots()).expect("resolved")
}

fn finding_keys(r: &song::assemblies::ResolvedAssemblies) -> Vec<String> {
    let mut keys: Vec<String> = r
        .findings
        .iter()
        .map(|f| {
            format!(
                "{}|{}",
                f.file,
                if f.severity == Severity::Error {
                    "error"
                } else {
                    "warning"
                }
            )
        })
        .collect();
    keys.sort();
    keys
}

// assemblies.test.ts "reads a template's plugs and promoted params".
#[test]
fn a_template_reads_its_plugs_and_promoted_params() {
    let t = decode_assembly_template("assembly.k7x", TEMPLATE).expect("a template");
    assert_eq!(t.label.as_ref().map(|l| l.0.as_str()), Some("Turntable"));
    let first = &t.outputs.as_ref().expect("outputs")[0];
    assert_eq!((first.name.0.as_str(), first.node.0.as_str()), ("mix", "fade"));
    assert_eq!(t.params.as_ref().expect("params")["Level"].param.0, "Gain");
}

// assemblies.test.ts "refuses two plugs called the same thing on one side" (273): on the second plug's name, line 3.
#[test]
fn two_plugs_of_one_name_are_refused_on_the_second() {
    let text =
        "outputs([\n  name(\"mix\").type(\"audio\").node(\"a\"),\n  name(\"mix\").type(\"audio\").node(\"b\"),\n])\n";
    let f = decode_assembly_template("assembly.k7x", text).expect_err("refused");
    assert_eq!((f.line, f.severity), (3, Severity::Error));
    // The same name on the two sides is two different addresses.
    let ok =
        "inputs([name(\"mix\").type(\"audio\").node(\"a\")])\noutputs([name(\"mix\").type(\"audio\").node(\"b\")])\n";
    assert!(decode_assembly_template("assembly.k7x", ok).is_ok());
}

// assemblies.test.ts "reads an instance's template ref and members", "names an unknown key rather than ignoring it".
#[test]
fn an_instance_reads_and_an_unknown_key_is_named() {
    let a = decode_assembly("assemblies/a.k7x", INSTANCE).expect("an instance");
    assert_eq!(a.uses.0, "@k7-core/turntable");
    assert_eq!(a.members["fade"], "fadeA");
    let f = decode_assembly("assemblies/a.k7x", &format!("{INSTANCE}colour(\"red\")\n")).expect_err("refused");
    assert_eq!(f.line, 3);
}

// assemblyYaml.test.ts "names what an empty file lacks, and a syntax error's place" (273).
#[test]
fn an_empty_file_and_a_syntax_error_are_findings_with_a_place() {
    for (f, text) in [("a", ""), ("a", "\n\n")] {
        assert!(decode_assembly(f, text).is_err() && decode_assembly_template(f, text).is_err());
    }
    let bad = decode_assembly("a.k7x", "uses(\"x\"\n").expect_err("syntax");
    assert!(bad.line >= 1);
    // The template's refinements are checks: an empty name, an empty member key.
    assert!(decode_assembly_template("a", "outputs([name(\"\").type(\"audio\").node(\"a\")])\n").is_err());
    assert!(decode_assembly("a", "uses(\"@k7-core/x\")\nmembers({ \"\": \"y\" })\n").is_err());
    assert!(decode_assembly("a", "uses(\"@k7-core/x\")\nmembers({ a: \"\" })\n").is_err());
    assert!(decode_assembly_template("a", "label(\"\")\n").is_err());
    assert!(decode_assembly_template("a", "params({ \"\": { node: \"a\", param: \"b\" } })\n").is_err());
}

// assemblies.test.ts "turns every template name into this project's node name".
#[test]
fn resolving_turns_template_names_into_this_projects_nodes() {
    let r = resolved(&[("assemblies/turntableA.k7x", INSTANCE), (LIB, TEMPLATE)]);
    assert!(r.findings.is_empty() && !r.capped);
    let a = &r.assemblies["turntableA"];
    assert_eq!(a.label, "Turntable");
    assert_eq!(
        a.outputs
            .iter()
            .map(|o| (o.name.as_str(), o.node.as_str(), o.mandatory))
            .collect::<Vec<_>>(),
        [("mix", "fadeA", true), ("cue", "cueA", true)]
    );
    assert_eq!(a.params["Level"].node.0, "levelA");
    assert_eq!(a.missing, None);
    assert!(a.inputs.is_empty());
    assert_eq!(serde_json::to_value(a).expect("json")["outputs"][0]["type"], "audio");
}

// assemblies.test.ts "drops a plug whose member this instance does not map" and `mandatory` is `!== false`.
#[test]
fn a_plug_or_param_whose_member_is_not_mapped_is_dropped_silently() {
    let only = "uses(\"@k7-core/turntable\")\nmembers({ fade: \"fadeA\" })\n";
    let r = resolved(&[("assemblies/turntableA.k7x", only), (LIB, TEMPLATE)]);
    let a = &r.assemblies["turntableA"];
    assert_eq!(a.outputs.iter().map(|o| o.name.as_str()).collect::<Vec<_>>(), ["mix"]);
    assert!(a.params.contains_key("Fade") && !a.params.contains_key("Level"));
    assert!(r.findings.is_empty());
    let optional = "outputs([name(\"o\").type(\"midi\").node(\"fade\").port(\"p\").mandatory(false)])\n";
    let r = resolved(&[("assemblies/t.k7x", only), (LIB, optional)]);
    let o = &r.assemblies["t"].outputs[0];
    assert_eq!((o.mandatory, o.port.as_deref()), (false, Some("p")));
}

// assemblies.test.ts "still yields the box when the template resolves to nothing" (275): missing, a WARNING on the instance's file.
#[test]
fn an_absent_template_is_missing_and_a_warning() {
    let r = resolved(&[("assemblies/turntableA.k7x", INSTANCE)]);
    let a = &r.assemblies["turntableA"];
    assert_eq!((a.missing, a.label.as_str()), (Some(true), "turntableA"));
    assert_eq!(a.members["fade"], "fadeA");
    assert!(a.outputs.is_empty());
    assert_eq!(finding_keys(&r), ["assemblies/turntableA.k7x|warning"]);
    assert_eq!(serde_json::to_value(a).expect("json")["missing"], true);
}

// 275: a template that does not decode is an EMPTY one with one ERROR on the instance's file at 1:1; the box still draws.
#[test]
fn a_template_with_a_schema_fault_or_empty_is_an_empty_one_and_an_error() {
    let bad = "plugs([])\n";
    let r = resolved(&[("assemblies/turntableA.k7x", INSTANCE), (LIB, bad)]);
    assert_eq!(finding_keys(&r), ["assemblies/turntableA.k7x|error"]);
    assert_eq!((r.findings[0].line, r.findings[0].col), (1, 1));
    let a = &r.assemblies["turntableA"];
    assert!(a.missing.is_none() && a.outputs.is_empty() && a.params.is_empty() && a.label == "turntableA");
    let empty = resolved(&[("assemblies/turntableA.k7x", INSTANCE), (LIB, "")]);
    assert_eq!(finding_keys(&empty), ["assemblies/turntableA.k7x|error"]);
    let twice = "outputs([name(\"m\").type(\"audio\").node(\"a\"), name(\"m\").type(\"audio\").node(\"b\")])\n";
    let r = resolved(&[("assemblies/turntableA.k7x", INSTANCE), (LIB, twice)]);
    assert_eq!(finding_keys(&r), ["assemblies/turntableA.k7x|error"]);
}

// assemblies.test.ts "reports a file that will not read and keeps the ones that do" (273): an empty file is a finding too.
#[test]
fn a_file_that_will_not_read_is_reported_and_the_rest_read() {
    let owned = lanes_support::laid(&[
        ("assemblies/turntableA.k7x", INSTANCE),
        ("assemblies/broken.k7x", "uses([])\n"),
        ("assemblies/blank.k7x", ""),
    ]);
    let refs: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let p = project_assemblies(&mem(&refs), &mut Cache::new(), "/p/ws/proj").expect("read");
    assert_eq!(
        p.assemblies.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(),
        ["turntableA"]
    );
    assert_eq!(p.assemblies[0].file, "assemblies/turntableA.k7x");
    let mut files: Vec<_> = p.findings.iter().map(|f| f.file.as_str()).collect();
    files.sort_unstable();
    assert_eq!(files, ["assemblies/blank.k7x", "assemblies/broken.k7x"]);
}

// Two instances of one template read it once from the cache and each draws its own box.
#[test]
fn two_instances_of_one_template_are_two_boxes() {
    let b = "uses(\"@k7-core/turntable\")\nmembers({ fade: \"fadeB\", cue: \"cueB\" })\n";
    let r = resolved(&[("assemblies/a.k7x", INSTANCE), ("assemblies/b.k7x", b), (LIB, TEMPLATE)]);
    assert_eq!(r.assemblies.keys().map(String::as_str).collect::<Vec<_>>(), ["a", "b"]);
    assert_eq!(r.assemblies["b"].outputs[0].node, "fadeB");
    assert_eq!(
        assembly_issues(&mem(&[]), &mut Cache::new(), &mem_world_roots())
            .map(|f| f.len())
            .ok(),
        Some(0)
    );
}

// assemblies.test.ts "an assembly is a DRAWING" (278): the same inputs, lanes and owners with and without `assemblies/`.
#[test]
fn an_assembly_changes_nothing_about_the_chains() {
    let base = [
        ("nodes/levelA.k7x", "plugin({ type: \"internal\", name: \"Chorus\" })\n"),
        ("nodes/fadeA.k7x", "plugin({ type: \"internal\", name: \"Chorus\" })\n"),
        ("nodes/gtr.k7x", "extends(\"@meta/track\")\n"),
        (
            "graph.k7x",
            "modules({ gtr: [\"levelA\"], levelA: [\"fadeA\"], fadeA: [\"MIX\"], MIX: [\"OUT\"] })\n",
        ),
    ];
    let mut with: Vec<(&str, &str)> = base.to_vec();
    with.push(("assemblies/turntableA.k7x", INSTANCE));
    with.push((LIB, TEMPLATE));
    let (a, b) = (inputs(&with), inputs(&base));
    let shape = |i: &song::lanes::inputs::Inputs| {
        let lanes = i.lanes();
        let own = chain_ownership(i, &lanes);
        (
            i.chains.clone(),
            i.local.modules.keys().cloned().collect::<Vec<_>>(),
            lanes.sound.keys().map(|k| (*k).to_owned()).collect::<Vec<_>>(),
            i.chains
                .keys()
                .map(|c| own.owner_of(c).map(str::to_owned))
                .collect::<Vec<_>>(),
            i.stops().len(),
        )
    };
    assert_eq!(shape(&a), shape(&b));
}

// 274: a template ref goes through the library ref, plain segments and the host's libraries only.
#[test]
fn a_hostile_template_ref_resolves_to_nothing() {
    let owned = lanes_support::laid(&[(LIB, TEMPLATE)]);
    let refs: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let src = mem(&refs);
    let places = Places::of(&mem_world_roots(), false).expect("places");
    let folder = |r: &str| assembly_folder(&src, &places, r).expect("answers");
    assert_eq!(
        folder("@k7-core/turntable").as_deref(),
        Some("/p/lib/k7-core/assemblies/turntable")
    );
    for bad in [
        "@k7-core/../../x",
        "@k7-core/./turntable",
        "@k7-core//turntable",
        "@project/x",
        "/etc/passwd",
        "@k7-core/a\\b",
        "@k7-core/a\0b",
        "@nolib/turntable",
        "@k7-core/",
        "@engine/turntable",
        "turntable",
    ] {
        assert_eq!(folder(bad), None, "{bad:?}");
    }
    // On Unix a backslash is a character of a name; on Windows the segment is refused: either way nothing resolves.
    assert!(matches!(
        load_template(&src, &mut Cache::new(), &mem_world_roots(), "@k7-core/missing").expect("loads"),
        Loaded::Missing
    ));
    assert!(matches!(
        load_template(&src, &mut Cache::new(), &mem_world_roots(), "@k7-core/turntable").expect("loads"),
        Loaded::Template { .. }
    ));
}

// 274: a run of `/` inside a ref is one separator, as the TypeScript's split on `[\\/]+` reads it; an empty first or last segment is refused.
#[test]
fn a_run_of_slashes_inside_a_template_ref_is_one_separator() {
    let owned = lanes_support::laid(&[("lib/k7-core/assemblies/a/b/assembly.k7x", TEMPLATE)]);
    let refs: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let src = mem(&refs);
    let places = Places::of(&mem_world_roots(), false).expect("places");
    let folder = |r: &str| assembly_folder(&src, &places, r).expect("answers");
    let want = Some("/p/lib/k7-core/assemblies/a/b".to_owned());
    for good in ["@k7-core/a/b", "@k7-core/a//b", "@k7-core/a///b"] {
        assert_eq!(folder(good), want, "{good:?}");
    }
    for bad in ["@k7-core/a/b/", "@k7-core//a/b", "@k7-core/a//../b", "@k7-core/a//./b"] {
        assert_eq!(folder(bad), None, "{bad:?}");
    }
}

// 295: DEPARTS. `@project/` is never a library, even when a library is really called `project`.
#[test]
fn a_library_called_project_is_not_reached_by_its_ref() {
    let files = [
        ("lib/project/assemblies/x/assembly.k7x", TEMPLATE),
        ("assemblies/x/assembly.k7x", TEMPLATE),
    ];
    let owned = lanes_support::laid(&files);
    let refs: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let src = mem(&refs);
    let places = Places::of(&mem_world_roots(), false).expect("places");
    assert_eq!(assembly_folder(&src, &places, "@project/x").expect("answers"), None);
}

// 274: the Windows branch refuses a segment holding `:` or `\`; on Unix a `:` is a plain character.
#[test]
fn a_colon_in_a_segment_is_refused_under_the_windows_rule_and_a_name_under_the_posix_one() {
    let owned = lanes_support::laid(&[("lib/k7-core/assemblies/a:b/assembly.k7x", TEMPLATE)]);
    let refs: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let src = mem(&refs);
    for rule in [song::PathRule::Posix, song::PathRule::Windows] {
        let roots = song::Roots {
            paths: rule,
            ..mem_world_roots()
        };
        let places = Places::of(&roots, false).expect("places");
        let found = assembly_folder(&src, &places, "@k7-core/a:b").expect("answers");
        assert_eq!(found.is_some(), !rule.is_windows(), "{rule:?}");
    }
}

// 275: a file the Source refuses (an instance linked out of the mounts) costs that file alone.
#[test]
fn an_instance_linked_out_of_the_mounts_is_skipped_with_a_warning() {
    let t = Tmp::new("asm-link");
    t.write("ws/proj/assemblies/good.k7x", INSTANCE);
    t.write("outside/bad.k7x", INSTANCE);
    #[cfg(unix)]
    let linked =
        std::os::unix::fs::symlink(t.0.join("outside/bad.k7x"), t.0.join("ws/proj/assemblies/bad.k7x")).is_ok();
    #[cfg(not(unix))]
    let linked = false;
    if !linked {
        return support::skip("an_instance_linked_out_of_the_mounts", "no links");
    }
    t.age();
    let r = roots(&t);
    let out = resolve_assemblies(&disk(&r), &mut Cache::new(), &r).expect("resolved");
    assert_eq!(out.assemblies.keys().map(String::as_str).collect::<Vec<_>>(), ["good"]);
    assert!(
        out.findings
            .iter()
            .any(|f| f.file == "assemblies/bad.k7x" && f.severity == Severity::Warning)
    );
}

/// The shipped turntable template, from where the manifest says the repository is.
fn turntable() -> (String, Vec<String>) {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../k7-core/assemblies/turntable");
    let base = std::fs::canonicalize(base).expect("k7-core's turntable");
    let text = std::fs::read_to_string(base.join(ASSEMBLY_FILE)).expect("the template");
    let mut members: Vec<String> = std::fs::read_dir(support::spell(&base.join("nodes")))
        .expect("nodes")
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter_map(|n| n.strip_suffix(".k7x").map(str::to_owned))
        .collect();
    members.sort();
    (text, members)
}

// turntableAssembly.test.ts "reads", "names only members in its plugs and its promoted params".
#[test]
fn the_shipped_turntable_reads_and_names_only_its_members() {
    let (text, members) = turntable();
    let t = decode_assembly_template("assembly.k7x", &text).expect("the shipped template reads");
    assert_eq!(t.label.as_ref().map(|l| l.0.as_str()), Some("Turntable"));
    assert_eq!(members.len(), 18);
    let named = t
        .inputs
        .iter()
        .flatten()
        .chain(t.outputs.iter().flatten())
        .map(|p| p.node.0.as_str());
    for n in named.chain(t.params.iter().flatten().map(|(_, p)| p.node.0.as_str())) {
        assert!(members.iter().any(|m| m == n), "{n}");
    }
}

/// `n` instances of the shipped turntable, each mapping all its members.
fn turntables(n: usize) -> Vec<(String, String)> {
    let (text, members) = turntable();
    let map = |k: usize| {
        members
            .iter()
            .map(|m| format!("{m}: \"{m}{k}\""))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut files = vec![(LIB.to_owned(), text)];
    files.extend((0..n).map(|k| {
        (
            format!("assemblies/t{k}.k7x"),
            format!("uses(\"@k7-core/turntable\")\nmembers({{ {} }})\n", map(k)),
        )
    }));
    files
}

fn resolved_owned(files: &[(String, String)]) -> song::assemblies::ResolvedAssemblies {
    let refs: Vec<(&str, &str)> = files.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    resolved(&refs)
}

// 276: the turntable costs 29 per instance (its label, 18 members, 2 plugs, 8 params), so 34,000 of them fit.
#[test]
fn the_turntable_costs_twenty_nine_an_instance_and_the_budget_stops_the_rest() {
    let files = turntables(1_000);
    let r = resolved_owned(&files);
    assert_eq!((r.assemblies.len(), r.weight, r.capped), (1_000, 29_000, false));
    // Printed: the most any call spent in the corpus is far below the budget.
    eprintln!("assemblies: 1,000 turntables weigh {} of {BUDGET}", r.weight);
    assert!(r.weight * 10 < BUDGET);
    // 29 an instance: 34,000 of them fit in the budget, and the huge-template case below shows the stop.
    const { assert!(29 * 34_000 <= BUDGET && 29 * 35_000 > BUDGET) };
}

// 276: a template of 100,000 plugs instanced 1,000 times is stopped at the budget, the fan-out and the bytes both.
#[test]
fn a_huge_template_instanced_often_is_stopped_at_the_budget() {
    let plugs: String = (0..100_000)
        .map(|n| format!("name(\"p{n}\").type(\"audio\").node(\"m\"),"))
        .collect();
    let template = format!("outputs([{plugs}])\n");
    let mut files = vec![(LIB.to_owned(), template)];
    files.extend((0..1_000).map(|k| {
        (
            format!("assemblies/i{k}.k7x"),
            "uses(\"@k7-core/turntable\")\nmembers({ m: \"n\" })\n".to_owned(),
        )
    }));
    let r = fast("100,000 plugs by 1,000", 20, || resolved_owned(&files));
    assert!(r.capped);
    assert_eq!(r.assemblies.len(), 9);
    let stopped = r
        .findings
        .last()
        .and_then(|f| f.file.strip_prefix("assemblies/"))
        .and_then(|f| f.strip_suffix(".k7x"));
    assert!(stopped.is_some_and(|id| !r.assemblies.contains_key(id)));
    // A plug name of a megabyte instanced 10,000 times is stopped by its bytes.
    let big = "x".repeat(1 << 20);
    let template = format!("outputs([name(\"{big}\").type(\"audio\").node(\"m\")])\n");
    let mut files = vec![(LIB.to_owned(), template)];
    files.extend((0..10_000).map(|k| {
        (
            format!("assemblies/i{k}.k7x"),
            "uses(\"@k7-core/turntable\")\nmembers({ m: \"n\" })\n".to_owned(),
        )
    }));
    let r = fast("1 MB plug names", 20, || resolved_owned(&files));
    assert!(r.capped && r.assemblies.len() < 100, "{}", r.assemblies.len());
}

// 276: a template's label is copied per instance, so its bytes are weighed like a plug's.
#[test]
fn a_label_of_a_megabyte_instanced_often_is_stopped_by_its_bytes() {
    let template = format!("label(\"{}\")\n", "x".repeat(1 << 20));
    let mut files = vec![(LIB.to_owned(), template)];
    files.extend((0..3_000).map(|k| {
        (
            format!("assemblies/i{k}.k7x"),
            "uses(\"@k7-core/turntable\")\nmembers({})\n".to_owned(),
        )
    }));
    let r = fast("1 MB labels", 20, || resolved_owned(&files));
    assert!(r.capped && r.assemblies.len() < 100, "{}", r.assemblies.len());
}

// A nest of 300 in either file is a syntax error; 100,000 members of one instance read and weigh.
#[test]
fn a_deep_nest_is_refused_and_a_hundred_thousand_members_are_weighed() {
    let nest = format!("members({})\n", "[".repeat(300));
    assert!(decode_assembly("a", &format!("uses(\"@k7-core/x\")\n{nest}")).is_err());
    assert!(decode_assembly_template("a", &format!("outputs({})\n", "[".repeat(300))).is_err());
    let members: String = (0..100_000).map(|n| format!("m{n}: \"n{n}\",")).collect();
    let files = vec![
        (LIB.to_owned(), TEMPLATE.to_owned()),
        (
            "assemblies/big.k7x".to_owned(),
            format!("uses(\"@k7-core/turntable\")\nmembers({{ {members} }})\n"),
        ),
    ];
    let r = fast("100,000 members", 10, || resolved_owned(&files));
    assert_eq!(r.assemblies["big"].members.len(), 100_000);
    assert!(r.weight >= 100_000 && !r.capped);
}
