// lines.rs — command lines two surfaces spell alike: running a library chain over a clip, and what a set list's file may say.
// A reader would get wrong that `clip` is a file's path: it is the clip's ref, that stem being the only name the verb knows.

use song_edit::js_text::js_trim;

/// The folder a separation writes its stems into, beside the file.
const STEMS_DIR: &str = "stems";

/// Filtered on the whole ref: a chain is filed by path, and an empty search is every chain rather than none.
#[must_use]
pub fn filter_ops(ops: &[String], search: &str) -> Vec<String> {
    let q = js_trim(search).to_lowercase();
    if q.is_empty() {
        return ops.to_vec();
    }
    ops.iter().filter(|o| o.to_lowercase().contains(&q)).cloned().collect()
}

/// The command line that runs a library chain over one clip.
#[must_use]
pub fn process_line(clip: &str, chain: &str) -> String {
    format!("clip process {clip} {chain}")
}

/// A line as the workspace path of its record, its slashes trimmed: none for one that leaves the workspace or names a library.
#[must_use]
pub fn record_path_of(line: &str) -> Option<String> {
    let clean = js_trim(line).trim_matches('/');
    if clean.is_empty() || clean.contains('\\') || clean.starts_with('@') {
        return None;
    }
    clean
        .split('/')
        .all(|s| !s.is_empty() && s != "." && s != "..")
        .then(|| clean.to_owned())
}

/// The records a line may name: every audio file but a `stems/` folder's, sorted.
#[must_use]
pub fn playlist_refs(files: &[String]) -> Vec<String> {
    let mut refs: Vec<String> = files
        .iter()
        .filter(|f| {
            record_path_of(f).as_deref() == Some(f.as_str()) && !f.split('/').rev().skip(1).any(|d| d == STEMS_DIR)
        })
        .cloned()
        .collect();
    // UTF-16 code unit order: the order a default sort gives a list of paths.
    refs.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    refs
}

/// The fields a set list's file may carry at `path`: the file's own two at the top, and the item's one under `items`.
#[must_use]
pub fn playlist_fields_at(path: &[String]) -> Vec<String> {
    if path.is_empty() {
        return vec!["title".to_owned(), "items".to_owned()];
    }
    // A `- ` under `items:` may be a mapping, so this is offered beside the refs.
    if path.len() == 1 && path[0] == "items" {
        return vec!["song".to_owned()];
    }
    vec![]
}

/// The values a set list's field may take: the records, for the bare item and for `song:`.
#[must_use]
pub fn playlist_values_at(refs: &[String], path: &[String], field: &str) -> Vec<String> {
    // The bare form, `- <ref>`: the value belongs to the key that introduced the list.
    if path.is_empty() && field == "items" {
        return refs.to_vec();
    }
    if path.len() == 1 && path[0] == "items" && field == "song" {
        return refs.to_vec();
    }
    vec![]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn a_search_filters_on_the_whole_ref_and_an_empty_one_keeps_every_chain() {
        let ops = strings(&["@k7-core/fx/reverb", "@k7-core/fx/delay", "@mine/a"]);
        assert_eq!(filter_ops(&ops, "  "), ops);
        assert_eq!(filter_ops(&ops, "REV"), strings(&["@k7-core/fx/reverb"]));
        assert_eq!(
            filter_ops(&ops, "fx/"),
            strings(&["@k7-core/fx/reverb", "@k7-core/fx/delay"])
        );
        assert!(filter_ops(&ops, "zzz").is_empty());
    }

    #[test]
    fn the_line_names_the_clip_by_its_ref_then_the_chain() {
        assert_eq!(
            process_line("kick", "@k7-core/fx/comp"),
            "clip process kick @k7-core/fx/comp"
        );
    }

    const FILES: [&str; 3] = [
        "records/Nirvana/Lithium.flac",
        "records/AC-DC/Hells Bells.flac",
        "records/AC-DC/stems/drums.wav",
    ];

    #[test]
    fn a_record_is_a_plain_workspace_path_and_never_a_stem() {
        let refs = playlist_refs(&strings(&FILES));
        assert_eq!(
            refs,
            strings(&["records/AC-DC/Hells Bells.flac", "records/Nirvana/Lithium.flac"])
        );
        assert_eq!(record_path_of(" /a/b/ ").as_deref(), Some("a/b"));
        assert_eq!(record_path_of("@lib/x"), None);
        assert_eq!(record_path_of("a/../b"), None);
        assert_eq!(record_path_of("a\\b"), None);
        assert_eq!(record_path_of("//"), None);
        // A path with a slash at its end is not its own plain form.
        assert!(playlist_refs(&strings(&["a/b/", "/a"])).is_empty());
    }

    #[test]
    fn the_fields_and_values_follow_where_the_cursor_is() {
        let refs = playlist_refs(&strings(&FILES));
        let p = |v: &[&str]| strings(v);
        assert_eq!(playlist_fields_at(&p(&[])), strings(&["title", "items"]));
        assert_eq!(playlist_fields_at(&p(&["items"])), strings(&["song"]));
        assert!(playlist_fields_at(&p(&["items", "song"])).is_empty());
        assert_eq!(playlist_values_at(&refs, &p(&[]), "items"), refs);
        assert!(playlist_values_at(&refs, &p(&["items"]), "song").contains(&"records/Nirvana/Lithium.flac".to_owned()));
        assert!(playlist_values_at(&refs, &p(&[]), "title").is_empty());
        assert!(playlist_values_at(&refs, &p(&["items"]), "colour").is_empty());
        // Before the listing arrives there is nothing to offer, and the file's shape is known all the same.
        assert!(playlist_values_at(&[], &p(&[]), "items").is_empty());
    }
}
