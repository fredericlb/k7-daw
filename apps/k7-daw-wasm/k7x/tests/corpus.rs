// corpus.rs — every .k7x file of demos/ and k7-core/ edited by the op list of `ops_for`, compared by hash to what the TypeScript editor did.
// The vectors are frozen (their generator left with @studio/k7x): a corpus file edited since is skipped, the rest still replay (docs/rustEdit.md).

mod common;

use common::{Json, fnv, parse_json};
use k7x::edit::{Document, PathSeg, Tree, Value, WriteOptions, parse, stringify};
use std::path::{Path, PathBuf};

type Path_ = Vec<PathSeg>;

#[derive(Default)]
struct Shape {
    scalars: Vec<(Path_, Value)>,
    entries: Vec<Path_>,
    maps: Vec<Path_>,
    lists: Vec<(Path_, usize)>,
}

fn walk(n: &Tree, at: &Path_, s: &mut Shape) {
    match n {
        Tree::Scalar { value, .. } => s.scalars.push((at.clone(), value.clone())),
        Tree::List { items, .. } => {
            s.lists.push((at.clone(), items.len()));
            for (i, x) in items.iter().enumerate() {
                walk(x, &[at.as_slice(), &[PathSeg::Index(i)]].concat(), s);
            }
        }
        Tree::Map(m) => {
            s.maps.push(at.clone());
            for e in &m.entries {
                let p = [at.as_slice(), &[PathSeg::Key(e.key.clone())]].concat();
                s.entries.push(p.clone());
                walk(&e.value, &p, s);
            }
        }
    }
}

fn changed(v: &Value) -> Value {
    match v {
        Value::Num(n) => Value::Num(n + 1.0),
        Value::Str(s) => Value::Str(format!("{s}x")),
        Value::Bool(b) => Value::Bool(!b),
        _ => Value::Num(1.0),
    }
}

fn mutate(v: &Value, c: &mut u64) -> Value {
    match v {
        Value::List(items) => {
            let mut out: Vec<Value> = items.iter().map(|x| mutate(x, c)).collect();
            *c += 1;
            if out.len() >= 3 && (*c).is_multiple_of(3) {
                out.remove(1);
            } else if (*c).is_multiple_of(4) {
                out.push(Value::Str("m".into()));
            }
            Value::List(out)
        }
        Value::Map(m) => {
            let mut out = Vec::new();
            for (k, x) in m {
                *c += 1;
                if (*c).is_multiple_of(7) {
                    continue;
                }
                out.push((k.clone(), mutate(x, c)));
            }
            *c += 1;
            if (*c).is_multiple_of(11) {
                out.push(("k7xAdded".into(), Value::Num(*c as f64)));
            }
            Value::object(out)
        }
        scalar => {
            *c += 1;
            if (*c).is_multiple_of(5) {
                changed(scalar)
            } else {
                scalar.clone()
            }
        }
    }
}

