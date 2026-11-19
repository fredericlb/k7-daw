//! file.rs — a text file's state, read once: its text, absent, or present and unreadable with the errno name.

use std::io;

/// A text file's state: its text, absent, or present and unreadable with the errno name.
pub enum Read {
    Text(String),
    Absent,
    Error(String),
}

pub fn read_text(file: &str) -> Read {
    match std::fs::read(file) {
        Ok(b) => Read::Text(String::from_utf8_lossy(&b).into_owned()),
        Err(e) => match e.kind() {
            io::ErrorKind::NotFound | io::ErrorKind::NotADirectory => Read::Absent,
            _ => Read::Error(crate::filter::code_of(&e).to_owned()),
        },
    }
}
