// trim.rs — where a take starts and stops sounding: the first and last bucket of an envelope above a gate in dBFS.
// A reader would get wrong that it is bucket-accurate with one bucket of margin, and that `within` is a cage, not a hint.

/// The default gate: -40 dBFS, below a room's noise floor and above true digital silence.
pub const DEFAULT_TRIM_DB: f64 = -40.0;

/// What a zero bucket reads as, so a comparison is total.
const MIN_DB: f64 = -100.0;

/// Seconds into the file where the sound starts and stops; `from` < `to` always.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrimWindow {
    pub from: f64,
    pub to: f64,
}

/// A linear 0..1 amplitude as dBFS, with zero at the floor rather than minus infinity.
#[must_use]
pub fn amp_to_db(amp: f64) -> f64 {
    if amp > 0.0 {
        20.0 * amp.log10()
    } else {
        MIN_DB
    }
}

/// The span of `env` above `gate_db` in seconds, bucket-accurate and clamped to `within`.
#[must_use]
pub fn trim_window(
    env: &[f64],
    seconds: f64,
    gate_db: f64,
    within: Option<TrimWindow>,
) -> Option<TrimWindow> {
    if env.is_empty() || seconds.is_nan() || seconds <= 0.0 {
        return None;
    }
    let per = seconds / env.len() as f64;
    let lo = within.map_or(0.0, |w| w.from).min(seconds).max(0.0);
    let hi = within.map_or(seconds, |w| w.to).min(seconds).max(lo);
    if hi.is_nan() || hi <= lo {
        return None;
    }
    // The buckets the interval touches: a partly covered one counts.
    let i0 = (lo / per).floor().max(0.0) as usize;
    let last = env.len() as f64 - 1.0;
    let i1 = ((hi / per).ceil() - 1.0).min(last);
    if i1 < 0.0 {
        return None;
    }
    let (mut first, mut last_hit) = (None, 0usize);
    for (i, a) in env.iter().enumerate().take(i1 as usize + 1).skip(i0) {
        if amp_to_db(*a) > gate_db {
            first.get_or_insert(i);
            last_hit = i;
        }
    }
    let first = first?;
    let from = lo.max((first as f64 - 1.0) * per);
    let to = hi.min((last_hit as f64 + 2.0) * per);
    // A single bucket over the gate still has to be a window, not a point.
    (to > from).then_some(TrimWindow { from, to })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db(v: f64) -> f64 {
        10f64.powf(v / 20.0)
    }
    fn w(from: f64, to: f64) -> Option<TrimWindow> {
        Some(TrimWindow { from, to })
    }
    fn win(a: f64, b: f64) -> Option<TrimWindow> {
        w(a, b)
    }

    #[test]
    fn cuts_the_silence_at_both_ends() {
        let loud = db(-6.0);
        let env = [0.0, 0.0, 0.0, loud, loud, loud, loud, 0.0, 0.0, 0.0];
        assert_eq!(trim_window(&env, 10.0, -40.0, None), w(2.0, 8.0));
    }

    #[test]
    fn keeps_a_bucket_of_margin() {
        assert_eq!(
            trim_window(&[0.0, db(-6.0), 0.0], 3.0, -40.0, None),
            w(0.0, 3.0)
        );
    }

    #[test]
    fn does_not_run_past_the_file() {
        let l = db(-6.0);
        assert_eq!(trim_window(&[l, l, l], 3.0, -40.0, None), w(0.0, 3.0));
    }

    #[test]
    fn reads_the_gate_in_dbfs() {
        let env = [0.0, db(-60.0), 0.0, db(-6.0), 0.0];
        assert_eq!(trim_window(&env, 5.0, -40.0, None), w(2.0, 5.0));
        assert_eq!(trim_window(&env, 5.0, -70.0, None), w(0.0, 5.0));
    }

    #[test]
    fn ignores_sound_outside_the_interval() {
        let l = db(-6.0);
        let env = [0.0, l, 0.0, 0.0, 0.0, l, 0.0, 0.0, l, 0.0];
        assert_eq!(trim_window(&env, 10.0, -40.0, win(3.0, 7.0)), w(4.0, 7.0));
    }

    #[test]
    fn clamps_the_margin_to_the_interval() {
        let env = [0.0, 0.0, 0.0, db(-6.0), 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        assert_eq!(trim_window(&env, 10.0, -40.0, win(3.0, 7.0)), w(3.0, 5.0));
    }

    #[test]
    fn counts_a_bucket_the_interval_only_touches() {
        let env = [0.0, 0.0, 0.0, 0.0, 0.0, db(-6.0), 0.0, 0.0, 0.0, 0.0];
        assert_eq!(trim_window(&env, 10.0, -40.0, win(3.0, 5.5)), w(4.0, 5.5));
    }

    #[test]
    fn answers_none_when_the_interval_holds_nothing() {
        let l = db(-6.0);
        let env = [l, l, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, l, l];
        assert_eq!(trim_window(&env, 10.0, -40.0, win(3.0, 7.0)), None);
    }

    #[test]
    fn survives_an_interval_the_file_outgrew_and_one_backwards() {
        let env = [0.0, db(-6.0), 0.0];
        assert_eq!(trim_window(&env, 3.0, -40.0, win(0.0, 99.0)), w(0.0, 3.0));
        assert_eq!(trim_window(&env, 3.0, -40.0, win(2.0, 1.0)), None);
    }

    #[test]
    fn answers_none_when_there_is_nothing_to_keep() {
        assert_eq!(trim_window(&[0.0, 0.0, 0.0], 3.0, -40.0, None), None);
        assert_eq!(trim_window(&[db(-60.0), db(-60.0)], 2.0, -40.0, None), None);
        assert_eq!(trim_window(&[], 10.0, -40.0, None), None);
        assert_eq!(trim_window(&[db(-6.0)], 0.0, -40.0, None), None);
    }

    #[test]
    fn puts_a_zero_bucket_at_the_floor() {
        assert_eq!(amp_to_db(0.0), -100.0);
        assert!(amp_to_db(1.0).abs() < 1e-6);
        assert!((amp_to_db(db(-20.0)) + 20.0).abs() < 1e-6);
    }
}
