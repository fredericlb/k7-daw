// templates.rs — the templates against the chains vectors the TypeScript wrote, and the hostile templates (1p-D2, 1p-D9, 1p-D28).
// A reader would get wrong that a vector's `error` row asks for a refusal, not for the TypeScript's sentence: the host declines.

use crate::support;
use song::session::Session;
use song::templates::{ChainAt, Layout, MAX_CHAIN_BYTES, Refused, Template, facts_of, read_template};
use song::{Library, PathRule, Roots, Source};

const HOST: PathRule = PathRule::HOST;

/// A MemSource world under `/k7`: the bench's project, one library `k7-core`, `models`, and `files` laid in.
fn world(root: &str, rule: PathRule, files: &[(&str, &str)]) -> (song::MemSource, Roots) {
    let src = support::mem_at(root, files);
    let roots = Roots {
        project: format!("{root}/ws/Banc/live.k7"),
        workspace: Some(format!("{root}/ws/Banc")),
        confine: Some(format!("{root}/ws")),
        libraries: vec![Library {
            name: "k7-core".into(),
            root: format!("{root}/lib/k7-core"),
            editable: false,
        }],
        models: Some(format!("{root}/models")),
        paths: rule,
        ..Roots::default()
    };
    (src, roots)
}

const VOX: &str = "plugin({ type: \"offline\", name: \"vox\" })\nparams({ onnx: \"vox.onnx\" })\n";

fn file_template(src: &impl Source, at: &str, root: &str) -> Result<Template, Refused> {
    read_template(
        src,
        &ChainAt {
            at,
            layout: Layout::File,
            library_root: root,
        },
        HOST,
    )
}

#[test]
fn a_chain_with_params_of_another_type_is_refused() {
    // 1p-D11 does not hold: a list, a map, null and a bool refuse the chain whole.
    for bad in ["[1, 2]", "{ x: true }", "null", "true"] {
        let text = format!(
            "steps([id(\"a\").plugin({{ type: \"internal\", name: \"Comp\" }}).params({{ A: {bad} }}), to([chain(\"@out\")])])\n"
        );
        let (src, _) = world("/k7", HOST, &[("lib/k7-core/chains/c.k7x", &text)]);
        let got = file_template(&src, "/k7/lib/k7-core/chains/c.k7x", "/k7/lib/k7-core");
        assert!(
            matches!(&got, Err(Refused::Native(m)) if m.contains("a number or a string")),
            "{bad}: {got:?}"
        );
    }
    let ok = "steps([id(\"a\").plugin({ type: \"internal\", name: \"Comp\" }).params({ A: 1, B: \"x\" }), to([chain(\"@out\")])])\n";
    let (src, _) = world("/k7", HOST, &[("lib/k7-core/chains/c.k7x", ok)]);
    let got = file_template(&src, "/k7/lib/k7-core/chains/c.k7x", "/k7/lib/k7-core").expect("numbers and strings read");
    assert_eq!(got.steps.len(), 1);
}

#[test]
fn names_that_differ_only_in_normalization_are_two_chains_and_two_modules() {
    // The NFD and NFC spellings of one letter are two names to the Source, to a chain and to a module ref.
    let (nfd, nfc) = ("e\u{301}", "\u{e9}");
    let chain =
        |module: &str| format!("steps([id(\"s\").extends(\"@k7-core/cold/{module}\"), to([chain(\"@out\")])])\n");
    let mut files: Vec<(String, String)> = Vec::new();
    for name in [nfd, nfc] {
        files.push((format!("lib/k7-core/chains/{name}.k7x"), chain(name)));
    }
    files.push((format!("lib/k7-core/modules/cold/{nfd}/module.k7x"), VOX.to_owned()));
    files.push((
        format!("lib/k7-core/modules/cold/{nfc}/module.k7x"),
        "plugin({ type: \"internal\", name: \"Plain\" })\n".to_owned(),
    ));
    let refs: Vec<(&str, &str)> = files.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let (src, roots) = world("/k7", HOST, &refs);
    let vocab = Session::new(HOST).vocabulary(&src, &roots).expect("a vocabulary");
    assert!(vocab.modules.contains_key(&format!("@k7-core/cold/{nfd}")));
    assert!(vocab.modules.contains_key(&format!("@k7-core/cold/{nfc}")));
    let offline = |name: &str| {
        let t = file_template(&src, &format!("/k7/lib/k7-core/chains/{name}.k7x"), "/k7/lib/k7-core").expect("read");
        assert_eq!(t.steps[0].label, format!("@k7-core/cold/{name}"));
        facts_of(&src, &roots, &vocab, &t.steps[0]).expect("facts").offline
    };
    assert!(offline(nfd) && !offline(nfc));
}

