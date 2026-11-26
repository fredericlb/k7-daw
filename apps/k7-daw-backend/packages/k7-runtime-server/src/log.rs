//! log.rs — the server's own lines on stderr, through `say!`: whole lines in one write each, tagged `[engine] `
//! when the host asked for it (the iOS app, whose stderr the front shares: `entry::k7_server_tag_log`).
//! What a reader gets wrong: the C++ side's iostreams are tagged by the iOS door itself; this tags Rust's lines only.

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

static TAGGED: AtomicBool = AtomicBool::new(false);

/// From here on every line `say!` writes starts with `[engine] `.
pub fn tag() {
    TAGGED.store(true, Ordering::SeqCst);
}

/// `text` as the line `line` writes, its newline added.
pub fn framed(text: &str, tagged: bool) -> String {
    let mut out = String::with_capacity(text.len() + 10);
    if tagged {
        out.push_str("[engine] ");
    }
    out.push_str(text);
    out.push('\n');
    out
}

/// One line on stderr in a single write, so threads never interleave inside it.
pub fn line(text: &str) {
    let out = framed(text, TAGGED.load(Ordering::SeqCst));
    let _ = std::io::stderr().lock().write_all(out.as_bytes());
}

/// `eprintln!`'s arguments, written by `line`.
macro_rules! say {
    ($($arg:tt)*) => {
        $crate::log::line(&format!($($arg)*))
    };
}

pub(crate) use say;
