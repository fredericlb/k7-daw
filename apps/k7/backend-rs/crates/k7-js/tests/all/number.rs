//! k7-js's native number formats against node's: the PRNG of `fixtures/numberVectors.ts` replayed bit
//! for bit, each batch hashed as node hashed it. `K7_NUMBER_VECTORS=<file>` points the ignored test at
//! a larger set made by `makeNumberVectors.ts <batches> <file>` (apps/k7/backend-rs/docs/parity.md).

use k7_js::number::{math_round, text_hash, to_fixed, to_string};
use serde_json::Value;
use sha2::{Digest, Sha256};

struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
}

fn unit(r: u64) -> f64 {
    (r >> 11) as f64 / 9007199254740992.0
}

fn draw(g: &mut SplitMix) -> f64 {
    let kind = g.next() % 5;
    let neg = if g.next() & 1 == 1 { -1.0 } else { 1.0 };
    let r = g.next();
    match kind {
        0 => f64::from_bits(r),
        1 => neg * unit(r),
        2 => (neg * (r % 1048576) as f64) / 2f64.powi((1 + (r >> 40) % 12) as i32),
        3 => (neg * (r % 30000) as f64) / 100.0,
        _ => {
            neg * unit(r)
                * format!("1e{}", ((r >> 50) % 61) as i64 - 30)
                    .parse::<f64>()
                    .unwrap()
        }
    }
}

fn line(x: f64) -> String {
    let fixed: Vec<String> = [0, 2, 4, 6].iter().map(|d| to_fixed(x, *d)).collect();
    format!(
        "{:x} {} {} {}\n",
        x.to_bits(),
        fixed.join(" "),
        to_string(x),
        to_string(math_round(x))
    )
}

fn draw_text(g: &mut SplitMix) -> String {
    let len = (g.next() % 41) as usize;
    let mut units: Vec<u16> = Vec::new();
    while units.len() < len {
        let r = g.next();
        let v = ((r >> 8) % 0x10000) as u32;
        match r % 4 {
            0 => units.push((32 + v % 95) as u16),
            1 => {
                let c = char::from_u32(0x10000 + ((r >> 8) % 0x100000) as u32).unwrap();
                let mut buf = [0u16; 2];
                units.extend_from_slice(c.encode_utf16(&mut buf));
            }
            _ => units.push(if (0xd800..=0xdfff).contains(&v) {
                (v - 0x800) as u16
            } else {
                v as u16
            }),
        }
    }
    String::from_utf16(&units).unwrap()
}

fn check(file: &Value) {
    let batch = file["batch"].as_u64().unwrap();
    let mut g = SplitMix(0x6b37);
    for (b, want) in file["numbers"].as_array().unwrap().iter().enumerate() {
        let mut h = Sha256::new();
        for _ in 0..batch {
            h.update(line(draw(&mut g)));
        }
        assert_eq!(
            format!("{:x}", h.finalize()),
            want.as_str().unwrap(),
            "number batch {b}"
        );
    }
    let mut g = SplitMix(0x7e47);
    let mut h = Sha256::new();
    for _ in 0..batch {
        h.update(format!("{}\n", text_hash(&draw_text(&mut g))));
    }
    assert_eq!(
        format!("{:x}", h.finalize()),
        file["textHashes"].as_str().unwrap(),
        "textHash"
    );
}

fn committed() -> Value {
    serde_json::from_str(include_str!("../../../../fixtures/number-vectors.json")).unwrap()
}

#[test]
fn edge_cases_format_as_node_does() {
    for want in committed()["edges"].as_array().unwrap() {
        let want = want.as_str().unwrap();
        let bits = u64::from_str_radix(want.split(' ').next().unwrap(), 16).unwrap();
        assert_eq!(line(f64::from_bits(bits)), want);
    }
}

#[test]
fn a_million_doubles_and_texts_format_as_node_does() {
    check(&committed());
}

#[test]
#[ignore = "a local run: K7_NUMBER_VECTORS names a larger set"]
fn a_larger_set_formats_as_node_does() {
    let path = std::env::var("K7_NUMBER_VECTORS").expect("K7_NUMBER_VECTORS");
    check(&serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap());
}
