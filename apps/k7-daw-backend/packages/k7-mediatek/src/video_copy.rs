//! video_copy.rs — the copies a plan asks for: ffmpeg's arguments per kind and where each lands (`video-prep/transcode.ts`, `copyPath.ts`).
//! What a reader gets wrong: pure; the spawn and the `.part` rename are the front's, and `host-copy.json` pins every string this spells.

/// What to make: the optimized copy (a stream copy, or a full re-encode) or the preview proxy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyKind {
    Remux,
    Transcode,
    Proxy,
}

/// The proxy's height; its width follows the source's aspect, rounded to an even number.
pub const PROXY_HEIGHT: u32 = 720;

/// `asFileInput`: a path named as a file, so a colon in its name is never read as a protocol.
fn as_file_input(source: &str) -> String {
    if source == "-" || source.starts_with("pipe:") || source.starts_with("file:") {
        source.to_string()
    } else {
        format!("file:{source}")
    }
}

/// `input(source)` of `@studio/ffmpeg`: one input behind the protocol whitelist.
pub fn input(source: &str) -> Vec<String> {
    ["-protocol_whitelist", "file,pipe", "-i"]
        .iter()
        .map(|s| s.to_string())
        .chain([as_file_input(source)])
        .collect()
}

fn strings(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| s.to_string()).collect()
}

/// ffmpeg's arguments for one copy of `source` into `dest`, as `copyArgs` spells them.
pub fn copy_args(kind: CopyKind, source: &str, dest: &str) -> Vec<String> {
    let mut args = strings(&["-v", "error", "-y"]);
    args.extend(input(source));
    args.extend(strings(&["-map", "0:v:0", "-an"]));
    // A keyframe every second, so a scrub lands on a frame without decoding back to a distant one.
    let scrubbable = ["-g", "30", "-keyint_min", "30", "-sc_threshold", "0"];
    let h264 = [
        "-c:v",
        "libx264",
        "-pix_fmt",
        "yuv420p",
        "-movflags",
        "+faststart",
    ];
    match kind {
        CopyKind::Remux => args.extend(strings(&["-c:v", "copy", "-movflags", "+faststart"])),
        CopyKind::Transcode => {
            args.extend(strings(&h264));
            args.extend(strings(&["-preset", "medium", "-crf", "18"]));
            args.extend(strings(&scrubbable));
        }
        CopyKind::Proxy => {
            args.extend(strings(&["-vf", &format!("scale=-2:{PROXY_HEIGHT}")]));
            args.extend(strings(&h264));
            args.extend(strings(&["-preset", "veryfast", "-crf", "26"]));
            args.extend(strings(&scrubbable));
        }
    }
    args.extend(strings(&["-f", "mp4"]));
    args.push(dest.to_string());
    args
}

/// `copyPathOf`: `source` with its last extension replaced by `.<kind>.mp4`; a name with none comes back as it is.
pub fn copy_path_of(source: &str, kind: &str) -> String {
    match source.rfind('.') {
        Some(dot) if dot + 1 < source.len() && !source[dot + 1..].contains('/') => {
            format!("{}.{kind}.mp4", &source[..dot])
        }
        _ => source.to_string(),
    }
}
