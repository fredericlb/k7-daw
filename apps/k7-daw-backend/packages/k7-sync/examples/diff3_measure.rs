//! What a whole-text merge costs, for docs/parity.md: the native port on 2,000, 5,000 and 20,000 lines.
//! `cargo run --release -p k7-sync --example diff3_measure` from apps/k7-daw-backend.

use std::time::{Duration, Instant};

use k7_sync::bases::TextBases;
use k7_sync::settle::{self, Native};
use serde_json::{Value, json};

fn ms(d: Duration) -> String {
    format!("{:.1} ms", d.as_secs_f64() * 1000.0)
}

/// The fastest of `n` runs of `f`.
fn best(n: usize, mut f: impl FnMut()) -> Duration {
    (0..n)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed()
        })
        .min()
        .unwrap()
}

/// `n` lines, one in `blank` of them empty (0 for none): a list, or prose with paragraphs.
fn text(n: usize, blank: usize) -> String {
    (0..n)
        .map(|i| {
            if blank > 0 && i % blank == 0 {
                "\n".to_string()
            } else {
                format!("- line {i}\n")
            }
        })
        .collect()
}

fn main() {
    let js = Native;
    for (shape, blank) in [("distinct lines", 0), ("a blank line in 8", 8)] {
        for n in [2_000, 5_000, 20_000] {
            let base = text(n, blank);
            let mine = base.replace("- line 11\n", "- line eleven\n");
            let theirs = base.replace(&format!("- line {}\n", n - 11), "- changed\n");
            let op: Value = json!({ "kind": "text", "file": "TODOS.md", "text": theirs, "was": k7_js::number::text_hash(&base) });
            let bases = TextBases::from_saved(Some(&json!({ "TODOS.md": [base] })));
            let native = best(5, || {
                let s = settle::settle(&js, Some(&mine), &op, &bases).unwrap();
                assert!(s.aside.is_empty());
            });
            println!(
                "{shape}, {n} lines ({} KB): native {}",
                base.len() / 1024,
                ms(native)
            );
        }
    }
    let same = "x\n".repeat(200_000);
    let t = Instant::now();
    let r = k7_sync::diff3::merge_texts(&same, &format!("y\n{same}"), &format!("{same}z\n"));
    println!("200,000 equal lines: {r:?} after {}", ms(t.elapsed()));
}