#[test]
fn a_c_drive_home_resolves_its_files_lexically_and_declines_nothing() {
    // 1p-D28: every path here is `C:/`, held in memory; nothing is written, so no folder can land in the working directory.
    let chain = "steps([id(\"s\").extends(\"@k7-core/cold/vox\"), to([chain(\"@out\")])])\n";
    let files = [
        ("lib/k7-core/chains/c.k7x", chain),
        ("lib/k7-core/modules/cold/vox/module.k7x", VOX),
        ("lib/k7-core/modules/cold/vox/assets/vox.onnx", "model"),
    ];
    let (src, roots) = world("C:/k7", PathRule::Windows, &files);
    let vocab = Session::new(PathRule::Windows)
        .vocabulary(&src, &roots)
        .expect("a vocabulary");
    let at = ChainAt {
        at: "C:/k7/lib/k7-core/chains/c.k7x",
        layout: Layout::File,
        library_root: "C:/k7/lib/k7-core",
    };
    let t = read_template(&src, &at, PathRule::Windows).expect("read");
    let facts = facts_of(&src, &roots, &vocab, &t.steps[0]).expect("facts");
    assert_eq!(
        facts.onnx.as_deref(),
        Some("C:/k7/lib/k7-core/modules/cold/vox/assets/vox.onnx")
    );
    assert_eq!(facts.onnx_authored.as_deref(), Some("vox.onnx"));
    assert!(facts.offline && facts.missing_model.is_none());
}

#[test]
fn a_chain_past_the_size_bound_is_a_limit_and_not_read() {
    let big = format!("steps([])\n{}", " ".repeat(MAX_CHAIN_BYTES as usize + 1));
    let (src, _) = world("/k7", HOST, &[("lib/k7-core/chains/big.k7x", &big)]);
    assert_eq!(
        file_template(&src, "/k7/lib/k7-core/chains/big.k7x", "/k7/lib/k7-core").err(),
        Some(Refused::Limit)
    );
}

#[test]
fn ten_thousand_steps_read_and_nesting_past_the_parser_refuses() {
    let mut text = String::from("steps([\n");
    for i in 0..10_000 {
        text.push_str(&format!("  id(\"s{i}\").extends(\"@k7-core/audio/comp\"),\n"));
    }
    text.push_str("  to([chain(\"@out\")]),\n])\n");
    let nested = format!("steps({}{})\n", "[".repeat(300), "]".repeat(300));
    let (src, _) = world(
        "/k7",
        HOST,
        &[
            ("lib/k7-core/chains/many.k7x", &text),
            ("lib/k7-core/chains/deep.k7x", &nested),
        ],
    );
    let many = file_template(&src, "/k7/lib/k7-core/chains/many.k7x", "/k7/lib/k7-core").expect("read");
    assert_eq!(many.steps.len(), 10_000);
    let deep = file_template(&src, "/k7/lib/k7-core/chains/deep.k7x", "/k7/lib/k7-core");
    assert!(matches!(deep, Err(Refused::Native(_))), "{deep:?}");
}

