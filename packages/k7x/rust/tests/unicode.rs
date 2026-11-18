// unicode.rs — the width rule on its own: the printer counts UTF-16 units, where Rust counts bytes (docs/wasm-step3a-brief.md).
// A reader would get wrong why these numbers: each text is 100 units wide and well over 100 bytes, so it must stay on one line.

use k7x::edit::{Document, PathSeg, Value, WriteOptions, js_number};

fn set(text: &str, key: &str, value: &str) -> String {
    let mut d = Document::new(text);
    d.set_in(
        &[PathSeg::Key(key.into())],
        &Value::Str(value.into()),
        WriteOptions::default(),
    )
    .unwrap();
    d.text().to_owned()
}

#[test]
fn a_line_is_measured_in_utf16_units() {
    // `a(` + two quotes + 96 units is exactly 100; one more unit breaks the line.
    for unit in ["a", "é", "e\u{301}", "日"] {
        let n = 96 / unit.encode_utf16().count();
        let s = unit.repeat(n);
        assert_eq!(set("a(1)\n", "a", &s), format!("a(\"{s}\")\n"), "{unit:?}");
    }
    let astral = "😀".repeat(48);
    assert_eq!(set("a(1)\n", "a", &astral), format!("a(\"{astral}\")\n"));
    let over = "😀".repeat(49);
    let m = {
        let mut d = Document::new("m({})\n");
        let v = Value::object(vec![("k".into(), Value::Str(over.clone()))]);
        d.set_in(&[PathSeg::Key("m".into())], &v, WriteOptions::default())
            .unwrap();
        d.text().to_owned()
    };
    assert_eq!(m, format!("m({{\n  k: \"{over}\",\n}})\n"));
}

#[test]
fn the_bom_is_one_column_and_one_header_offset() {
    let s = "x".repeat(95);
    assert_eq!(
        set("\u{feff}a(1)\n", "a", &s),
        format!("\u{feff}a(\"{s}\")\n")
    );
    let mut d = Document::new("\u{feff}// one\na(1)\n");
    assert_eq!(d.header(), vec!["one".to_owned()]);
    d.set_header(&["two".to_owned()]).unwrap();
    assert_eq!(d.text(), "\u{feff}// two\na(1)\n");
    assert_eq!((d.changes()[0].start16, d.changes()[0].end16), (1, 8));
}

#[test]
fn crlf_is_kept_and_a_comment_ends_before_its_return() {
    let mut d = Document::new("a(1) // c\r\nb(2)\r\n");
    d.set_in(
        &[PathSeg::Key("c".into())],
        &Value::Num(3.0),
        WriteOptions::default(),
    )
    .unwrap();
    assert_eq!(d.text(), "a(1) // c\r\nb(2)\r\nc(3)\n");
}

#[test]
fn numbers_print_as_javascript_prints_them() {
    for (n, s) in [
        (1e21, "1e+21"),
        (1e20, "100000000000000000000"),
        (1.5e-7, "1.5e-7"),
        (1e-6, "0.000001"),
        (0.1 + 0.2, "0.30000000000000004"),
        (-1.5, "-1.5"),
        (5e-324, "5e-324"),
        (f64::MAX, "1.7976931348623157e+308"),
        (123_456_789_012_345_680_000.0, "123456789012345680000"),
    ] {
        assert_eq!(js_number(n), s);
    }
}

#[test]
fn an_object_lists_its_index_keys_first() {
    let v = Value::object(vec![
        ("b".into(), Value::Null),
        ("2".into(), Value::Null),
        ("1".into(), Value::Null),
        ("a".into(), Value::Null),
    ]);
    let Value::Map(m) = v else { panic!() };
    assert_eq!(
        m.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
        ["1", "2", "b", "a"]
    );
}
