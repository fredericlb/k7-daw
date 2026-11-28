// read_print.rs — what the reader makes of each form and the printer's layout.
// A reader would get wrong that these are hand-written expectations: the frozen vectors (edit_vectors.rs) are the machine-made ones.

#[macro_use]
mod common;

use common::{data, lines, v};
use k7x::edit::{PathSeg, StringifyOptions, Value, parse, stringify};

#[test]
fn stringify_prints_plain_with_no_options_and_writes_the_header_first() {
    let opts = StringifyOptions::default();
    assert_eq!(
        stringify(&v(r#"{"a":1,"b":[1,2]}"#), &opts).unwrap(),
        "a(1)\nb([1, 2])\n"
    );
    let header = lines(&["synth pad", "", "v2"]);
    let with = StringifyOptions {
        header: &header,
        inline: None,
    };
    let text = stringify(&v(r#"{"a":1}"#), &with).unwrap();
    assert_eq!(text, "// synth pad\n//\n// v2\na(1)\n");
    assert_eq!(data(&text), v(r#"{"a":1}"#));
    let only = lines(&["only"]);
    let alone = StringifyOptions {
        header: &only,
        inline: None,
    };
    assert_eq!(stringify(&v("{}"), &alone).unwrap(), "// only\n");
}

fn wide_steps() -> Value {
    let steps: Vec<String> = (0..12)
        .map(|i| format!(r#"{{"t":{i},"d":1,"k":{}}}"#, 60 + i))
        .collect();
    v(&format!(
        r#"{{"steps":[{}],"params":{{"a":1}}}}"#,
        steps.join(",")
    ))
}

#[test]
fn stringify_keeps_the_nodes_inline_names_on_one_line_whatever_their_width() {
    let value = wide_steps();
    assert!(
        stringify(&value, &StringifyOptions::default())
            .unwrap()
            .split('\n')
            .count()
            > 5
    );
    let inline =
        |path: &[PathSeg], _: &Value| path.len() == 1 && path[0] == PathSeg::Key("steps".into());
    let opts = StringifyOptions {
        header: &[],
        inline: Some(&inline),
    };
    let text = stringify(&value, &opts).unwrap();
    assert!(text.split('\n').next().unwrap().len() > 100);
    assert!(text.starts_with("steps([t(0).d(1).k(60), "));
    assert_eq!(data(&text), value);
}

#[test]
fn stringify_hands_inline_each_nodes_path_and_value() {
    use std::cell::RefCell;
    let seen = RefCell::new(Vec::new());
    let inline = |path: &[PathSeg], value: &Value| {
        let p: Vec<String> = path
            .iter()
            .map(|s| match s {
                PathSeg::Key(k) => k.clone(),
                PathSeg::Index(i) => i.to_string(),
            })
            .collect();
        seen.borrow_mut().push((p.join("."), value.clone()));
        false
    };
    let opts = StringifyOptions {
        header: &[],
        inline: Some(&inline),
    };
    stringify(&v(r#"{"a":{"b":[1]}}"#), &opts).unwrap();
    let keys: Vec<String> = seen.borrow().iter().map(|(p, _)| p.clone()).collect();
    assert_eq!(keys, ["", "a", "a.b", "a.b.0"]);
    assert_eq!(seen.borrow()[0].1, v(r#"{"a":{"b":[1]}}"#));
    assert_eq!(seen.borrow()[3].1, Value::Num(1.0));
}

#[test]
fn numbers_keep_an_integer_past_32_bits_minus_zero_and_an_overflow() {
    let d = data("a(5000000000)\nb(-123456789012)\nc(-0)\nd(1e999)\ne(0.1)");
    assert_eq!(d.get("a"), Some(&Value::Num(5e9)));
    assert_eq!(d.get("b"), Some(&Value::Num(-123456789012.0)));
    assert!(matches!(d.get("c"), Some(Value::Num(n)) if *n == 0.0 && n.is_sign_negative()));
    assert_eq!(d.get("d"), Some(&Value::Num(f64::INFINITY)));
    assert_eq!(d.get("e"), Some(&Value::Num(0.1)));
}

#[test]
fn a_file_nested_past_the_limit_is_a_syntax_error_not_a_stack_overflow() {
    let deep = format!("a({}{})", "[".repeat(100_000), "]".repeat(100_000));
    let e = parse(&deep).unwrap_err().to_string();
    assert!(e.contains("nested deeper than 256"), "{e}");
    let objects = format!("a({}1{})", "{ b: ".repeat(300), " }".repeat(300));
    assert!(parse(&objects).is_err());
    assert!(
        parse(&format!("a({}{})", "[".repeat(200), "]".repeat(200)))
            .unwrap()
            .get("a")
            .is_some()
    );
}

#[test]
fn the_grammar_reads_statements_chains_objects_and_lists_as_one_map() {
    let text = "// a comment\nproject({ name: \"demo\", tempo: 120, video: false })\nsteps([\n  extends(\"@k7-core/audio/compressor\").engine('tracktion'),\n  to([chain(\"@out\")]), /* trailing comma */\n])\nmarkers({})";
    assert_eq!(
        data(text),
        v(
            r#"{"project":{"name":"demo","tempo":120,"video":false},"steps":[{"extends":"@k7-core/audio/compressor","engine":"tracktion"},{"to":[{"chain":"@out"}]}],"markers":{}}"#
        )
    );
    assert_eq!(
        data("a(1)\n  .b(2)\nc(3).d(4)"),
        v(r#"{"a":1,"b":2,"c":3,"d":4}"#)
    );
    assert_eq!(
        data("a() b(1, 2) c([1]) d(null)"),
        v(r#"{"a":true,"b":[1,2],"c":[1],"d":null}"#)
    );
    assert_eq!(
        data("\"Filter Freq\"(40) synth-pad([\"MIX\"]) x({ \"pad#midi\": 1, true: 2 })"),
        v(r##"{"Filter Freq":40,"synth-pad":["MIX"],"x":{"pad#midi":1,"true":2}}"##)
    );
    assert_eq!(data(""), v("{}"));
    assert_eq!(data("// only a comment\n"), v("{}"));
}

#[test]
fn strings_read_escapes_single_quotes_and_raw_multi_line_backticks() {
    let raw = "`line 1\n  \\` ${x} \\\\`";
    let read = "line 1\n  ` ${x} \\";
    let d = data(&format!("a(\"é\\n\\u0041\") b('it\\'s') c({raw})"));
    assert_eq!(d.get("a"), Some(&Value::Str("é\nA".into())));
    assert_eq!(d.get("b"), Some(&Value::Str("it's".into())));
    assert_eq!(d.get("c"), Some(&Value::Str(read.into())));
}

#[test]
fn a_mistake_is_rejected_with_its_sentence_and_reported_at_its_line_and_column() {
    for (text, message) in [
        ("a(1", "expected \",\" or \")\""),
        ("a(foo)", "bare word \"foo\""),
        ("a(1) a(2)", "duplicate key \"a\""),
        ("a({ b 1 })", "expected \":\" after b"),
        ("a(\"x\ny\")", "unterminated string"),
        ("a(01)", "expected \",\" or \")\""),
    ] {
        let e = parse(text).unwrap_err().to_string();
        assert!(e.contains(message), "{text:?}: {e}");
    }
    let e = parse("a(1)\nb(2,, 3)").unwrap_err();
    assert_eq!((e.line, e.column), (2, 5));
}

#[test]
fn stringify_reads_back_what_it_writes_in_the_canonical_layout() {
    let notes: Vec<String> = (0..3)
        .map(|i| format!(r#"{{"t":{i},"d":1,"k":{}}}"#, 60 + i))
        .collect();
    let value = v(&format!(
        r#"{{"project":{{"name":"synth-demo","tempo":120,"sampleRate":48000,"timeSignature":"4/4","video":false,"layers":[]}},"markers":{{}},"notes":[{}],"params":{{"Filter Freq":40,"Level":-12.5}},"content":"two\nlines with a ` and a \\","steps":[{{"extends":"@k7-core/audio/compressor","engine":"tracktion","engines":{{"tracktion":{{"params":{{"Ratio":0.125,"Attack":12,"Release":140,"Knee":3,"Makeup":0}}}}}}}},{{"to":[{{"chain":"@out"}}]}}]}}"#,
        notes.join(",")
    ));
    let text = stringify(&value, &StringifyOptions::default()).unwrap();
    assert_eq!(data(&text), value);
    let want = "project({\n  name: \"synth-demo\",\n  tempo: 120,\n  sampleRate: 48000,\n  timeSignature: \"4/4\",\n  video: false,\n  layers: [],\n})\nmarkers({})\nnotes([t(0).d(1).k(60), t(1).d(1).k(61), t(2).d(1).k(62)])\nparams({ \"Filter Freq\": 40, Level: -12.5 })\ncontent(`two\nlines with a \\` and a \\\\`)\nsteps([\n  extends(\"@k7-core/audio/compressor\")\n    .engine(\"tracktion\")\n    .engines({\n      tracktion: { params: { Ratio: 0.125, Attack: 12, Release: 140, Knee: 3, Makeup: 0 } },\n    }),\n  to([chain(\"@out\")]),\n])\n";
    assert_eq!(text, want);
}