enum Op {
    Set(Path_, Value, bool),
    Delete(Path_, bool),
    Insert(Path_, usize, Value),
    InsertSource(Path_, usize, &'static str),
    Add(Path_, Value),
    Move(Path_, usize, usize),
    Rename(Path_, String),
    SetSource(Path_, &'static str),
    Apply(Value),
    Header,
}

fn ops_for(root: &Tree, data: &Value, take: usize) -> Vec<Op> {
    let mut s = Shape::default();
    walk(root, &vec![], &mut s);
    let long = Value::Str("L".repeat(120));
    let mut ops = Vec::new();
    for (p, v) in s.scalars.iter().take(take) {
        ops.push(Op::Set(p.clone(), changed(v), false));
    }
    for p in s.entries.iter().take(take) {
        ops.push(Op::Delete(p.clone(), false));
    }
    for p in s.entries.iter().take(take / 2) {
        ops.push(Op::Delete(p.clone(), true));
    }
    let probe = Value::object(vec![(
        "a".into(),
        Value::List(vec![Value::Num(1.0), Value::Str("two".into())]),
    )]);
    for p in s.maps.iter().take(take) {
        ops.push(Op::Set(
            [p.as_slice(), &[PathSeg::Key("k7xProbe".into())]].concat(),
            probe.clone(),
            false,
        ));
    }
    for (p, len) in s.lists.iter().take(take / 2) {
        ops.push(Op::Insert(p.clone(), 0, Value::Str("ins".into())));
        ops.push(Op::Add(
            p.clone(),
            Value::object(vec![("x".into(), Value::Num(1.0))]),
        ));
        ops.push(Op::Insert(
            p.clone(),
            len / 2,
            Value::List(vec![Value::Num(1.0), Value::Num(2.0)]),
        ));
        ops.push(Op::InsertSource(p.clone(), *len, "7 /* c */"));
    }
    for (p, len) in s.lists.iter().filter(|l| l.1 >= 2).take(take / 2) {
        ops.push(Op::Move(p.clone(), 0, len - 1));
    }
    for p in s.entries.iter().take(take / 2) {
        let PathSeg::Key(k) = &p[p.len() - 1] else {
            continue;
        };
        ops.push(Op::Rename(p.clone(), format!("{k}_r")));
    }
    for (p, _) in s.scalars.iter().take(take / 2) {
        ops.push(Op::SetSource(p.clone(), "[1, 2]"));
    }
    for (p, _) in s.scalars.iter().take(take / 2) {
        ops.push(Op::Set(p.clone(), long.clone(), true));
    }
    ops.push(Op::Apply(mutate(data, &mut 0)));
    ops.push(Op::Header);
    ops
}

fn run(doc: &mut Document, op: &Op) -> Result<(), k7x::edit::EditError> {
    match op {
        Op::Set(p, v, inline) => doc.set_in(p, v, WriteOptions { inline: *inline }),
        Op::Delete(p, prune) => doc.delete_in(p, *prune).map(|_| ()),
        Op::Insert(p, i, v) => doc.insert_in(p, *i, v, WriteOptions::default()),
        Op::InsertSource(p, i, s) => doc.insert_source_in(p, *i, s),
        Op::Add(p, v) => doc.add_in(p, v, WriteOptions::default()),
        Op::Move(p, a, b) => doc.move_in(p, *a, *b),
        Op::Rename(p, k) => doc.rename_key(p, k),
        Op::SetSource(p, s) => doc.set_source_in(p, s),
        Op::Apply(v) => doc.apply_in(&[], v),
        Op::Header => doc.set_header(&["hello".to_owned(), "wörld 😀".to_owned()]),
    }
}

fn root_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .unwrap()
}

#[test]
fn every_corpus_file_edits_to_the_same_bytes() {
    let root = root_dir();
    let v = parse_json(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/vectors/corpus.json"
        ))
        .unwrap(),
    );
    let files = v.arr();
    assert!(files.len() > 1000, "{} files", files.len());
    let debug = std::env::var("K7X_VECTORS_DEBUG").ok();
    let mut replayed = 0;
    for f in files {
        let name = f.get("file").unwrap().str();
        let Ok(text) = std::fs::read_to_string(root.join(name)) else {
            continue;
        };
        if fnv(&text) != f.get("input").unwrap().str() {
            continue;
        }
        replayed += 1;
        let doc = Document::new(&text);
        let tree = doc.node_at(&[], |n| n.cloned()).unwrap().unwrap();
        let data = doc.to_js().unwrap();
        let mut lines: Vec<String> = ops_for(&tree, &data, 8)
            .iter()
            .map(|op| {
                let mut d = Document::new(&text);
                match run(&mut d, op) {
                    Ok(()) => format!("T:{}", fnv(d.text())),
                    Err(e) => format!("E:{e}"),
                }
            })
            .collect();
        lines.push(match stringify(&data, &Default::default()) {
            Ok(t) => format!("T:{}", fnv(&t)),
            Err(e) => format!("E:{e}"),
        });
        if debug.as_deref() == Some(name) {
            for (i, l) in lines.iter().enumerate() {
                eprintln!("{i} {l}");
            }
        }
        assert_eq!(lines.len(), f.get("ops").unwrap().num(), "{name}: op count");
        assert_eq!(
            fnv(&lines.join("\n")),
            f.get("hash").unwrap().str(),
            "{name}: hash (K7X_VECTORS_DEBUG={name} prints each op)"
        );
    }
    // Most of the corpus is still the one the vectors were frozen on.
    assert!(
        replayed * 10 > files.len() * 8,
        "{replayed} of {} files replayed",
        files.len()
    );
    let _: Option<Json> = None;
    let _ = parse("a(1)");
}
