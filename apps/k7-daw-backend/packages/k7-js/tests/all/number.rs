//! k7-js's native number formats against what node printed for the edge cases of `fixtures/number-vectors.json`.
//! What a reader gets wrong: the table is a snapshot of node's answers (bit pattern, then the text), not a second implementation.

use k7_js::number::{math_round, to_fixed, to_string};
use serde_json::Value;

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
