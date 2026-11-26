// playlist.rs — a workspace playlist read: where its file lives, and its ordered songs with their optional variant.
// A reader would get wrong that an unreadable text is `None` while a readable one with no items is an empty playlist.

use crate::js_text::js_trim;
use k7x::edit::{Value, parse};

/// The folder a workspace keeps its playlists in, beside `_modules`.
pub const PLAYLISTS_DIR: &str = "_playlists";

/// What a playlist file is called at its end.
pub const PLAYLIST_EXT: &str = ".k7x";

/// One line of a playlist; `song` is kept verbatim, since resolving it needs the library's listing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub song: String,
    pub variant: Option<String>,
}

/// A parsed playlist: its title when it has one, and its lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Playlist {
    pub title: Option<String>,
    pub items: Vec<Item>,
}

/// A playlist NAME as the workspace path holding it, None for one that would leave the folder.
#[must_use]
pub fn playlist_path_of(name: &str) -> Option<String> {
    let clean = js_trim(name).trim_matches('/');
    if clean.is_empty() || clean.split('/').any(|s| s.is_empty() || s == "." || s == "..") {
        return None;
    }
    let ends = clean.len() >= 4
        && clean.is_char_boundary(clean.len() - 4)
        && clean[clean.len() - 4..].eq_ignore_ascii_case(PLAYLIST_EXT);
    Some(format!(
        "{PLAYLISTS_DIR}/{clean}{}",
        if ends { "" } else { PLAYLIST_EXT }
    ))
}

/// A string value, trimmed, when it says something.
fn text_of(v: Option<&Value>) -> Option<String> {
    match v {
        Some(Value::Str(s)) if !js_trim(s).is_empty() => Some(js_trim(s).to_owned()),
        _ => None,
    }
}

/// A playlist's k7x text, or None when it does not read as a map: an unreadable file is not an empty one.
#[must_use]
pub fn parse_playlist(text: &str) -> Option<Playlist> {
    let doc = parse(text).ok()?;
    if !matches!(doc, Value::Map(_)) {
        return None;
    }
    let items = match doc.get("items") {
        Some(Value::List(lines)) => lines
            .iter()
            .filter_map(|l| match text_of(Some(l)) {
                Some(song) => Some(Item { song, variant: None }),
                None => text_of(l.get("song")).map(|song| Item {
                    song,
                    variant: text_of(l.get("variant")),
                }),
            })
            .collect(),
        _ => Vec::new(),
    };
    Some(Playlist {
        title: text_of(doc.get("title")),
        items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_a_path_under_the_folder_or_nothing() {
        assert_eq!(playlist_path_of(" Friday ").as_deref(), Some("_playlists/Friday.k7x"));
        assert_eq!(playlist_path_of("/a/b.K7X/").as_deref(), Some("_playlists/a/b.K7X"));
        assert_eq!(playlist_path_of("../x"), None);
        assert_eq!(playlist_path_of("a//b"), None);
        assert_eq!(playlist_path_of("  "), None);
    }

    #[test]
    fn items_are_strings_or_maps_and_the_rest_is_skipped() {
        let p = parse_playlist("title(\"Set\")\nitems([\"One\", song(\"Two\").variant(\"live\"), 3, song(\"\")])\n")
            .expect("reads");
        assert_eq!(p.title.as_deref(), Some("Set"));
        let songs: Vec<_> = p
            .items
            .iter()
            .map(|i| (i.song.as_str(), i.variant.as_deref()))
            .collect();
        assert_eq!(songs, [("One", None), ("Two", Some("live"))]);
        assert_eq!(parse_playlist("items([\n"), None);
        assert_eq!(parse_playlist("title(\"x\")").map(|p| p.items.len()), Some(0));
    }
}
