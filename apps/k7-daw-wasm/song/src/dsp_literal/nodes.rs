// dsp_literal/nodes.rs — the vocabulary: each builder call of @studio/dsp, the inputs it takes and the JSON node it records.
// A reader would get wrong the key order: it differs by family (`out` before `in` for amul, after `k` for lfo), copied from index.ts.

use super::json::J;
use super::{Ctx, Made, R, Rate, Val, fail, finite};
use k7x::{Entry, ListForm, Node, Scalar};

/// Every node word a `dsp.k7x` may call: the builder's method names, which the engine's types follow.
pub(crate) const WORDS: [&str; 32] = [
    "envelope",
    "todb",
    "fromdb",
    "lerp",
    "linmap",
    "geolerp",
    "clamp",
    "select",
    "add",
    "sub",
    "mul",
    "glide",
    "biquad",
    "gain",
    "mix",
    "input",
    "lfo",
    "allpass",
    "delay",
    "amul",
    "aadd",
    "saturate",
    "output",
    "hold",
    "strike",
    "pattern",
    "accent",
    "chance",
    "decay",
    "transpose",
    "velocity",
    "channel",
];

/// The nodes that may write a declared wire with `.into(…)`.
pub(crate) const WRITES_WIRE: [&str; 2] = ["allpass", "delay"];

const PASS_MODES: [&str; 4] = ["bandpass", "lowpass", "highpass", "notch"];
const GAIN_MODES: [&str; 3] = ["peak", "lowshelf", "highshelf"];
const SHAPES: [&str; 4] = ["sine", "triangle", "saw", "square"];

pub(crate) fn vocabulary() -> String {
    format!("the node types are {}", WORDS.join(", "))
}

/// A call's arguments: `f()` none, `f(x)` one, `f(a, b)` several.
fn args<'a>(e: &'a Entry<'a>) -> &'a [Node<'a>] {
    match &e.value {
        Node::Scalar {
            value: Scalar::Bool(true),
            start,
            end,
        } if start == end => &[],
        Node::List {
            items,
            form: ListForm::Args,
            ..
        } => items,
        one => std::slice::from_ref(one),
    }
}

fn arity(a: &[Node], counts: &[usize], word: &Entry, path: &str) -> R<()> {
    if counts.contains(&a.len()) {
        return Ok(());
    }
    let want: Vec<String> = counts.iter().map(usize::to_string).collect();
    fail(
        word.key_start,
        path,
        format!("`{}` takes {} inputs, not {}", word.key, want.join(" or "), a.len()),
    )
}

fn at(path: &str, i: usize) -> String {
    format!("{path}[{i}]")
}

/// A once-per-block input: a number, a param, `sr` or a control node.
fn scalar(cx: &mut Ctx, n: &Node, path: &str) -> R<J> {
    match cx.input(n, path)? {
        Val::Num(v) => Ok(J::Num(v)),
        Val::Ref(id, Rate::Scalar) => Ok(J::Str(id)),
        Val::Ref(..) => fail(
            n.start(),
            path,
            "expected a scalar (a number, a param, a control node), not an audio wire",
        ),
    }
}

/// A per-sample input: an `input`, an `lfo`, a wire or a node made of them.
fn audio(cx: &mut Ctx, n: &Node, path: &str) -> R<String> {
    match cx.input(n, path)? {
        Val::Ref(id, Rate::Audio) => Ok(id),
        _ => fail(
            n.start(),
            path,
            "expected an audio wire (input, lfo, a wire, or a node made of them)",
        ),
    }
}

/// A word from a closed list, written as a string literal.
fn choice<'a>(n: &Node<'a>, allowed: &[&str], path: &str) -> R<&'a str> {
    match *n {
        Node::Scalar {
            value: Scalar::Str(s), ..
        } if allowed.contains(&s) => Ok(s),
        _ => fail(n.start(), path, format!("expected one of {}", allowed.join(", "))),
    }
}

fn str_list(v: Vec<J>) -> (&'static str, J) {
    ("in", J::Arr(v))
}

/// A control node: its scalar inputs in order, then `{ type, in, out }`.
fn control(cx: &mut Ctx, kind: &str, a: &[Node], path: &str) -> R<Made> {
    let ins = a
        .iter()
        .enumerate()
        .map(|(i, n)| scalar(cx, n, &at(path, i)))
        .collect::<R<Vec<_>>>()?;
    let out = cx.fresh();
    cx.nodes.push(J::Obj(vec![
        ("type", J::Str(kind.to_owned())),
        str_list(ins),
        ("out", J::Str(out.clone())),
    ]));
    Ok(Made::Value(out, Rate::Scalar))
}

