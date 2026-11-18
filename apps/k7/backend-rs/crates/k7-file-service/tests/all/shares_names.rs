//! 1m's M1: `/export`'s asks and header beyond the vectors — the hostile names and queries the brief lists (2520, 2521).

use k7_file_service::shares::names::{
    TimelineAsk, check_timeline, content_disposition, mix_name, timeline_ask, wants_mp3,
};

#[test]
fn shares_a_timeline_is_one_plain_name_or_it_is_refused() {
    assert_eq!(check_timeline(&timeline_ask("")), Ok(None));
    assert_eq!(check_timeline(&timeline_ask("?timeline=")), Ok(None));
    assert_eq!(
        check_timeline(&timeline_ask("?timeline=a.b-c_D1")),
        Ok(Some("a.b-c_D1"))
    );
    assert_eq!(
        check_timeline(&timeline_ask("?timeline=a%2Fb")),
        Err("'a/b' names no timeline".into())
    );
    assert_eq!(
        check_timeline(&timeline_ask("?timeline=..")),
        Ok(Some(".."))
    );
    for (q, shown) in [
        ("?timeline=a&timeline=b", r#"["a","b"]"#),
        ("?timeline[]=a", r#"["a"]"#),
        ("?timeline[k]=v", r#"{"k":"v"}"#),
        ("?timeline=a&timeline[]=b", r#"["a","b"]"#),
    ] {
        assert_eq!(
            check_timeline(&timeline_ask(q)),
            Err(format!("'{shown}' names no timeline")),
            "{q}"
        );
    }
}

#[test]
fn shares_a_64_kib_timeline_and_a_hostile_name_answer_at_once() {
    let long = format!("?timeline={}", "a".repeat(64 * 1024));
    assert!(matches!(timeline_ask(&long), TimelineAsk::Name(n) if n.len() == 64 * 1024));
    assert!(check_timeline(&timeline_ask(&long)).is_ok());
    let bad = format!("?timeline={}%2F", "a".repeat(64 * 1024));
    assert!(
        check_timeline(&timeline_ask(&bad))
            .unwrap_err()
            .ends_with("names no timeline")
    );
    let name = mix_name("x\r\nSet-Cookie: y", "/ws/t.k7", None);
    let h = content_disposition(&format!("{name}.wav"));
    assert!(!h.contains('\r') && !h.contains('\n'), "{h}");
    assert!(h.starts_with("attachment; filename=\"x__Set-Cookie_ y.wav\"; filename*=UTF-8''x%0D%0ASet-Cookie_%20y.wav"), "{h}");
    let big = content_disposition(&format!("{}.wav", "日".repeat(20_000)));
    assert!(big.is_ascii());
}

#[test]
fn shares_the_format_is_mp3_only_when_the_whole_ask_says_so() {
    assert!(wants_mp3("?format=mp3"));
    assert!(wants_mp3("?format[]=mp3"));
    for q in [
        "",
        "?format=MP3",
        "?format=wav&format=mp3",
        "?format[k]=mp3",
        "?format=",
    ] {
        assert!(!wants_mp3(q), "{q}");
    }
}

#[test]
fn shares_a_name_keeps_runs_of_forbidden_characters_as_one_underscore() {
    assert_eq!(
        mix_name("a/b\\c:d*e?f\"g<h>i|j", "/x", None),
        "a_b_c_d_e_f_g_h_i_j"
    );
    assert_eq!(mix_name("a//\\b", "/x", Some("v")), "a_b-v");
    assert_eq!(mix_name("", "/", None), "mixdown");
}
