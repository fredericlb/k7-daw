//! attributes.rs — the `.gitattributes` lines k7 writes to send large files through LFS, and the reading of them.
//! What a reader gets wrong: only the root `.gitattributes` is read, and the last line that names `filter` for a
//! path decides, as git reads it; k7's block is rewritten between its markers, the person's other lines are kept.

use std::path::Path;

use crate::Result;

const BEGIN: &str = "# k7 begin (rewritten by k7)";
const END: &str = "# k7 end";
const LINE: &str = "filter=lfs diff=lfs merge=lfs -text";

/// The extensions k7 always tracks: audio, video and archives.
pub const TRACKED: &[&str] = &[
    "wav", "aif", "aiff", "flac", "mp3", "ogg", "opus", "m4a", "caf", "mp4", "mov", "webm", "zip",
    "k7x-pack",
];

/// A file over this size and binary gets its own line.
pub const BIG: u64 = 1_000_000;

fn block() -> String {
    let lines: String = TRACKED.iter().map(|e| format!("*.{e} {LINE}\n")).collect();
    format!("{BEGIN}\n{lines}{END}\n")
}

/// `existing` with k7's block replaced where it is, or put first.
pub fn with_block(existing: &str) -> String {
    match (existing.find(BEGIN), existing.find(END)) {
        (Some(a), Some(b)) if a < b => {
            let tail = existing[b + END.len()..].trim_start_matches('\n');
            format!("{}{}{tail}", &existing[..a], block())
        }
        _ if existing.is_empty() => block(),
        _ if existing.ends_with('\n') => format!("{existing}{}", block()),
        _ => format!("{existing}\n{}", block()),
    }
}

/// k7's block written into the workspace's `.gitattributes`, the rest of the file kept.
pub fn ensure(workdir: &Path) -> Result<()> {
    let file = workdir.join(".gitattributes");
    let old = std::fs::read_to_string(&file).unwrap_or_default();
    let new = with_block(&old);
    if new != old {
        std::fs::write(file, new)?;
    }
    Ok(())
}

