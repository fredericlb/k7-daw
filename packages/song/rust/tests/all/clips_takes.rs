// clips_takes.rs — an event's comp in beats: which material plays over which span, and the crossfades at its seams.
// A reader would get wrong that a comp span is in SONG time, and that a take is never read before its first beat.

use song::clips::{Budget, CompSpan, Piece, SeamFade, TakeRef, comp_pieces, seam_crossfades};

fn base() -> Piece {
    Piece {
        ref_: "gtr".into(),
        start: 16.0,
        offset: 0.0,
        length: Some(8.0),
    }
}

fn piece(r: &str, start: f64, offset: f64, length: f64) -> Piece {
    Piece {
        ref_: r.into(),
        start,
        offset,
        length: Some(length),
    }
}

fn take(r: &str, start: f64) -> TakeRef {
    TakeRef { ref_: r.into(), start }
}

fn span(t: &str, from: f64, to: f64) -> CompSpan {
    CompSpan {
        take: t.into(),
        from,
        to,
    }
}

fn big() -> Budget {
    Budget::new(song::clips::BUDGET)
}

#[test]
fn the_event_is_its_own_material_when_nothing_is_comped() {
    // takes.test.ts "is the event's own material when nothing is comped"
    assert_eq!(comp_pieces(&base(), &[], &[], &mut big()), Some(vec![base()]));
}

#[test]
fn a_take_punched_into_the_middle_is_read_from_where_the_span_starts_in_it() {
    // takes.test.ts "punches a take into the middle"
    let got = comp_pieces(&base(), &[take("t2", 16.0)], &[span("t2", 18.0, 20.0)], &mut big());
    assert_eq!(
        got,
        Some(vec![
            piece("gtr", 16.0, 0.0, 2.0),
            piece("t2", 18.0, 2.0, 2.0),
            piece("gtr", 20.0, 4.0, 4.0)
        ])
    );
}

#[test]
fn a_span_past_the_end_extends_the_event_and_silence_plays_nothing() {
    // takes.test.ts "extends the event past its end, and plays nothing where a span is silence"
    let got = comp_pieces(
        &base(),
        &[take("t2", 22.0)],
        &[span("t2", 22.0, 28.0), span("", 17.0, 18.0)],
        &mut big(),
    );
    assert_eq!(
        got,
        Some(vec![
            piece("gtr", 16.0, 0.0, 1.0),
            piece("gtr", 18.0, 2.0, 4.0),
            piece("t2", 22.0, 0.0, 6.0)
        ])
    );
}

#[test]
fn a_take_is_never_read_before_its_first_beat() {
    // takes.test.ts "never reads a take before its first beat"
    let got = comp_pieces(&base(), &[take("t2", 19.0)], &[span("t2", 17.0, 21.0)], &mut big()).expect("pieces");
    assert_eq!(got.iter().find(|p| p.ref_ == "t2"), Some(&piece("t2", 19.0, 0.0, 2.0)));
}

#[test]
fn an_unknown_take_silences_its_span() {
    // decision 162's neighbour: a take the event does not hold has no start, so the span cuts and plays nothing
    let got = comp_pieces(&base(), &[], &[span("nope", 18.0, 20.0)], &mut big()).expect("pieces");
    assert_eq!(got, vec![piece("gtr", 16.0, 0.0, 2.0), piece("gtr", 20.0, 4.0, 4.0)]);
}

fn faded(at: f64, length: f64, take_start: f64) -> Vec<song::clips::FadedPiece> {
    let pieces = comp_pieces(
        &base(),
        &[take("t2", take_start)],
        &[span("t2", 18.0, 20.0)],
        &mut big(),
    )
    .expect("pieces");
    seam_crossfades(&pieces, &[SeamFade { at, length }], &|_| Some(12.0), &mut big()).expect("faded")
}

#[test]
fn a_seam_crossfade_overlaps_the_two_pieces_centred_and_within_their_material() {
    // takes.test.ts "overlaps the two pieces at a seam by the crossfade, centred, within their material"
    let f = faded(18.0, 1.0, 16.0);
    assert_eq!(
        (f[0].piece.start, f[0].piece.length, f[0].fade_out),
        (16.0, Some(2.5), Some(1.0))
    );
    assert_eq!(
        (f[1].piece.start, f[1].piece.offset, f[1].fade_in),
        (17.5, 1.5, Some(1.0))
    );
}

#[test]
fn a_crossfade_shrinks_to_what_a_take_has_before_the_seam() {
    // takes.test.ts "shrinks a crossfade to what a take has before the seam": t2 starts AT the seam, so no crossfade
    let f = faded(18.0, 1.0, 18.0);
    assert_eq!(f[1].fade_in, None);
}

#[test]
fn the_comparisons_keep_their_epsilon() {
    // decision 153: a seam off by less than 1e-9 is still the seam, one off by more is not
    let pieces = vec![piece("a", 0.0, 0.0, 2.0 + 5e-10), piece("b", 2.0, 4.0, 2.0)];
    let near = seam_crossfades(&pieces, &[SeamFade { at: 2.0, length: 1.0 }], &|_| None, &mut big()).expect("faded");
    assert!(near[0].fade_out.is_some());
    let pieces = vec![piece("a", 0.0, 0.0, 2.0 + 1e-8), piece("b", 2.0, 4.0, 2.0)];
    let far = seam_crossfades(&pieces, &[SeamFade { at: 2.0, length: 1.0 }], &|_| None, &mut big()).expect("faded");
    assert!(far[0].fade_out.is_none());
}

#[test]
fn a_nan_start_sorts_last_instead_of_breaking_the_order() {
    // decision 173: JavaScript's subtraction calls a NaN equal to everything, which is no order and panics a Rust sort
    let spans: Vec<CompSpan> = vec![span("t2", f64::NAN, 20.0), span("t2", 18.0, 19.0)];
    assert!(comp_pieces(&base(), &[take("t2", 16.0)], &spans, &mut big()).is_some());
}
