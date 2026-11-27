// audio_files.rs — which extensions are audio: one list, since a dropped file, the verbs' media members and a set list judge by it.
// A reader would get wrong that `.webm` is audio here though video names it too: a name alone cannot say what a container holds.

const AUDIO_EXT: [&str; 9] = [
    ".wav", ".flac", ".aiff", ".aif", ".mp3", ".ogg", ".opus", ".m4a", ".webm",
];

/// The extension, lowercased and with its dot, or "": a leading dot is a whole name, not one.
#[must_use]
pub fn ext_of(name: &str) -> String {
    match name.rfind('.') {
        Some(i) if i > 0 => name[i..].to_lowercase(),
        _ => String::new(),
    }
}

/// Is this file name an audio file, by its extension.
#[must_use]
pub fn is_audio(name: &str) -> bool {
    AUDIO_EXT.contains(&ext_of(name).as_str())
}
