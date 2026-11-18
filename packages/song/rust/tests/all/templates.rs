// templates.rs — the templates against the chains vectors the TypeScript wrote, and the hostile templates (1p-D2, 1p-D9, 1p-D28).
// A reader would get wrong that a vector's `error` row asks for a refusal, not for the TypeScript's sentence: the host declines.

use crate::support::{self, Tmp};
use serde_json::Value;
use song::session::Session;
use song::templates::{ChainAt, Layout, MAX_CHAIN_BYTES, Refused, Template, facts_of, read_template};
use song::{Library, PathRule, Roots, Source};
use std::path::Path;

const HOST: PathRule = PathRule::HOST;

fn repo() -> String {
    let at = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    support::spell(&std::fs::canonicalize(at).expect("the repository"))
}

fn vectors() -> Value {
    let at = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../apps/k7/backend-rs/crates/k7-file-service")
        .join("tests/vectors")
        .join("chains.json");
    serde_json::from_str(&std::fs::read_to_string(at).expect("chains.json")).expect("valid json")
}

/// The libraries a home lays out: the account's, then each under `libraries/` at the version the bench pins.
fn libraries(t: &Tmp) -> Vec<Library> {
    let mut out = vec![Library {
        name: "account".into(),
        root: t.at("studio/library"),
        editable: true,
    }];
    let pins = std::fs::read_to_string(t.0.join("studio/workspace/Banc/_library-pins.k7x")).unwrap_or_default();
    let mut names: Vec<String> = std::fs::read_dir(t.0.join("libraries"))
        .map(|d| d.flatten().filter_map(|e| e.file_name().into_string().ok()).collect())
        .unwrap_or_default();
    names.sort();
    for name in names {
        let versioned = std::fs::read_dir(t.0.join("libraries").join(&name))
            .map(|d| {
                d.flatten()
                    .any(|e| e.file_name().to_string_lossy().parse::<u32>().is_ok())
            })
            .unwrap_or(false);
        let pinned = pins
            .lines()
            .find_map(|l| l.strip_prefix(&format!("{name}("))?.strip_suffix(')'));
        let root = match (versioned, pinned) {
            (true, Some(v)) => format!("libraries/{name}/{v}"),
            _ => format!("libraries/{name}"),
        };
        out.push(Library {
            name,
            root: t.at(&root),
            editable: false,
        });
    }
    out
}

/// The roots the bench's views read: its project, the libraries, `@meta`, and the models root.
fn roots_of(t: &Tmp, repo: &str) -> Roots {
    Roots {
        project: t.at("studio/workspace/Banc/live.k7"),
        workspace: Some(t.at("studio/workspace/Banc")),
        confine: Some(t.at("studio/workspace")),
        libraries: libraries(t),
        meta: Some(format!("{repo}/packages/song/meta")),
        models: Some(t.at("models")),
        ..Roots::default()
    }
}

/// Numbers compared as JavaScript holds them: `1` and `1.0` are one value.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q)),
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w)))
        }
        _ => a == b,
    }
}

fn text_of(v: &Value) -> Option<&str> {
    v.as_str()
}

/// A home's vector with `<base>` and `<repo>` spelled for this machine.
fn spelled(home: &Value, base: &str, repo: &str) -> Value {
    let text = home.to_string().replace("<base>", base).replace("<repo>", repo);
    serde_json::from_str(&text).expect("json")
}

fn write_home(t: &Tmp, home: &Value) {
    for e in home["entries"].as_array().expect("entries") {
        let p = e["p"].as_str().expect("path");
        if p.ends_with('/') {
            t.mkdir(p);
        } else {
            t.write(p, e["t"].as_str().unwrap_or(""));
        }
    }
}

fn chain_at(d: &Value) -> (String, Layout, String) {
    let layout = if d["layout"] == "file" {
        Layout::File
    } else {
        Layout::Folder
    };
    (
        d["at"].as_str().expect("at").to_owned(),
        layout,
        d["libraryRoot"].as_str().expect("root").to_owned(),
    )
}

/// What a vector's step says of a template step, field for field.
fn check_step(want: &Value, step: &song::templates::TemplateStep, at: &str) {
    assert_eq!(want["name"], step.name.as_str(), "{at}: name");
    assert_eq!(want["label"], step.label.as_str(), "{at}: label");
    assert_eq!(want["bypass"].as_bool().unwrap_or(false), step.bypass, "{at}: bypass");
    assert_eq!(text_of(&want["preset"]), step.preset.as_deref(), "{at}: preset");
    let got = serde_json::to_value(&step.module).expect("module json");
    assert!(
        same(&want["module"], &got),
        "{at}: module {} against {got}",
        want["module"]
    );
}

