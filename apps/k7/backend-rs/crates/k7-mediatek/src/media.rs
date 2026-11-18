//! media.rs — which names are audio and which video, the FLAC's derived copy beside it, and a folder's master,
//! to the letter of what node's `media.ts` decided before it went.
//! What a reader gets wrong: an extension is the last dot's, lowercased, and a name that STARTS with its only dot has none.

/// The extensions the scan lists as audio.
pub const AUDIO_EXT: [&str; 9] = [
    ".wav", ".flac", ".aiff", ".aif", ".mp3", ".ogg", ".opus", ".m4a", ".webm",
];
/// …and as video.
pub const VIDEO_EXT: [&str; 5] = [".mp4", ".mov", ".mkv", ".webm", ".avi"];

/// The copy the library derives beside a FLAC, and the MP3 a volume may still hold from before.
const DERIVED: &str = ".webm";
const LEGACY: &str = ".mp3";

/// `extOf`: the extension, lowercased, with its dot, or "".
pub fn ext_of(name: &str) -> String {
    match name.rfind('.') {
        Some(i) if i > 0 => name[i..].to_lowercase(),
        _ => String::new(),
    }
}

pub fn is_audio(name: &str) -> bool {
    AUDIO_EXT.contains(&ext_of(name).as_str())
}

pub fn is_video(name: &str) -> bool {
    VIDEO_EXT.contains(&ext_of(name).as_str())
}

/// The name without its extension; called only where the extension is ASCII, so it is `extOf`'s cut.
fn stem(name: &str) -> &str {
    match name.rfind('.') {
        Some(i) if i > 0 => &name[..i],
        _ => name,
    }
}

/// `derivedOf`: the stream copy the library derives beside a FLAC, same stem, or None for anything else.
pub fn derived_of(name: &str) -> Option<String> {
    (ext_of(name) == ".flac").then(|| format!("{}{DERIVED}", &name[..name.len() - ".flac".len()]))
}

/// `MIME_BY_EXT`: enough content type for a browser to play or show what the library holds (`docs/mime.md`).
const MIME_BY_EXT: [(&str, &str); 22] = [
    (".wav", "audio/wav"),
    (".flac", "audio/flac"),
    (".aiff", "audio/aiff"),
    (".aif", "audio/aiff"),
    (".mp3", "audio/mpeg"),
    (".ogg", "audio/ogg"),
    (".opus", "audio/ogg"),
    (".m4a", "audio/mp4"),
    (".mp4", "video/mp4"),
    (".mov", "video/quicktime"),
    (".mkv", "video/x-matroska"),
    (".webm", "video/webm"),
    (".avi", "video/x-msvideo"),
    (".yaml", "text/yaml; charset=utf-8"),
    (".yml", "text/yaml; charset=utf-8"),
    (".json", "application/json"),
    (".nam", "application/json"),
    (".txt", "text/plain; charset=utf-8"),
    (".png", "image/png"),
    (".jpg", "image/jpeg"),
    (".jpeg", "image/jpeg"),
    (".webp", "image/webp"),
];

/// `mimeFor`: what to serve a path as, by its LAST segment's extension, so a dotted folder is not one.
pub fn mime_for(path: &str) -> &'static str {
    let last = path.rsplit(['/', '\\']).next().unwrap_or("");
    let ext = ext_of(last);
    MIME_BY_EXT
        .iter()
        .find(|(e, _)| *e == ext)
        .map_or("application/octet-stream", |(_, m)| m)
}

/// `withoutDerived`: a folder's names with every FLAC's derived copy taken out, order kept.
pub fn without_derived<T>(entries: Vec<T>, name: impl Fn(&T) -> &str) -> Vec<T> {
    let derived = |n: &str| {
        let e = ext_of(n);
        e == DERIVED || e == LEGACY
    };
    if !entries.iter().any(|e| derived(name(e))) {
        return entries;
    }
    let lower: std::collections::HashSet<String> =
        entries.iter().map(|e| name(e).to_lowercase()).collect();
    entries
        .into_iter()
        .filter(|e| {
            let n = name(e);
            let ext = ext_of(n);
            !((ext == DERIVED || ext == LEGACY)
                && lower.contains(&format!("{}.flac", stem(n)).to_lowercase()))
        })
        .collect()
}

/// `masterOf`: `original.*`, else the mirror's `master.*`, else the first audio file by name (code units).
pub fn master_of<'a>(files: &[&'a str]) -> Option<&'a str> {
    let mut audio: Vec<&str> = files.iter().copied().filter(|f| is_audio(f)).collect();
    audio.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    let stem_of = |f: &str| stem(f).to_lowercase();
    audio
        .iter()
        .find(|f| stem_of(f) == "original")
        .or_else(|| audio.iter().find(|f| stem_of(f) == "master"))
        .or_else(|| audio.first())
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_an_extension_as_media_ts() {
        assert_eq!(ext_of("a.MP3"), ".mp3");
        assert_eq!(ext_of(".flac"), "");
        assert_eq!(ext_of("noext"), "");
        assert_eq!(ext_of("a.b.Wav"), ".wav");
        assert!(is_audio("x.webm") && is_video("x.webm"));
        assert!(!is_audio("cover.jpg"));
    }

    #[test]
    fn names_a_copy_and_a_type_as_media_ts() {
        assert_eq!(derived_of("a/Take.FLAC").as_deref(), Some("a/Take.webm"));
        assert_eq!(derived_of("take.wav"), None);
        assert_eq!(mime_for("songs/a.b/c.MP3"), "audio/mpeg");
        assert_eq!(mime_for("songs/x.flac/notes"), "application/octet-stream");
        assert_eq!(mime_for("C:\\x\\n.txt"), "text/plain; charset=utf-8");
        assert_eq!(mime_for(".nam"), "application/octet-stream");
        assert_eq!(mime_for("amp.nam"), "application/json");
    }

    #[test]
    fn takes_out_a_flacs_derived_copies_only() {
        let names = vec!["Song.FLAC", "song.webm", "song.mp3", "other.mp3", "x.webm"];
        assert_eq!(
            without_derived(names, |n| n),
            vec!["Song.FLAC", "other.mp3", "x.webm"]
        );
    }

    #[test]
    fn finds_the_master_in_its_order() {
        assert_eq!(
            master_of(&["b.wav", "Master.flac", "Original.mp3"]),
            Some("Original.mp3")
        );
        assert_eq!(
            master_of(&["b.wav", "master.flac", "a.ogg"]),
            Some("master.flac")
        );
        assert_eq!(
            master_of(&["b.wav", "a.ogg", "Z.wav", "song.yaml"]),
            Some("Z.wav")
        );
        assert_eq!(master_of(&["song.yaml"]), None);
    }
}
