// format_level.rs — the project FORMAT LEVEL as song-schema's formatLevel.ts states it: constants and the level read off raw text.
// A reader would get wrong that the level is read off the RAW text, before any parse: a newer file may use syntax this build cannot parse.

pub const FORMAT_LEVEL: u32 = 5;
pub const SECONDS_START_LEVEL: u32 = 2;
pub const PICTURE_LANE_LEVEL: u32 = 3;
pub const PICTURE_SOUND_LEVEL: u32 = 4;
pub const RENDER_AUDIO_LEVEL: u32 = 5;
pub const FORMAT_KEY: &str = "format";

/// JS `\s`: the whitespace set of a regex, which holds U+FEFF and U+00A0 and not U+0085.
#[must_use]
pub fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n'
            | '\u{b}'
            | '\u{c}'
            | '\r'
            | ' '
            | '\u{a0}'
            | '\u{1680}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202f}'
            | '\u{205f}'
            | '\u{3000}'
            | '\u{feff}'
    ) || ('\u{2000}'..='\u{200a}').contains(&c)
}

/// Whether the text at the start of `line` opens a top-level `format(N)`; the digits when it does.
fn level_at(line: &str) -> Option<&str> {
    let mut rest = line.strip_prefix(['"', '\'']).unwrap_or(line);
    rest = rest.strip_prefix("format")?;
    rest = rest.strip_prefix(['"', '\'']).unwrap_or(rest);
    rest = rest.trim_start_matches(is_js_space).strip_prefix('(')?;
    rest = rest.trim_start_matches(is_js_space);
    let n = rest.bytes().take_while(u8::is_ascii_digit).count();
    (n > 0).then(|| &rest[..n])
}

/// `^` of a multiline regex: the start of the text and the char after a line end (`\n`, `\r`, U+2028, U+2029).
fn line_starts(text: &str) -> impl Iterator<Item = (usize, &str)> {
    let starts = std::iter::once(0).chain(
        text.char_indices()
            .filter(|(_, c)| matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}'))
            .map(|(i, c)| i + c.len_utf8()),
    );
    starts.map(|i| (i, &text[i..]))
}

/// The level a root file's raw text declares, or None when it declares none.
#[must_use]
pub fn format_level_of_text(text: &str) -> Option<u32> {
    line_starts(text)
        .find_map(|(_, line)| level_at(line))
        .map(|d| d.parse::<u32>().unwrap_or(u32::MAX))
}

/// The 1-based line the level is declared on, or None.
#[must_use]
pub fn format_line_of(text: &str) -> Option<usize> {
    let (at, _) = line_starts(text).find(|(_, line)| level_at(line).is_some())?;
    Some(text[..at].split('\n').count())
}