fn check_facts(want: &Value, got: &song::templates::StepFacts, at: &str) {
    assert_eq!(want["offline"], got.offline, "{at}: offline");
    assert_eq!(want["coldFlag"], got.cold, "{at}: cold");
    let outputs = match &got.outputs {
        Ok(v) => serde_json::json!(v),
        Err(e) => serde_json::json!({ "error": e }),
    };
    assert_eq!(want["outputs"], outputs, "{at}: outputs");
    assert_eq!(
        text_of(&want["authored"]),
        got.onnx_authored.as_deref(),
        "{at}: authored"
    );
    assert_eq!(text_of(&want["onnx"]), got.onnx.as_deref(), "{at}: onnx");
    let missing = got.missing_model.as_ref().map(|(file, url)| {
        let mut m = serde_json::json!({ "file": file });
        if let Some(u) = url {
            m["url"] = Value::from(u.as_str());
        }
        m
    });
    assert_eq!(want["missing"], missing.unwrap_or(Value::Null), "{at}: missing");
}

#[test]
fn every_home_reads_as_the_typescript_did() {
    let (all, repo) = (vectors(), repo());
    let homes = all["homes"].as_array().expect("homes");
    let (mut templates, mut refused) = (0, 0);
    for (i, raw) in homes.iter().enumerate() {
        let t = Tmp::new("tpl");
        write_home(&t, raw);
        t.mkdir("studio/workspace/Banc/live.k7");
        let home = spelled(raw, &support::spell(&t.0), &repo);
        let roots = roots_of(&t, &repo);
        let source = support::disk(&roots);
        let mut session = Session::new(HOST);
        let vocab = session.vocabulary(&source, &roots).expect("a vocabulary");
        let answers = &home["answers"];
        // The vocabulary itself: a module kept or dropped with no chain naming it moves this row.
        let mut keys: Vec<&String> = vocab.modules.keys().collect();
        keys.sort();
        let want_keys: Vec<&str> = answers["vocabulary"]
            .as_array()
            .expect("vocabulary")
            .iter()
            .map(|v| v["ref"].as_str().expect("ref"))
            .collect();
        assert_eq!(keys, want_keys, "home {i}: the vocabulary's refs");
        for v in answers["vocabulary"].as_array().expect("vocabulary") {
            let got = vocab.modules[v["ref"].as_str().expect("ref")].folder_abs.as_deref();
            assert_eq!(text_of(&v["folderAbs"]), got, "home {i}: folder of {}", v["ref"]);
        }
        let dirs = answers["dirs"].as_array().expect("dirs");
        for (d, row) in dirs.iter().zip(answers["templates"].as_array().expect("templates")) {
            let (at, layout, library_root) = chain_at(d);
            let tag = format!("home {i}: {}", d["ref"]);
            let got = read_template(
                &source,
                &ChainAt {
                    at: &at,
                    layout,
                    library_root: &library_root,
                },
                HOST,
            );
            let want = &row["read"];
            if want.get("error").is_some() {
                assert!(matches!(got, Err(Refused::Native(_))), "{tag}: refused, got {got:?}");
                refused += 1;
                continue;
            }
            let got = got.unwrap_or_else(|e| panic!("{tag}: {e:?}"));
            templates += 1;
            let steps = want["steps"].as_array().expect("steps");
            assert_eq!(steps.len(), got.steps.len(), "{tag}: steps");
            assert_eq!(want["missing"], serde_json::json!(got.missing), "{tag}: missing");
            let input = got.input.as_ref().map(|c| {
                let mut m = serde_json::json!({ "channel": c.channel });
                if let Some(mon) = &c.monitor {
                    m["monitor"] = serde_json::to_value(mon).expect("monitor");
                }
                m
            });
            assert_eq!(
                want.get("input").cloned().unwrap_or(Value::Null),
                input.unwrap_or(Value::Null),
                "{tag}: input"
            );
            for (j, ((w, s), f)) in steps
                .iter()
                .zip(&got.steps)
                .zip(want["facts"].as_array().expect("facts"))
                .enumerate()
            {
                let at = format!("{tag} step {j}");
                check_step(w, s, &at);
                let facts = facts_of(&source, &roots, &vocab, s).unwrap_or_else(|e| panic!("{at}: {e:?}"));
                check_facts(f, &facts, &at);
            }
        }
    }
    assert!(templates > 60 && refused > 8, "{templates} read, {refused} refused");
}

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
