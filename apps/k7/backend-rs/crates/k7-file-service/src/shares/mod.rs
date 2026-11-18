//! The export family's model (sub-step 1m, M1): the mixdown's names and the picture's job, as pure functions the
//! front's I/O (M2) calls.
//! What a reader gets wrong: every sentence here is the TypeScript's; the vectors and `docs/file-shares.md` hold the proof.

pub mod names;
pub mod picture;
pub mod qs;

/// An error body is read up to this much into a sentence (2535).
pub const ERROR_BODY_CAP: usize = 64 * 1024;

/// What a render says when this machine has no engine (`requireRenderer`).
pub fn no_engine(what: &str) -> String {
    format!("no engine to render with — cannot render the {what}")
}

/// What an MP3 export says when no ffmpeg is on the host (2523).
pub const MP3_NEEDS_FFMPEG: &str =
    "mp3 needs ffmpeg on the engine host — install ffmpeg, or use Export WAV";

/// An error body cut to `ERROR_BODY_CAP` bytes, at a character (2535).
pub fn capped(body: &str) -> &str {
    let mut end = body.len().min(ERROR_BODY_CAP);
    while !body.is_char_boundary(end) {
        end -= 1;
    }
    &body[..end]
}

/// `<what>: <the body's `error`, else the body>`, the body capped (2535).
pub fn said(what: &str, body: &str) -> String {
    let body = capped(body);
    let error = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("error").and_then(|e| e.as_str().map(str::to_owned)));
    format!("{what}: {}", error.as_deref().unwrap_or(body))
}
