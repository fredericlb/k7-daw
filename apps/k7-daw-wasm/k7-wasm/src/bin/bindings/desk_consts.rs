// desk_consts.rs — the DJ desk's constants as TypeScript literals: the decks, the ends of every knob, the loop and click numbers.
// A reader would get wrong that `deckVocabulary.ts` writes none of them: it re-exports these, so the page reads Rust's numbers.

use serde_json::{Value, json};
use turntables::vocab as v;

/// One exported constant: its doc line, its name and its value, `as const` for a table.
fn export(doc: &str, name: &str, value: &Value) -> [String; 3] {
    let frozen = if value.is_array() || value.is_object() {
        " as const"
    } else {
        ""
    };
    [
        format!("/** {doc} */"),
        format!("export const {name} = {value}{frozen};"),
        String::new(),
    ]
}

pub fn desk() -> Vec<[String; 3]> {
    let bands: Vec<Value> = v::EQ_BANDS.iter().map(|id| json!({ "id": id })).collect();
    vec![
        export(
            "The two decks, named as the project names them.",
            "DECKS",
            &json!(v::DECKS),
        ),
        export(
            "The crossfader's middle, where a set starts.",
            "MIX_CENTRE",
            &json!(v::MIX_CENTRE),
        ),
        export(
            "The tempo the timeline runs at; each deck warps its own clip.",
            "DOC_TEMPO",
            &json!(v::DOC_TEMPO),
        ),
        export(
            "How far a tempo bends either way.",
            "PITCH_RANGE",
            &json!(v::PITCH_RANGE),
        ),
        export(
            "The module a deck's tempo belongs to.",
            "TEMPO_MODULE",
            &json!(v::TEMPO_MODULE),
        ),
        export(
            "The tempo module's target param.",
            "TEMPO_PARAM",
            &json!(v::TEMPO_PARAM),
        ),
        export(
            "The tempo module's measured param.",
            "TEMPO_SOURCE_PARAM",
            &json!(v::TEMPO_SOURCE_PARAM),
        ),
        export("The ends of the tempo fader.", "BPM_RANGE", &json!(v::BPM_RANGE)),
        export("The ends of an EQ knob, in dB.", "EQ_DB", &json!(v::EQ_DB)),
        export(
            "The ends of a deck's filter knob.",
            "FILTER_RANGE",
            &json!(v::FILTER_RANGE),
        ),
        export("The ends of the input trim, in dB.", "TRIM_DB", &json!(v::TRIM_DB)),
        export("The ends of the channel fader, in dB.", "LEVEL_DB", &json!(v::LEVEL_DB)),
        export(
            "The echo's amount, dry to fully wet.",
            "ECHO_RANGE",
            &json!(v::ECHO_RANGE),
        ),
        export(
            "How fast a record runs backwards under the censor.",
            "CENSOR_RATE",
            &json!(v::CENSOR_RATE),
        ),
        export(
            "The three bands a deck's EQ strip draws, low to high.",
            "EQ_BANDS",
            &Value::Array(bands),
        ),
        export(
            "The metre everything on the desk assumes.",
            "BEATS_PER_BAR",
            &json!(v::BEATS_PER_BAR),
        ),
        export(
            "How far a tempo must move for a drag to send again.",
            "LIVE_WARP_BPM",
            &json!(v::LIVE_WARP_BPM),
        ),
        export(
            "The lengths a loop button offers, in beats.",
            "LOOP_BEATS",
            &json!(v::LOOP_BEATS),
        ),
        export(
            "What a beat jump moves by before anybody has chosen a length.",
            "DEFAULT_LOOP_BEATS",
            &json!(v::DEFAULT_LOOP_BEATS),
        ),
    ]
}