/// A node acting on the buffer or the notes: `{ type, in }`, no value.
fn sink(cx: &mut Ctx, kind: &str, a: &[Node], path: &str) -> R<Made> {
    let ins = a
        .iter()
        .enumerate()
        .map(|(i, n)| scalar(cx, n, &at(path, i)))
        .collect::<R<Vec<_>>>()?;
    cx.nodes
        .push(J::Obj(vec![("type", J::Str(kind.to_owned())), str_list(ins)]));
    Ok(Made::Nothing)
}

fn wire(cx: &mut Ctx, into: Option<&str>) -> String {
    into.map_or_else(|| cx.fresh(), str::to_owned)
}

/// `amul` and `aadd`: a second wire goes in `in`, a scalar in `k`.
fn arith(cx: &mut Ctx, kind: &str, a: &[Node], path: &str) -> R<Made> {
    let x = audio(cx, &a[0], &at(path, 0))?;
    let (ins, ks) = match cx.input(&a[1], &at(path, 1))? {
        Val::Ref(id, Rate::Audio) => (vec![J::Str(x), J::Str(id)], vec![]),
        Val::Ref(id, Rate::Scalar) => (vec![J::Str(x)], vec![J::Str(id)]),
        Val::Num(v) => (vec![J::Str(x)], vec![J::Num(v)]),
    };
    let out = cx.fresh();
    let mut fields = vec![
        ("type", J::Str(kind.to_owned())),
        ("out", J::Str(out.clone())),
        ("in", J::Arr(ins)),
    ];
    if !ks.is_empty() {
        fields.push(("k", J::Arr(ks)));
    }
    cx.nodes.push(J::Obj(fields));
    Ok(Made::Value(out, Rate::Audio))
}

pub(crate) fn emit(cx: &mut Ctx, word: &Entry, path: &str, into: Option<&str>) -> R<Made> {
    let a = args(word);
    match word.key {
        "todb" | "fromdb" | "glide" => arity(a, &[1], word, path).and_then(|()| control(cx, word.key, a, path)),
        "envelope" | "add" | "sub" | "mul" => arity(a, &[2], word, path).and_then(|()| control(cx, word.key, a, path)),
        "lerp" | "linmap" | "clamp" | "select" => {
            arity(a, &[3], word, path).and_then(|()| control(cx, word.key, a, path))
        }
        "geolerp" => {
            arity(a, &[3], word, path)?;
            if let (Ok(lo), Ok(hi)) = (finite(&a[0], path), finite(&a[1], path))
                && lo > hi
            {
                return fail(
                    a[0].start(),
                    path,
                    format!(
                        "geolerp({lo}, {hi}) is descending and the engine sorts its ends: invert t instead, geolerp({hi}, {lo}, sub(1, t))"
                    ),
                );
            }
            control(cx, "geolerp", a, path)
        }
        "gain" | "mix" => arity(a, &[1], word, path).and_then(|()| sink(cx, word.key, a, path)),
        "biquad" => biquad(cx, word, a, path),
        "input" => input(cx, word, a, path),
        "lfo" => lfo(cx, word, a, path),
        "allpass" => allpass(cx, word, a, path, into),
        "delay" => delay(cx, word, a, path, into),
        "amul" | "aadd" => arity(a, &[2], word, path).and_then(|()| arith(cx, word.key, a, path)),
        "saturate" => saturate(cx, word, a, path),
        "output" => {
            arity(a, &[1], word, path)?;
            let x = audio(cx, &a[0], &at(path, 0))?;
            cx.nodes
                .push(J::Obj(vec![("type", J::Str("aout".into())), str_list(vec![J::Str(x)])]));
            Ok(Made::Nothing)
        }
        _ => midi(cx, word, a, path),
    }
}

fn biquad(cx: &mut Ctx, word: &Entry, a: &[Node], path: &str) -> R<Made> {
    arity(a, &[3, 4], word, path)?;
    let modes: Vec<&str> = PASS_MODES.iter().chain(GAIN_MODES.iter()).copied().collect();
    let mode = choice(&a[0], &modes, &at(path, 0))?;
    let want = if PASS_MODES.contains(&mode) { 3 } else { 4 };
    if a.len() != want {
        return fail(
            word.key_start,
            path,
            format!("a {mode} biquad takes {want} inputs, not {}", a.len()),
        );
    }
    let ins = a[1..]
        .iter()
        .enumerate()
        .map(|(i, n)| scalar(cx, n, &at(path, i + 1)))
        .collect::<R<Vec<_>>>()?;
    cx.nodes.push(J::Obj(vec![
        ("type", J::Str("biquad".into())),
        str_list(ins),
        ("mode", J::Str(mode.to_owned())),
    ]));
    Ok(Made::Nothing)
}

