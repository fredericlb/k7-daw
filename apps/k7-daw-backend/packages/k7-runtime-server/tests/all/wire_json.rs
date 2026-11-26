//! wire_json.rs — `wire::json::dump` against nlohmann's own output (`fixtures/nlohmann_dump.txt`, which
//! k7-runtime's `WireJson_test.cpp` holds to nlohmann): doubles by their bits, then whole documents.

use k7_runtime_server::wire::json::dump;
use serde_json::Value;

#[test]
fn every_fixture_line_prints_as_nlohmann_printed_it() {
    let text = crate::repo_file(
        "apps/k7-daw-backend/packages/k7-runtime-server/fixtures/nlohmann_dump.txt",
    );
    let mut checked = 0;
    for line in text.lines() {
        let mut parts = line.splitn(3, '\t');
        let (Some(kind), Some(input), Some(expected)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let got = match kind {
            "f64" => {
                let x = f64::from_bits(u64::from_str_radix(input, 16).unwrap());
                dump(&serde_json::json!(x))
            }
            _ => dump(&serde_json::from_str::<Value>(input).unwrap()),
        };
        assert_eq!(got, expected, "{kind} {input}");
        checked += 1;
    }
    assert!(checked > 1000, "only {checked} lines");
}

#[test]
fn a_float_widened_from_f32_prints_its_double() {
    let level: f32 = -3.1;
    assert_eq!(
        dump(&serde_json::json!(f64::from(level))),
        "-3.0999999046325684"
    );
}

#[test]
fn a_value_that_is_not_finite_is_null() {
    let mut out = String::new();
    k7_runtime_server::wire::json::write_f64(&mut out, f64::INFINITY);
    assert_eq!(out, "null");
}

#[test]
fn an_integer_minus_zero_is_read_as_a_float() {
    // nlohmann reads `-0` as the integer 0 and serde_json as -0.0: a difference only a message echoed back can show.
    assert_eq!(dump(&serde_json::from_str::<Value>("-0").unwrap()), "-0.0");
}
