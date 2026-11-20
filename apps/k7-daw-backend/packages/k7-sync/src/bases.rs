//! bases.rs — the recent versions of each text that is not k7x, which a late whole text merges against
//! (`TextBases` in `@studio/live-sync`'s `textMerge.ts`); only what the room wrote, so a replay rebuilds it.
//! What a reader gets wrong: sizes are UTF-16 code units, as JavaScript counts a string's length.

use std::cell::OnceCell;

use indexmap::IndexMap;
use serde_json::Value;

use crate::json::{js_keys, utf16_len};

/// How many versions of one file the room keeps as bases, and how many characters in all.
const VERSIONS_KEEP: usize = 20;
const CHARS_KEEP: usize = 1 << 20;

/// The blob ref's opening, which no text file can start with.
pub const BLOB_PREFIX: &str = "\u{0}k7fs-blob ";

/// The blob a text names, `(hash, size)`, or None for an ordinary text (`parseBlobRef`).
pub fn parse_blob_ref(text: &str) -> Option<(&str, f64)> {
    let tail = text.strip_prefix(BLOB_PREFIX)?.strip_suffix('\n')?;
    let (hash, size) = tail.split_once(' ')?;
    let hex = hash.len() == 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    let digits = !size.is_empty() && size.bytes().all(|b| b.is_ascii_digit());
    (hex && digits).then(|| (hash, size.parse::<f64>().unwrap_or(f64::INFINITY)))
}

/// `/\.k7x$/i`.
pub fn is_k7x(file: &str) -> bool {
    let b = file.as_bytes();
    b.len() >= 4 && b[b.len() - 4..].eq_ignore_ascii_case(b".k7x")
}

/// A k7x file merges by path ops, and a blob ref is no text: neither is kept or merged as lines.
pub fn mergeable(file: &str, text: Option<&str>) -> bool {
    text.is_some_and(|t| !is_k7x(file) && parse_blob_ref(t).is_none())
}

/// One kept version, its length and hash worked out once.
#[derive(Debug, Clone)]
pub struct Version {
    text: String,
    len: usize,
    hash: OnceCell<String>,
}

impl Version {
    fn new(text: String) -> Self {
        Version {
            len: utf16_len(&text),
            text,
            hash: OnceCell::new(),
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct TextBases {
    by_file: IndexMap<String, Vec<Version>>,
}

impl TextBases {
    /// Bases as `state()` saved them.
    pub fn from_saved(saved: Option<&Value>) -> Self {
        let mut bases = TextBases::default();
        if let Some(Value::Object(m)) = saved {
            for f in js_keys(m) {
                let texts = m[f]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str);
                bases.by_file.insert(
                    f.clone(),
                    texts.map(|t| Version::new(t.to_string())).collect(),
                );
            }
        }
        bases
    }

    /// Each file's versions, in the order they were first kept.
    pub fn iter(&self) -> impl Iterator<Item = (&String, impl Iterator<Item = &str>)> {
        self.by_file
            .iter()
            .map(|(f, v)| (f, v.iter().map(|x| x.text.as_str())))
    }

    /// Follow a write: a removed or unmergeable file keeps nothing, any other text joins its versions.
    pub fn record(&mut self, file: &str, text: Option<&str>) {
        let Some(text) = text.filter(|t| mergeable(file, Some(t))) else {
            self.by_file.shift_remove(file);
            return;
        };
        let v = Version::new(text.to_string());
        if v.len > CHARS_KEEP {
            self.by_file.shift_remove(file);
            return;
        }
        let kept = self.by_file.entry(file.to_string()).or_default();
        if kept.last().is_some_and(|l| l.text == text) {
            return;
        }
        kept.push(v);
        let mut size: usize = kept.iter().map(|x| x.len).sum();
        while kept.len() > VERSIONS_KEEP || size > CHARS_KEEP {
            size -= kept.remove(0).len;
        }
    }

    /// A copy of the versions kept for `file`, oldest first.
    pub fn versions(&self, file: &str) -> Option<Vec<String>> {
        self.by_file
            .get(file)
            .map(|v| v.iter().map(|x| x.text.clone()).collect())
    }

    /// What `snapshot` took, put back for the undo of a write that failed.
    pub fn snapshot(&self, file: &str) -> Option<Vec<Version>> {
        self.by_file.get(file).cloned()
    }

    pub fn restore(&mut self, file: &str, kept: Option<Vec<Version>>) {
        match kept {
            Some(k) => {
                self.by_file.insert(file.to_string(), k);
            }
            None => {
                self.by_file.shift_remove(file);
            }
        }
    }

    /// The kept version of `file` whose hash is `hash`.
    pub fn find(&self, file: &str, hash: &str) -> Option<&str> {
        let kept = self.by_file.get(file)?;
        kept.iter()
            .rev()
            .find(|v| v.hash.get_or_init(|| k7_js::number::text_hash(&v.text)) == hash)
            .map(|v| v.text.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_twenty_versions_of_mergeable_texts() {
        let mut b = TextBases::default();
        for i in 0..25 {
            b.record("a.md", Some(&format!("{i}\n")));
        }
        b.record("a.md", Some("24\n"));
        let v = b.versions("a.md").unwrap();
        assert_eq!(v.len(), 20);
        assert_eq!(v[0], "5\n");
        assert_eq!(
            b.find("a.md", &k7_js::number::text_hash("7\n")),
            Some("7\n")
        );
        b.record("s.K7X", Some("x"));
        assert!(b.versions("s.K7X").is_none());
        b.record("a.md", None);
        assert!(b.versions("a.md").is_none());
        let blob = format!("{BLOB_PREFIX}{} 3\n", "a".repeat(64));
        assert_eq!(parse_blob_ref(&blob), Some((&"a".repeat(64)[..], 3.0)));
        b.record("img.png", Some(&blob));
        assert!(b.versions("img.png").is_none());
    }

    #[test]
    fn a_text_past_the_cap_keeps_nothing() {
        let mut b = TextBases::default();
        b.record("a.md", Some("x"));
        b.record("a.md", Some(&"é".repeat(CHARS_KEEP + 1)));
        assert!(b.versions("a.md").is_none());
    }
}
