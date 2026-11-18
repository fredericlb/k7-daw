//! 1m's M1: the picture job's numbers and an error body's cap beyond the vectors (2504, 2535).

use k7_file_service::shares::picture::{
    Format, Size, VideoSettings, job_text, resolve_video_settings,
};
use k7_file_service::shares::{ERROR_BODY_CAP, capped, said};

fn plain() -> VideoSettings {
    VideoSettings {
        format: Format::Webm,
        audio: false,
        size: None,
        quality: None,
        from: None,
        to: None,
    }
}

#[test]
fn shares_a_number_is_spelt_as_javascript_spells_it_never_as_serde() {
    let s = VideoSettings {
        from: Some(1e21),
        size: Some(Size::Both(0.1, 1e-7)),
        ..plain()
    };
    let r = resolve_video_settings(&s, [1280.0, 720.0], 30.0, 6.0);
    let text = job_text("{}", &s, &r, false);
    assert!(text.contains("\"from\":1e+21"), "{text}");
    assert!(text.contains("\"size\":[0.1,1e-7]"), "{text}");
    let zero = VideoSettings {
        size: Some(Size::Height(10.0)),
        ..plain()
    };
    let r = resolve_video_settings(&zero, [5.0, 0.0], 30.0, 6.0);
    assert!(job_text("{}", &zero, &r, false).contains("\"size\":[null,10]"));
}

#[test]
fn shares_an_error_body_is_read_up_to_64_kib_at_a_character() {
    let body = "é".repeat(ERROR_BODY_CAP);
    assert!(capped(&body).len() <= ERROR_BODY_CAP);
    assert!(said("x", &body).len() < ERROR_BODY_CAP + 8);
    assert_eq!(said("naming", r#"{"error":"gone"}"#), "naming: gone");
    assert_eq!(said("naming", "<html>"), "naming: <html>");
}

#[test]
fn shares_a_quality_is_one_of_four_and_an_empty_one_is_no_setting() {
    use k7_file_service::shares::picture::Quality;
    for q in ["low", "medium", "high", "max"] {
        assert_eq!(Quality::parse(q).map(Quality::as_str), Some(q));
    }
    for q in ["", "MAX", "ultra", " high"] {
        assert_eq!(Quality::parse(q), None, "{q:?}");
    }
}