/// The path as a pattern: quoted, with glob characters escaped.
fn pattern_of(rel: &str) -> String {
    let mut out = String::from("\"");
    for c in rel.chars() {
        if matches!(c, '*' | '?' | '[' | '\\' | '"') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

/// Adds a line for one path, once.
pub fn add_path(workdir: &Path, rel: &str) -> Result<()> {
    let file = workdir.join(".gitattributes");
    let mut text = std::fs::read_to_string(&file).unwrap_or_default();
    let line = format!("{} {LINE}\n", pattern_of(rel));
    if text.contains(&line) {
        return Ok(());
    }
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&line);
    std::fs::write(file, text)?;
    Ok(())
}

/// A glob as git's attributes read it: `*` and `?` stop at `/`, `**` does not.
fn glob(p: &[u8], t: &[u8]) -> bool {
    match p.split_first() {
        None => t.is_empty(),
        Some((b'*', rest)) if rest.first() == Some(&b'*') => {
            let rest = &rest[1..];
            let rest = rest.strip_prefix(b"/").unwrap_or(rest);
            (0..=t.len()).any(|i| glob(rest, &t[i..]))
        }
        Some((b'*', rest)) => (0..=t.len())
            .take_while(|i| *i == 0 || t[*i - 1] != b'/')
            .any(|i| glob(rest, &t[i..])),
        Some((b'?', rest)) => !t.is_empty() && t[0] != b'/' && glob(rest, &t[1..]),
        Some((b'\\', rest)) if !rest.is_empty() => {
            !t.is_empty() && t[0] == rest[0] && glob(&rest[1..], &t[1..])
        }
        Some((c, rest)) => !t.is_empty() && t[0] == *c && glob(rest, &t[1..]),
    }
}

/// The root `.gitattributes`, read for whether a path goes through LFS.
#[derive(Clone, Debug, Default)]
pub struct Attributes {
    lines: Vec<(String, bool)>,
}

impl Attributes {
    pub fn parse(text: &str) -> Attributes {
        let mut lines = Vec::new();
        for raw in text.lines() {
            let raw = raw.trim();
            if raw.is_empty() || raw.starts_with('#') {
                continue;
            }
            let (pattern, rest) = if let Some(q) = raw.strip_prefix('"') {
                let mut p = String::new();
                let mut chars = q.char_indices();
                let mut end = q.len();
                while let Some((i, c)) = chars.next() {
                    match c {
                        '\\' => {
                            if let Some((_, n)) = chars.next() {
                                p.push('\\');
                                p.push(n);
                            }
                        }
                        '"' => {
                            end = i + 1;
                            break;
                        }
                        c => p.push(c),
                    }
                }
                (p, &q[end..])
            } else {
                match raw.split_once(char::is_whitespace) {
                    Some((p, r)) => (p.to_string(), r),
                    None => continue,
                }
            };
            for attr in rest.split_whitespace() {
                match attr {
                    "filter=lfs" => lines.push((pattern.clone(), true)),
                    "-filter" | "!filter" => lines.push((pattern.clone(), false)),
                    a if a.starts_with("filter=") => lines.push((pattern.clone(), false)),
                    _ => {}
                }
            }
        }
        Attributes { lines }
    }

    pub fn read(workdir: &Path) -> Attributes {
        Attributes::parse(
            &std::fs::read_to_string(workdir.join(".gitattributes")).unwrap_or_default(),
        )
    }

    /// Whether `rel` goes through LFS: the last line that names `filter` and matches it.
    pub fn tracks(&self, rel: &str) -> bool {
        let base = rel.rsplit('/').next().unwrap_or(rel);
        let mut on = false;
        for (pattern, value) in &self.lines {
            let hit = match pattern.strip_prefix('/') {
                Some(anchored) => glob(anchored.as_bytes(), rel.as_bytes()),
                None if pattern.contains('/') => glob(pattern.as_bytes(), rel.as_bytes()),
                None => glob(pattern.as_bytes(), base.as_bytes()),
            };
            if hit {
                on = *value;
            }
        }
        on
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn k7s_block_tracks_audio_video_and_archives_at_any_depth() {
        let a = Attributes::parse(&with_block(""));
        for p in [
            "a.wav",
            "deep/er/take.flac",
            "x.k7x-pack",
            "m.mp4",
            "z.zip",
            "C.m4a",
        ] {
            assert!(a.tracks(p), "{p}");
        }
        for p in ["song.k7x", "notes.txt", "wav", "a.wav.txt", "readme.md"] {
            assert!(!a.tracks(p), "{p}");
        }
    }

    #[test]
    fn the_last_matching_line_wins_and_a_person_can_untrack() {
        let a = Attributes::parse(
            "*.wav filter=lfs diff=lfs merge=lfs -text\nscratch/*.wav -filter\n/keep.bin filter=lfs\n",
        );
        assert!(a.tracks("take.wav"));
        assert!(!a.tracks("scratch/take.wav"));
        assert!(a.tracks("keep.bin"));
        assert!(
            !a.tracks("sub/keep.bin"),
            "a leading slash anchors at the root"
        );
        assert!(!Attributes::parse("*.wav filter=other\n").tracks("a.wav"));
    }

    #[test]
    fn a_path_line_is_quoted_escaped_and_added_once() {
        let d = std::env::temp_dir().join(format!("k7-attr-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        ensure(&d).unwrap();
        add_path(&d, "big files/take [1].bin").unwrap();
        add_path(&d, "big files/take [1].bin").unwrap();
        let text = std::fs::read_to_string(d.join(".gitattributes")).unwrap();
        assert_eq!(text.matches("take").count(), 1);
        let a = Attributes::parse(&text);
        assert!(a.tracks("big files/take [1].bin"));
        assert!(!a.tracks("big files/take 1.bin"));
        let again = with_block(&text);
        assert_eq!(
            again, text,
            "the block is rewritten in place and the person's lines stay"
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}
