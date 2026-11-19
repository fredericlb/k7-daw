//! The probe's pure half replayed over what the TypeScript answered (`makeHostVectors.mts`): ffprobe's arguments and its JSON as facts.


use k7_mediatek::video_probe::{facts_of, rotation_of};
use serde_json::json;



#[test]
fn text_that_is_no_json_is_an_error_and_a_side_data_rotation_beats_the_tag() {
    assert!(facts_of("not json").is_err());
    let s = json!({ "side_data_list": [{ "rotation": 90 }], "tags": { "rotate": "180" } });
    assert_eq!(rotation_of(&s), 90);
    assert_eq!(rotation_of(&json!({ "tags": { "rotate": "180" } })), 180);
    assert_eq!(rotation_of(&json!({})), 0);
}
