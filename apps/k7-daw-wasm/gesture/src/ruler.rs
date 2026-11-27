// ruler.rs — the arrangement ruler's ladder: a tick's weight and number, the ticks between two beats, the scale a zoom draws at.
// Ticks count from the ruler's ORIGIN, never from the first visible one, so panning cannot re-place a line.

use crate::js;
use crate::ticks::{
    MAX_COLUMN_PX, MONO_CHAR_PX, choose_tick, format_clock, format_second_tick, label_min_px, label_stride,
    second_ticks, subdivide_beats, ticks_up_to,
};
use serde::{Deserialize, Serialize};

/// What the ruler is READING IN: not what the song is placed in, which is always beats.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum RulerMode {
    Bars,
    Seconds,
}

/// A JavaScript-style integer in text, for the labels and keys that print a float.
fn int(x: f64) -> String {
    js::number_string(x)
}

/// A tick's weight and its number: asked by the ruler and by the lanes' grid, which must read one answer.
pub fn tick_at(b: f64, bar_beats: f64) -> (bool, bool, String) {
    let bar = (b / bar_beats - js::round(b / bar_beats)).abs() < 1e-6;
    let on_beat = (b - js::round(b)).abs() < 1e-6;
    let label = if bar {
        int(js::round(b / bar_beats) + 1.0)
    } else if on_beat {
        format!(
            "{}.{}",
            int((b / bar_beats).floor() + 1.0),
            int(js::round(b % bar_beats) + 1.0)
        )
    } else {
        String::new()
    };
    (bar, on_beat, label)
}

/// Every tick drawn between the view's edges, accumulated from the first one on the unit's own grid.
pub fn tick_beats(beat_lo: f64, last_beat: f64, unit: f64) -> Vec<f64> {
    let mut out = Vec::new();
    let mut b = (beat_lo / unit - 1e-9).ceil() * unit;
    while b <= last_beat {
        out.push(b);
        b += unit;
    }
    out
}

/// One tick of a ruler.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RulerTick {
    /// Where it sits: the surface's one coordinate, whatever the ruler counts in.
    pub beat: f64,
    /// Its number from the ORIGIN, for the label stride.
    pub index: f64,
    pub key: String,
    pub label: String,
    /// Coarse, medium or fine: a bar, a beat, a subdivision; or a minute, a second, a fraction.
    pub level: u8,
}

/// What a ruler is asked to tick.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct TickSpec {
    pub mode: RulerMode,
    pub beat_lo: f64,
    pub last_beat: f64,
    pub unit: f64,
    pub bar_beats: f64,
    /// Beats per second: the whole of the conversion, since there is no tempo map.
    pub bps: f64,
    pub sec_step: f64,
    /// The slice actually ON SCREEN, when the caller knows it; `index` is still computed from the ORIGIN.
    #[cfg_attr(feature = "ts", ts(optional))]
    #[serde(default)]
    pub view: Option<View>,
    /// The picture's frame rate, when the song has one.
    #[cfg_attr(feature = "ts", ts(optional))]
    #[serde(default)]
    pub fps: Option<f64>,
}

/// The beat range on screen.
#[derive(Clone, Copy, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct View {
    pub lo: f64,
    pub hi: f64,
}

/// The ticks a ruler draws, in BEATS whichever unit it reads in: one function for both ladders.
pub fn ruler_ticks(s: &TickSpec) -> Vec<RulerTick> {
    let lo = js::max(s.beat_lo, s.view.map_or(s.beat_lo, |v| v.lo));
    let hi = js::min(s.last_beat, s.view.map_or(s.last_beat, |v| v.hi));
    if s.mode == RulerMode::Bars {
        return tick_beats(lo, hi, s.unit)
            .into_iter()
            .map(|b| {
                let (bar, on_beat, label) = tick_at(b, s.bar_beats);
                RulerTick {
                    beat: b,
                    index: js::round(b / s.unit),
                    key: format!("b:{}", int(b)),
                    label,
                    level: if bar {
                        0
                    } else if on_beat {
                        1
                    } else {
                        2
                    },
                }
            })
            .collect();
    }
    let mut out = Vec::new();
    // From the ORIGIN and not from the first visible second, exactly as `tick_beats` is, or panning would re-place every line.
    for sec in ticks_up_to(hi / s.bps, s.sec_step) {
        let beat = sec * s.bps;
        if beat < lo - 1e-9 {
            continue;
        }
        let whole = (sec - js::round(sec)).abs() < 1e-6;
        out.push(RulerTick {
            beat,
            index: js::round(sec / s.sec_step),
            key: format!("s:{}", int(sec)),
            label: format_second_tick(sec, s.sec_step, s.fps),
            // A minute carries the bar's weight and a whole second the beat's: the same three-level rhythm in either unit.
            level: if (sec % 60.0).abs() < 1e-6 {
                0
            } else if whole {
                1
            } else {
                2
            },
        });
    }
    out
}