fn input(cx: &mut Ctx, word: &Entry, a: &[Node], path: &str) -> R<Made> {
    arity(a, &[0, 1], word, path)?;
    let port = match a.first() {
        None => 0.0,
        Some(n) => finite(n, &at(path, 0))?,
    };
    if port.fract() != 0.0 || !(0.0..=64.0).contains(&port) {
        return fail(word.key_start, path, "input takes a port index, a whole number from 0");
    }
    let out = cx.fresh();
    let mut fields = vec![("type", J::Str("ain".into()))];
    if port > 0.0 {
        fields.push(("port", J::Num(port)));
    }
    fields.push(("out", J::Str(out.clone())));
    cx.nodes.push(J::Obj(fields));
    Ok(Made::Value(out, Rate::Audio))
}

fn lfo(cx: &mut Ctx, word: &Entry, a: &[Node], path: &str) -> R<Made> {
    arity(a, &[2, 3], word, path)?;
    let shape = choice(&a[0], &SHAPES, &at(path, 0))?;
    let ks = a[1..]
        .iter()
        .enumerate()
        .map(|(i, n)| scalar(cx, n, &at(path, i + 1)))
        .collect::<R<Vec<_>>>()?;
    let out = cx.fresh();
    cx.nodes.push(J::Obj(vec![
        ("type", J::Str("alfo".into())),
        ("k", J::Arr(ks)),
        ("out", J::Str(out.clone())),
        ("shape", J::Str(shape.to_owned())),
    ]));
    Ok(Made::Value(out, Rate::Audio))
}

fn allpass(cx: &mut Ctx, word: &Entry, a: &[Node], path: &str, into: Option<&str>) -> R<Made> {
    arity(a, &[2, 3], word, path)?;
    let x = audio(cx, &a[0], &at(path, 0))?;
    let base = scalar(cx, &a[1], &at(path, 1))?;
    let mut ins = vec![J::Str(x)];
    if let Some(o) = a.get(2) {
        ins.push(J::Str(audio(cx, o, &at(path, 2))?));
    }
    let out = wire(cx, into);
    cx.nodes.push(J::Obj(vec![
        ("type", J::Str("aallpass".into())),
        str_list(ins),
        ("k", J::Arr(vec![base])),
        ("out", J::Str(out.clone())),
    ]));
    Ok(Made::Value(out, Rate::Audio))
}

fn delay(cx: &mut Ctx, word: &Entry, a: &[Node], path: &str, into: Option<&str>) -> R<Made> {
    arity(a, &[3], word, path)?;
    let x = audio(cx, &a[0], &at(path, 0))?;
    let time = cx.input(&a[1], &at(path, 1))?;
    let max_ms = finite(&a[2], &at(path, 2))?;
    let (ins, k) = match time {
        Val::Ref(id, Rate::Audio) => (vec![J::Str(x), J::Str(id)], None),
        Val::Ref(id, Rate::Scalar) => (vec![J::Str(x)], Some(J::Str(id))),
        Val::Num(v) => (vec![J::Str(x)], Some(J::Num(v))),
    };
    let out = wire(cx, into);
    let mut fields = vec![("type", J::Str("adelay".into())), str_list(ins)];
    if let Some(k) = k {
        fields.push(("k", J::Arr(vec![k])));
    }
    fields.push(("out", J::Str(out.clone())));
    fields.push(("maxMs", J::Num(max_ms)));
    cx.nodes.push(J::Obj(fields));
    Ok(Made::Value(out, Rate::Audio))
}

fn saturate(cx: &mut Ctx, word: &Entry, a: &[Node], path: &str) -> R<Made> {
    arity(a, &[1, 2], word, path)?;
    let x = audio(cx, &a[0], &at(path, 0))?;
    let drive = match a.get(1) {
        Some(d) => scalar(cx, d, &at(path, 1))?,
        None => J::Num(1.0),
    };
    let out = cx.fresh();
    cx.nodes.push(J::Obj(vec![
        ("type", J::Str("atanh".into())),
        ("out", J::Str(out.clone())),
        str_list(vec![J::Str(x)]),
        ("k", J::Arr(vec![drive])),
    ]));
    Ok(Made::Value(out, Rate::Audio))
}

/// The note-stream nodes: they act on the one stream in place, so they take scalars and leave nothing.
fn midi(cx: &mut Ctx, word: &Entry, a: &[Node], path: &str) -> R<Made> {
    let (kind, n) = match word.key {
        "transpose" => ("transpose", 1),
        "velocity" => ("velocity", 1),
        "channel" => ("mchannel", 1),
        "chance" => ("mchance", 1),
        "decay" => ("mdecay", 1),
        "strike" => ("mstrike", 2),
        "pattern" => ("mpattern", 4),
        "accent" => ("maccent", 4),
        "hold" => ("mhold", 0),
        _ => return fail(word.key_start, path, format!("unknown node type `{}`", word.key)),
    };
    arity(a, &[n], word, path)?;
    if n == 0 {
        cx.nodes.push(J::Obj(vec![("type", J::Str(kind.into()))]));
        return Ok(Made::Nothing);
    }
    sink(cx, kind, a, path)
}
