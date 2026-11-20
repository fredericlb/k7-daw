//! The account's runtime, as a model with no I/O and no clock of its own: the Logs ring, `/logs`' numbers and answer,
//! the process table, the background registry and the stop's plan (sub-step 1n, docs/file-runtime.md).
//! What a reader gets wrong: every time is an argument, so a test turns the clock; ids and cursors are the model's own.

use std::time::Duration;

pub mod background;
pub mod list;
pub mod logs;
pub mod ring;
pub mod stop;

/// Lines the ring keeps.
pub const LINES: usize = 1000;
/// Bytes of one line's text; a longer one is cut and ends ` …` (2620).
pub const TEXT: usize = 16 * 1024;
/// How long a command runs before it is listed.
pub const SHOW_AFTER: Duration = Duration::from_secs(1);
/// How long an ended process stays listed.
pub const LINGER: Duration = Duration::from_secs(8);
/// How long a registered job holds the runtime without being heard from (2631).
pub const LEASE: Duration = Duration::from_secs(10 * 60);
/// The front's stop, in all (2670).
pub const STOP: Duration = Duration::from_secs(5);

/// JavaScript's `\s` and its string trim: the same set, which is wider than Rust's `char::is_whitespace` at U+FEFF.
pub fn is_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}
