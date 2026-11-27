// fuzzy.rs — the one subsequence matcher the shell searches with: the port of `fuzzy.ts`'s 24 lines, no library (decision 10).
// A reader would get wrong that the score is an integer: the length term makes it fractional, and -1 alone says no match.

/// Characters after which a hit counts as the start of a word.
const BOUNDARY: &str = "/-_. ";

/// Fuzzy subsequence match, -1 for no match: contiguous runs and post-separator hits score higher, and shorter strings win ties.
#[must_use]
pub fn fuzzy_score(q: &str, s: &str) -> f64 {
    // Indexed as UTF-16 units, as the page's strings were: a lone unit never equals an astral char.
    let t: Vec<u16> = s.to_lowercase().encode_utf16().collect();
    let mut score = 0.0_f64;
    let mut si = 0;
    let mut run = 0.0_f64;
    for ch in q.to_lowercase().chars() {
        let mut buf = [0u16; 2];
        let unit = match ch.encode_utf16(&mut buf) {
            [u] => Some(*u),
            _ => None,
        };
        let start = si;
        while si < t.len() && Some(t[si]) != unit {
            si += 1;
        }
        if si >= t.len() {
            return -1.0;
        }
        let at_boundary = si == 0 || char::from_u32(u32::from(t[si - 1])).is_some_and(|c| BOUNDARY.contains(c));
        let contiguous = si == start;
        score += 1.0 + if contiguous { run * 2.0 } else { 0.0 } + if at_boundary { 3.0 } else { 0.0 };
        run = if contiguous { run + 1.0 } else { 1.0 };
        si += 1;
    }
    score - t.len() as f64 * 0.01
}

/// Each string's score against `q`, in order: one call for a whole list.
#[must_use]
pub fn fuzzy_scores(q: &str, strings: &[String]) -> Vec<f64> {
    strings.iter().map(|s| fuzzy_score(q, s)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_subsequence_matches_and_anything_else_is_minus_one() {
        assert!(fuzzy_score("tmt", "track mute") >= 0.0);
        assert_eq!(fuzzy_score("xyz", "track mute"), -1.0);
        assert_eq!(fuzzy_score("mt", "tm"), -1.0);
    }

    #[test]
    fn a_contiguous_run_beats_a_scattered_one_and_shorter_wins_a_tie() {
        assert!(fuzzy_score("mute", "track mute") > fuzzy_score("mute", "m u t e"));
        assert!(fuzzy_score("a", "ab") > fuzzy_score("a", "abc"));
    }

    #[test]
    fn a_word_start_scores_above_a_mid_word_hit() {
        assert!(fuzzy_score("m", "track mute") > fuzzy_score("u", "track mute"));
    }

    #[test]
    fn the_scores_are_the_pages_own_numbers() {
        // 'tm': t at 0 (1 + 3), m at 6 after a space (1 + 3), minus 10 chars at 0.01.
        assert!((fuzzy_score("tm", "track mute") - (8.0 - 0.10)).abs() < 1e-12);
        assert!((fuzzy_score("MUTE", "track mute") - fuzzy_score("mute", "track mute")).abs() < 1e-12);
    }
}
