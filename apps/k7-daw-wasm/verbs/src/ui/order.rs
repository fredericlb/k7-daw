// order.rs — the text order `localeCompare` gives ASCII and Latin names: spaces and marks, then digits, then letters, case last.
// A reader would get wrong that this is a full collation: it is the root order for the catalogue's words, which is what ties need.

use std::cmp::Ordering;

/// The marks ICU's root order lists after whitespace, in its order.
const MARKS: &str = "_-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$";

/// The primary weight of one char: whitespace, then marks, then digits, then letters, accents and case folded.
fn primary(c: char) -> (u8, u32) {
    let lower = fold_accent(c).to_lowercase().next().unwrap_or(c);
    if lower.is_alphabetic() {
        (3, lower as u32)
    } else if lower.is_numeric() {
        (2, lower as u32)
    } else if lower.is_whitespace() {
        (0, lower as u32)
    } else {
        (1, MARKS.find(lower).map_or(1000 + lower as u32, |i| i as u32))
    }
}

/// The base letter of a Latin-1 accented one; every other char is itself.
fn fold_accent(c: char) -> char {
    match c {
        '\u{e0}'..='\u{e5}' => 'a',
        '\u{c0}'..='\u{c5}' => 'A',
        '\u{e7}' => 'c',
        '\u{c7}' => 'C',
        '\u{e8}'..='\u{eb}' => 'e',
        '\u{c8}'..='\u{cb}' => 'E',
        '\u{ec}'..='\u{ef}' => 'i',
        '\u{cc}'..='\u{cf}' => 'I',
        '\u{f1}' => 'n',
        '\u{d1}' => 'N',
        '\u{f2}'..='\u{f6}' => 'o',
        '\u{d2}'..='\u{d6}' => 'O',
        '\u{f9}'..='\u{fc}' => 'u',
        '\u{d9}'..='\u{dc}' => 'U',
        '\u{fd}' | '\u{ff}' => 'y',
        '\u{dd}' => 'Y',
        other => other,
    }
}

/// `a.localeCompare(b)` as a total order: letters by their base first, then accents, then lowercase before uppercase.
#[must_use]
pub fn locale_cmp(a: &str, b: &str) -> Ordering {
    a.chars()
        .map(primary)
        .cmp(b.chars().map(primary))
        .then_with(|| {
            a.chars()
                .map(|c| fold_accent(c) != c)
                .cmp(b.chars().map(|c| fold_accent(c) != c))
        })
        .then_with(|| a.chars().map(char::is_uppercase).cmp(b.chars().map(char::is_uppercase)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_come_before_digits_and_digits_before_letters() {
        assert_eq!(locale_cmp("track", "track mute"), Ordering::Less);
        assert_eq!(locale_cmp("a b", "a-b"), Ordering::Less);
        assert_eq!(locale_cmp("a-b", "a1"), Ordering::Less);
        assert_eq!(locale_cmp("a1", "ab"), Ordering::Less);
    }

    #[test]
    fn case_and_accents_only_break_a_tie() {
        assert_eq!(locale_cmp("a", "B"), Ordering::Less);
        assert_eq!(locale_cmp("e", "\u{e9}"), Ordering::Less);
        assert_eq!(locale_cmp("\u{e9}", "f"), Ordering::Less);
        assert_eq!(locale_cmp("a", "A"), Ordering::Less);
        assert_eq!(locale_cmp("x", "x"), Ordering::Equal);
    }
}
