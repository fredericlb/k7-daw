//! onnx_model.rs — a tiny HT-Demucs stand-in as ONNX bytes, written by hand so no binary asset is kept: `mix`
//! `[1, 2, N]` in, `stems` `[1, 4, 2, N]` out, the stems being the mix, its negation, the mix and its negation
//! (drums, bass, other, vocals). It exercises the real windowing, the real session and the real fold.

use k7_supervisor::separation::audio::SEGMENT;

fn varint(mut v: u64, out: &mut Vec<u8>) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return;
        }
        out.push(b | 0x80);
    }
}

fn field(num: u64, bytes: &[u8], out: &mut Vec<u8>) {
    varint(num << 3 | 2, out);
    varint(bytes.len() as u64, out);
    out.extend_from_slice(bytes);
}

fn number(num: u64, v: u64, out: &mut Vec<u8>) {
    varint(num << 3, out);
    varint(v, out);
}

fn text(num: u64, s: &str, out: &mut Vec<u8>) {
    field(num, s.as_bytes(), out);
}

fn node(op: &str, name: &str, inputs: &[&str], output: &str, axis: Option<u64>) -> Vec<u8> {
    let mut n = Vec::new();
    inputs.iter().for_each(|i| text(1, i, &mut n));
    text(2, output, &mut n);
    text(3, name, &mut n);
    text(4, op, &mut n);
    if let Some(axis) = axis {
        let mut a = Vec::new();
        text(1, "axis", &mut a);
        number(3, axis, &mut a);
        number(20, 2, &mut a);
        field(5, &a, &mut n);
    }
    n
}

fn value_info(name: &str, dims: &[u64]) -> Vec<u8> {
    let mut shape = Vec::new();
    for d in dims {
        let mut dim = Vec::new();
        number(1, *d, &mut dim);
        field(1, &dim, &mut shape);
    }
    let mut tensor = Vec::new();
    number(1, 1, &mut tensor);
    field(2, &shape, &mut tensor);
    let mut ty = Vec::new();
    field(1, &tensor, &mut ty);
    let mut v = Vec::new();
    text(1, name, &mut v);
    field(2, &ty, &mut v);
    v
}

/// The model's bytes: Neg, Concat on axis 1, Reshape to the four stems.
pub fn model() -> Vec<u8> {
    let n = SEGMENT as u64;
    let mut shape = Vec::new();
    for d in [1i64, 4, 2, SEGMENT as i64] {
        shape.extend_from_slice(&d.to_le_bytes());
    }
    let mut init = Vec::new();
    number(1, 4, &mut init);
    number(2, 7, &mut init);
    text(8, "shape", &mut init);
    field(9, &shape, &mut init);
    let mut graph = Vec::new();
    field(
        1,
        &node("Neg", "neg", &["mix"], "negated", None),
        &mut graph,
    );
    let cat = node(
        "Concat",
        "cat",
        &["mix", "negated", "mix", "negated"],
        "cat",
        Some(1),
    );
    field(1, &cat, &mut graph);
    field(
        1,
        &node("Reshape", "reshape", &["cat", "shape"], "stems", None),
        &mut graph,
    );
    text(2, "stand-in", &mut graph);
    field(5, &init, &mut graph);
    field(11, &value_info("mix", &[1, 2, n]), &mut graph);
    field(12, &value_info("stems", &[1, 4, 2, n]), &mut graph);
    let mut opset = Vec::new();
    number(2, 13, &mut opset);
    let mut model = Vec::new();
    number(1, 8, &mut model);
    field(8, &opset, &mut model);
    field(7, &graph, &mut model);
    model
}
