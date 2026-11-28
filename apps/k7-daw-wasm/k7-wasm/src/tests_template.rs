// tests_template.rs — a library chain file drawn as one chain, through the bytes the ABI carries.
// A reader would get wrong that a refusal answers `error` as a value, not as an error status: the page shows it as a line.

use crate::tests_support::call;
use serde_json::json;

#[test]
fn the_steps_draw_as_one_chain_from_in_to_out() {
    let text = [
        "steps([",
        "  id(\"a\").extends(\"@k7-core/audio/eq\"),",
        "  id(\"b\").extends(\"@k7-core/audio/comp\").bypass(true),",
        "  to([chain(\"@out\")]),",
        "])",
        "",
    ]
    .join("\n");
    let got = call("reads.template_graph", json!({ "name": "@k7-core/x", "text": text }));
    assert_eq!(
        got["graph"]["chains"]["@k7-core/x"],
        json!({
            "from": "@in",
            "use": [
                { "module": "a", "uses": "@k7-core/audio/eq" },
                { "module": "b", "uses": "@k7-core/audio/comp", "enabled": false },
            ],
            "out": [{ "to": "@out", "gain": 1 }],
        })
    );
}

#[test]
fn a_text_that_does_not_read_answers_its_reason_and_a_parse_failure_the_line_it_stops_on() {
    assert!(
        call("reads.template_graph", json!({ "name": "x", "text": "steps([3])\n" }))
            .get("error")
            .is_some()
    );
    let got = call(
        "reads.template_graph",
        json!({ "name": "x", "text": "steps([\n  id(\n" }),
    );
    let reason = got["error"].as_str().expect("a reason");
    let digits = reason.chars().take_while(char::is_ascii_digit).count();
    assert!(digits > 0 && reason[digits..].starts_with(": "), "{reason}");
}
