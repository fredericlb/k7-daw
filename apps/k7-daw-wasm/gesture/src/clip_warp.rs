// clip_warp.rs — turning a gesture on the waveform into a `clip warp` line.
// The picture is in REGION seconds and the command speaks a moment of the SOURCE file and a BEAT: a conversion taken backwards puts a stretch point where the drawing never showed.

use crate::js;
use serde::Deserialize;

/// The file to display mapping's constants, as `WebWaveform.sourceMap` sends them.
#[derive(Clone, Copy, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SourceMap {
    /// The file second the region's zero shows.
    pub in_point: f64,
    /// Timeline seconds per source second over the WHOLE clip: the map's overall ratio.
    pub stretch: f64,
    /// The map beat the region's zero sits on.
    pub zero_beat: f64,
}

/// One point of the map in both spaces: where the moment is drawn, and the FILE second it pins.
#[derive(Clone, Copy, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WarpPoint {
    pub orig: f64,
    pub src: f64,
}

/// A region second to the second of the SOURCE FILE drawn there: PIECEWISE, because the picture is.
pub fn to_file_sec(display_sec: f64, map: &SourceMap, marks: &[WarpPoint]) -> f64 {
    let rate = if map.stretch > 0.0 { map.stretch } else { 1.0 };
    let mut pts = marks.to_vec();
    pts.sort_by(|a, b| (a.orig - b.orig).partial_cmp(&0.0).unwrap_or(std::cmp::Ordering::Equal));
    if pts.len() < 2 {
        return display_sec / rate + map.in_point;
    }
    let seg = |i: usize| -> f64 {
        let (a, b) = (pts[i], pts[i + 1]);
        let d = b.orig - a.orig;
        if d > 0.0 {
            a.src + ((display_sec - a.orig) / d) * (b.src - a.src)
        } else {
            a.src
        }
    };
    if display_sec <= pts[0].orig {
        return seg(0);
    }
    for i in 0..pts.len() - 1 {
        if display_sec <= pts[i + 1].orig {
            return seg(i);
        }
    }
    seg(pts.len() - 2)
}

/// A region second to the BEAT of the map that lands there: the map counts from the FILE's start, the region from the in-point.
pub fn to_clip_beat(display_sec: f64, tempo: f64, map: &SourceMap) -> f64 {
    if tempo > 0.0 {
        map.zero_beat + (display_sec * tempo) / 60.0
    } else {
        map.zero_beat
    }
}

/// The line that pins the source moment under `at_sec` to the beat under `to_sec`: one verb for the double-click and the drag alike.
pub fn warp_line(clip: &str, at_sec: f64, to_sec: f64, map: &SourceMap, tempo: f64, marks: &[WarpPoint]) -> String {
    let src = js::max(0.0, to_file_sec(at_sec, map, marks));
    let beat = js::max(0.0, to_clip_beat(to_sec, tempo, map));
    format!("clip warp {clip} {} {}", js::to_fixed(src, 4), js::to_fixed(beat, 3))
}

#[cfg(test)]
mod tests {
    use super::*;

    // A clip trimmed to start 2 s in, played at half speed: one source second occupies two of the picture.
    const FLAT: SourceMap = SourceMap {
        in_point: 2.0,
        stretch: 0.5,
        zero_beat: 0.0,
    };
    const STRAIGHT: SourceMap = SourceMap {
        in_point: 0.0,
        stretch: 2.0,
        zero_beat: 0.0,
    };
    fn bent() -> Vec<WarpPoint> {
        [(0.0, 0.0), (4.0, 1.0), (8.0, 4.0)]
            .iter()
            .map(|&(orig, src)| WarpPoint { orig, src })
            .collect()
    }
    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} is not {b}");
    }

    #[test]
    fn the_file_second_is_the_inverse_of_the_producers_ratio_when_the_map_is_straight() {
        for file in [2.0, 3.5, 10.0, 61.25] {
            close(to_file_sec((file - FLAT.in_point) * FLAT.stretch, &FLAT, &[]), file);
        }
    }

    #[test]
    fn the_pictures_zero_is_the_clips_in_point_not_the_files_head() {
        assert_eq!(to_file_sec(0.0, &FLAT, &[]), 2.0);
    }

    #[test]
    fn the_file_second_follows_the_map_segment_by_segment() {
        close(to_file_sec(2.0, &STRAIGHT, &bent()), 0.5);
        close(to_file_sec(6.0, &STRAIGHT, &bent()), 2.5);
        for p in bent() {
            close(to_file_sec(p.orig, &STRAIGHT, &bent()), p.src);
        }
    }

    #[test]
    fn the_marks_come_in_any_order() {
        let b = bent();
        close(to_file_sec(2.0, &STRAIGHT, &[b[2], b[0], b[1]]), 0.5);
    }

    #[test]
    fn it_falls_back_to_the_ratio_with_fewer_than_two_marks() {
        let one = [bent()[0]];
        close(
            to_file_sec(
                3.0,
                &SourceMap {
                    in_point: 0.0,
                    stretch: 1.0,
                    zero_beat: 0.0,
                },
                &one,
            ),
            3.0,
        );
        assert!(
            to_file_sec(
                3.0,
                &SourceMap {
                    in_point: 0.0,
                    stretch: 0.0,
                    zero_beat: 0.0
                },
                &[]
            )
            .is_finite()
        );
    }

    #[test]
    fn the_beat_reads_a_marks_target_backwards_and_adds_zero_beat_back() {
        close(to_clip_beat((8.0 * 60.0) / 120.0, 120.0, &FLAT), 8.0);
        let trimmed = SourceMap {
            in_point: 2.0,
            stretch: 1.0,
            zero_beat: 6.0,
        };
        assert_eq!(to_clip_beat(0.0, 120.0, &trimmed), 6.0);
        close(to_clip_beat((2.0 * 60.0) / 120.0, 120.0, &trimmed), 8.0);
    }

    #[test]
    fn the_line_names_the_source_second_and_the_beat() {
        assert_eq!(
            warp_line("break", 4.0, 2.0, &FLAT, 120.0, &[]),
            "clip warp break 10.0000 4.000"
        );
    }

    #[test]
    fn a_double_click_pins_a_moment_to_where_it_already_is() {
        assert_eq!(
            warp_line("break", 4.0, 4.0, &FLAT, 120.0, &[]),
            "clip warp break 10.0000 8.000"
        );
    }

    #[test]
    fn it_restates_an_existing_point_unchanged_so_placing_one_beside_it_bends_nothing() {
        let b = bent();
        assert_eq!(
            warp_line("b", b[1].orig, b[1].orig, &STRAIGHT, 120.0, &b),
            "clip warp b 1.0000 8.000"
        );
    }

    #[test]
    fn it_never_emits_a_negative_second_or_beat() {
        let map = SourceMap {
            in_point: 0.0,
            stretch: 1.0,
            zero_beat: 0.0,
        };
        assert_eq!(warp_line("b", -5.0, -5.0, &map, 120.0, &[]), "clip warp b 0.0000 0.000");
    }

    #[test]
    fn a_tie_on_the_fixed_digits_goes_up_as_to_fixed_does() {
        let map = SourceMap {
            in_point: 0.0,
            stretch: 1.0,
            zero_beat: 0.0,
        };
        assert_eq!(
            warp_line("b", 0.03125, 0.0, &map, 120.0, &[]),
            "clip warp b 0.0313 0.000"
        );
    }
}