/// The scale a ruler draws at: ONE function, so the timeline and the clip editors cannot pick different ladders for the same zoom.
pub fn ruler_scale(
    mode: RulerMode,
    ppb: f64,
    bar_beats: f64,
    bps: f64,
    last_beat: f64,
    fps: Option<f64>,
) -> (f64, f64, f64) {
    let unit = subdivide_beats(bar_beats, ppb, MAX_COLUMN_PX);
    let widest_clock = format_clock(js::max(last_beat, 0.0) / bps, 1.0);
    let sec_step = choose_tick(
        &second_ticks(fps),
        ppb / bps,
        label_min_px(widest_clock.chars().count() as f64, MONO_CHAR_PX),
    );
    let last_bar_no = (js::max(last_beat, 0.0) / bar_beats).floor() + 1.0;
    let widest = if unit < bar_beats {
        format!("{}.{}", int(last_bar_no), int(js::round(bar_beats)))
    } else {
        int(last_bar_no)
    };
    let bar_stride = if mode == RulerMode::Bars {
        label_stride(unit * ppb, label_min_px(widest.chars().count() as f64, MONO_CHAR_PX))
    } else {
        1.0
    };
    (unit, sec_step, bar_stride)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(mode: RulerMode, lo: f64, hi: f64, unit: f64, view: Option<(f64, f64)>) -> TickSpec {
        TickSpec {
            mode,
            beat_lo: lo,
            last_beat: hi,
            unit,
            bar_beats: 4.0,
            bps: 2.0,
            sec_step: 1.0,
            view: view.map(|(lo, hi)| View { lo, hi }),
            fps: None,
        }
    }
    fn bars(lo: f64, hi: f64, unit: f64, view: Option<(f64, f64)>) -> Vec<RulerTick> {
        ruler_ticks(&spec(RulerMode::Bars, lo, hi, unit, view))
    }

    #[test]
    fn builds_the_whole_range_when_nobody_says_what_is_on_screen() {
        assert_eq!(bars(0.0, 16.0, 1.0, None).len(), 17);
    }

    #[test]
    fn builds_only_the_slice_asked_for() {
        let all = bars(0.0, 64.0, 1.0, None);
        let seen = bars(0.0, 64.0, 1.0, Some((10.0, 20.0)));
        assert_eq!((all.len(), seen.len()), (65, 11));
        assert_eq!(seen[0].beat, 10.0);
        assert_eq!(seen[seen.len() - 1].beat, 20.0);
    }

    #[test]
    fn gives_a_tick_the_same_identity_whether_or_not_the_list_starts_at_it() {
        let all = bars(0.0, 64.0, 1.0, None);
        for t in bars(0.0, 64.0, 1.0, Some((10.0, 20.0))) {
            let same = all
                .iter()
                .find(|x| (x.beat - t.beat).abs() < 1e-9)
                .expect("the same tick");
            assert_eq!(&t, same);
        }
    }

    #[test]
    fn never_runs_past_the_range_it_was_given_whatever_the_window_claims() {
        let wide = bars(4.0, 8.0, 1.0, Some((-100.0, 100.0)));
        assert_eq!((wide[0].beat, wide[wide.len() - 1].beat), (4.0, 8.0));
    }

    #[test]
    fn windows_the_seconds_ladder_by_the_same_rule() {
        let all = ruler_ticks(&spec(RulerMode::Seconds, 0.0, 120.0, 1.0, None));
        let seen = ruler_ticks(&spec(RulerMode::Seconds, 0.0, 120.0, 1.0, Some((20.0, 30.0))));
        assert!(seen.len() < all.len());
        for t in seen {
            let same = all.iter().find(|x| x.key == t.key).expect("the same tick");
            assert_eq!((t.index, &t.label), (same.index, &same.label));
        }
    }

    #[test]
    fn bounds_the_count_by_the_pane_not_by_the_song() {
        let unit = 1.0 / 64.0;
        assert_eq!(bars(0.0, 3000.0, unit, None).len(), 3000 * 64 + 1);
        assert!(bars(0.0, 3000.0, unit, Some((100.0, 104.0))).len() < 300);
    }

    #[test]
    fn a_tick_is_weighed_and_numbered() {
        assert_eq!(tick_at(0.0, 4.0), (true, true, "1".to_owned()));
        assert_eq!(tick_at(8.0, 4.0), (true, true, "3".to_owned()));
        assert_eq!(tick_at(5.0, 4.0), (false, true, "2.2".to_owned()));
        assert_eq!(tick_at(5.5, 4.0), (false, false, String::new()));
    }

    #[test]
    fn a_minute_is_coarse_a_whole_second_medium_and_a_fraction_fine() {
        let t = ruler_ticks(&TickSpec {
            sec_step: 0.5,
            ..spec(RulerMode::Seconds, 0.0, 240.0, 1.0, None)
        });
        assert_eq!(t.iter().find(|x| x.key == "s:60").map(|x| x.level), Some(0));
        assert_eq!(t.iter().find(|x| x.key == "s:3").map(|x| x.level), Some(1));
        assert_eq!(t.iter().find(|x| x.key == "s:0.5").map(|x| x.level), Some(2));
    }

    #[test]
    fn the_scale_picks_the_unit_the_seconds_rung_and_the_label_stride() {
        let (unit, sec, stride) = ruler_scale(RulerMode::Bars, 10.0, 4.0, 2.0, 64.0, None);
        assert_eq!(unit, 4.0);
        assert!(sec >= 1.0);
        assert!(stride >= 1.0);
        let (_, _, seconds_stride) = ruler_scale(RulerMode::Seconds, 10.0, 4.0, 2.0, 64.0, None);
        assert_eq!(seconds_stride, 1.0);
    }
}