#[test]
fn modules_that_extend_themselves_or_each_other_resolve_without_looping() {
    let files = [
        (
            "lib/k7-core/modules/audio/la/module.k7x",
            "extends(\"@k7-core/audio/la\")\n",
        ),
        (
            "lib/k7-core/modules/audio/lb/module.k7x",
            "extends(\"@k7-core/audio/lc\")\n",
        ),
        (
            "lib/k7-core/modules/audio/lc/module.k7x",
            "extends(\"@k7-core/audio/lb\")\n",
        ),
    ];
    let (src, roots) = world("/k7", HOST, &files);
    let vocab = Session::new(HOST).vocabulary(&src, &roots).expect("a vocabulary");
    // The modules are in the vocabulary: a folder of no known type would leave the cycle unread, and the test empty.
    assert!(
        ["la", "lb", "lc"]
            .iter()
            .all(|m| vocab.modules.contains_key(&format!("@k7-core/audio/{m}")))
    );
    for module in ["a", "b"] {
        let step = song::templates::TemplateStep {
            name: "s".into(),
            label: module.into(),
            module: song::modules::decode_module("m", &format!("extends(\"@k7-core/audio/l{module}\")\n"))
                .expect("module"),
            bypass: false,
            preset: None,
        };
        let facts = facts_of(&src, &roots, &vocab, &step).expect("a cycle only warns");
        assert!(!facts.offline, "{module}");
    }
}

#[cfg(unix)]
#[test]
fn links_and_folders_where_files_belong_are_refused_never_followed() {
    use song::DiskSource;
    use std::os::unix::fs::symlink;
    let t = Tmp::new("tpl-link");
    let out = Tmp::new("tpl-out");
    out.write("node.k7x", "extends(\"@k7-core/audio/comp\")\n");
    out.write("p.vstpreset", "state");
    t.write(
        "lib/chains/ln/graph.k7x",
        "modules({ \"@in\": [\"amp\"], amp: [\"@out\"] })\n",
    );
    t.mkdir("lib/chains/ln/nodes");
    symlink(out.at("node.k7x"), t.0.join("lib/chains/ln/nodes/amp.k7x")).expect("link");
    t.mkdir("lib/chains/fd/graph.k7x");
    t.write(
        "lib/chains/pl.k7x",
        "steps([id(\"a\").extends(\"@k7-core/audio/comp\").vstpreset(\"p.vstpreset\"), to([chain(\"@out\")])])\n",
    );
    t.mkdir("lib/vstpresets");
    symlink(out.at("p.vstpreset"), t.0.join("lib/vstpresets/p.vstpreset")).expect("link");
    let source = DiskSource::new(&[t.at("lib")]).expect("a source");
    let (root, rel) = (t.at("lib"), |p: &str| t.at(&format!("lib/chains/{p}")));
    let read = |at: &str, layout| {
        read_template(
            &source,
            &ChainAt {
                at,
                layout,
                library_root: &root,
            },
            HOST,
        )
    };
    for (at, layout, what) in [
        (rel("ln"), Layout::Folder, "a node that is a link out"),
        (rel("fd"), Layout::Folder, "a graph that is a folder"),
        (rel("pl.k7x"), Layout::File, "a preset that is a link out"),
    ] {
        assert!(matches!(read(&at, layout), Err(Refused::Native(_))), "{what}");
    }
}

#[test]
fn a_fold_past_the_resolvers_depth_is_a_limit_and_never_a_partial_answer() {
    let mut files: Vec<(String, String)> = vec![(
        "lib/k7-core/modules/audio/m0/module.k7x".into(),
        "plugin({ type: \"internal\", name: \"Deep\" })\n".into(),
    )];
    for n in 1..100 {
        files.push((
            format!("lib/k7-core/modules/audio/m{n}/module.k7x"),
            format!("extends(\"@k7-core/audio/m{}\")\n", n - 1),
        ));
    }
    let refs: Vec<(&str, &str)> = files.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let (src, roots) = world("/k7", HOST, &refs);
    let vocab = Session::new(HOST).vocabulary(&src, &roots).expect("a vocabulary");
    let step = song::templates::TemplateStep {
        name: "s".into(),
        label: "m99".into(),
        module: song::modules::decode_module("m", "extends(\"@k7-core/audio/m99\")\n").expect("module"),
        bypass: false,
        preset: None,
    };
    assert_eq!(facts_of(&src, &roots, &vocab, &step).err(), Some(Refused::Limit));
}
