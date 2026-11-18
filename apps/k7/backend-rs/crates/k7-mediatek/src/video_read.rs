//! video_read.rs — what a video file IS, read from its own headers where there is no ffprobe (the phone): ISO by `mp4parse`, Matroska by `matroska-demuxer`.
//! What a reader gets wrong: the container is sniffed by magic bytes, never by extension, and every read is bounded
//! (a box or element claiming more than the file holds is refused, never allocated); see docs/video-read.md.

use std::io::{Cursor, Read, Seek, SeekFrom};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use mp4parse::TrackType;
use serde_json::{Map, Value, json};

use crate::video_probe::{json_number, rotation_of};

/// The file is neither ISO nor Matroska, or its headers are damaged: nothing can be said about it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unsupported;

/// The code an answer carries when the reader cannot say.
pub const UNSUPPORTED_CODE: &str = "unsupported";

const ISO_CONTAINER: &str = "mov,mp4,m4a,3gp,3g2,mj2";
pub(crate) const MATROSKA_CONTAINER: &str = "matroska,webm";
/// The largest `moov` read into memory; a camera's index for hours of film is a few megabytes.
const MOOV_CAP: u64 = 64 * 1024 * 1024;
/// Top-level boxes walked before giving up on finding `moov`.
const MAX_TOP_BOXES: usize = 4096;
/// A sample entry's fixed fields before its child boxes.
const VISUAL_ENTRY_FIXED: usize = 78;

/// The facts of `path`'s first video stream as `facts_of` states them; `None` for a file with none.
pub fn read_facts(path: &Path) -> Result<Option<Value>, Unsupported> {
    let file = std::fs::File::open(path).map_err(|_| Unsupported)?;
    let len = file.metadata().map_err(|_| Unsupported)?.len();
    read_facts_from(file, len)
}

/// `read_facts` over any seekable source of `len` bytes; a panic inside a parser is an `Unsupported`.
pub fn read_facts_from<R: Read + Seek>(mut r: R, len: u64) -> Result<Option<Value>, Unsupported> {
    let mut head = [0u8; 12];
    let got = read_at(&mut r, 0, &mut head).map_err(|_| Unsupported)?;
    let head = &head[..got];
    let done = catch_unwind(AssertUnwindSafe(|| {
        if head.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
            crate::video_read_mkv::read_matroska(r)
        } else if head.len() >= 8 && is_iso_start(&head[4..8]) {
            read_iso(&mut r, len)
        } else {
            Err(Unsupported)
        }
    }));
    done.unwrap_or(Err(Unsupported))
}

/// The first box of an ISO file: `ftyp`, or one of the boxes an old QuickTime file opens with.
fn is_iso_start(name: &[u8]) -> bool {
    [
        &b"ftyp"[..],
        b"moov",
        b"mdat",
        b"free",
        b"skip",
        b"wide",
        b"pnot",
    ]
    .contains(&name)
}

fn read_at<R: Read + Seek>(r: &mut R, at: u64, buf: &mut [u8]) -> std::io::Result<usize> {
    r.seek(SeekFrom::Start(at))?;
    let mut got = 0;
    while got < buf.len() {
        match r.read(&mut buf[got..])? {
            0 => break,
            n => got += n,
        }
    }
    Ok(got)
}

// ---- ISO base media ----

/// The `moov` box of the file, whole, found by hopping over the top-level boxes without reading them.
fn find_moov<R: Read + Seek>(r: &mut R, len: u64) -> Result<Vec<u8>, Unsupported> {
    let mut pos = 0u64;
    for _ in 0..MAX_TOP_BOXES {
        if pos.saturating_add(8) > len {
            break;
        }
        let mut h = [0u8; 16];
        let got = read_at(r, pos, &mut h).map_err(|_| Unsupported)?;
        if got < 8 {
            break;
        }
        let small = u32::from_be_bytes([h[0], h[1], h[2], h[3]]) as u64;
        let (header, size) = match small {
            0 => (8, len - pos),
            1 if got >= 16 => (16, u64::from_be_bytes(h[8..16].try_into().unwrap())),
            1 => return Err(Unsupported),
            n => (8, n),
        };
        if size < header {
            return Err(Unsupported);
        }
        if &h[4..8] == b"moov" {
            // A size past the end is a truncated file; one past the cap is not worth the memory.
            if size > MOOV_CAP || pos + size > len {
                return Err(Unsupported);
            }
            let mut moov = vec![0u8; size as usize];
            if read_at(r, pos, &mut moov).map_err(|_| Unsupported)? != moov.len() {
                return Err(Unsupported);
            }
            return Ok(moov);
        }
        pos = pos.checked_add(size).ok_or(Unsupported)?;
    }
    Err(Unsupported)
}

/// The children of a box body as (name, payload); a child whose size leaves the body ends the list.
fn children(body: &[u8]) -> Vec<([u8; 4], &[u8])> {
    let mut out = Vec::new();
    let mut at = 0usize;
    while at + 8 <= body.len() {
        let small = u32::from_be_bytes(body[at..at + 4].try_into().unwrap()) as usize;
        let name: [u8; 4] = body[at + 4..at + 8].try_into().unwrap();
        let (header, size) = match small {
            0 => (8, body.len() - at),
            1 if at + 16 <= body.len() => {
                let big = u64::from_be_bytes(body[at + 8..at + 16].try_into().unwrap());
                (16, usize::try_from(big).unwrap_or(usize::MAX))
            }
            n => (8, n),
        };
        if size < header || size > body.len() - at {
            break;
        }
        out.push((name, &body[at + header..at + size]));
        at += size;
    }
    out
}

/// The bytes a box header takes: 8, or 16 with a 64-bit size.
fn header_len(b: &[u8]) -> usize {
    if b.starts_with(&[0, 0, 0, 1]) {
        16.min(b.len())
    } else {
        8.min(b.len())
    }
}

fn child<'a>(body: &'a [u8], name: &[u8; 4]) -> Option<&'a [u8]> {
    children(body)
        .into_iter()
        .find(|(n, _)| n == name)
        .map(|(_, p)| p)
}

/// What the first video `trak` states in its sample description: the entry's name and size, and its config box.
struct Entry<'a> {
    fourcc: [u8; 4],
    width: u32,
    height: u32,
    body: &'a [u8],
}

/// The sample entries of every video `trak`, in file order.
fn video_entries(moov: &[u8]) -> Vec<Option<Entry<'_>>> {
    let mut out = Vec::new();
    for (name, trak) in children(&moov[header_len(moov)..]) {
        if &name != b"trak" {
            continue;
        }
        let Some(mdia) = child(trak, b"mdia") else {
            continue;
        };
        let is_video = child(mdia, b"hdlr").is_some_and(|h| h.len() >= 12 && &h[8..12] == b"vide");
        if !is_video {
            continue;
        }
        let entry = child(mdia, b"minf")
            .and_then(|m| child(m, b"stbl"))
            .and_then(|s| child(s, b"stsd"))
            .filter(|d| d.len() >= 8)
            .and_then(|d| children(&d[8..]).into_iter().next())
            .filter(|(_, b)| b.len() >= VISUAL_ENTRY_FIXED)
            .map(|(fourcc, body)| Entry {
                fourcc,
                width: u16::from_be_bytes([body[24], body[25]]) as u32,
                height: u16::from_be_bytes([body[26], body[27]]) as u32,
                body,
            });
        out.push(entry);
    }
    out
}

/// ffprobe's codec name for a sample entry's name.
fn iso_codec(fourcc: &[u8; 4]) -> &'static str {
    match fourcc {
        b"avc1" | b"avc3" => "h264",
        b"hvc1" | b"hev1" => "hevc",
        b"vp08" => "vp8",
        b"vp09" => "vp9",
        b"av01" => "av1",
        b"mp4v" => "mpeg4",
        b"apch" | b"apcn" | b"apcs" | b"apco" | b"ap4h" | b"ap4x" => "prores",
        b"jpeg" | b"mjpa" => "mjpeg",
        _ => "",
    }
}

/// ffprobe's pixel format for a chroma ("420", "422", "444", "gray") at a bit depth.
pub(crate) fn pix_fmt(chroma: &str, depth: u32) -> String {
    let base = if chroma == "gray" {
        "gray".to_string()
    } else {
        format!("yuv{chroma}p")
    };
    if depth > 8 {
        format!("{base}{depth}le")
    } else {
        base
    }
}

fn chroma_of_idc(idc: u8) -> &'static str {
    ["gray", "420", "422", "444"][(idc & 3) as usize]
}

/// The pixel format an avcC states; the High profiles state chroma and depth after the parameter sets.
pub(crate) fn avcc_pix_fmt(b: &[u8]) -> String {
    if b.len() < 2 || ![100, 110, 122, 144, 244].contains(&b[1]) {
        return pix_fmt("420", 8);
    }
    let mut i = 5usize;
    let u16_at = |i: usize| {
        b.get(i..i + 2)
            .map(|s| u16::from_be_bytes([s[0], s[1]]) as usize)
    };
    let sets = b.get(i).map_or(0, |n| (n & 0x1f) as usize);
    i += 1;
    for _ in 0..sets {
        match u16_at(i) {
            Some(n) => i += 2 + n,
            None => break,
        }
    }
    let pps = b.get(i).copied().unwrap_or(0) as usize;
    i += 1;
    for _ in 0..pps {
        match u16_at(i) {
            Some(n) => i += 2 + n,
            None => break,
        }
    }
    if let (Some(c), Some(d)) = (b.get(i), b.get(i + 1)) {
        return pix_fmt(chroma_of_idc(*c), (*d & 7) as u32 + 8);
    }
    match b[1] {
        110 => pix_fmt("420", 10),
        122 => pix_fmt("422", 10),
        144 | 244 => pix_fmt("444", 10),
        _ => pix_fmt("420", 8),
    }
}

/// The pixel format an hvcC states (empty when the box is too short to say).
pub(crate) fn hvcc_pix_fmt(b: &[u8]) -> String {
    match (b.get(16), b.get(17)) {
        (Some(c), Some(d)) => pix_fmt(chroma_of_idc(*c), (*d & 7) as u32 + 8),
        _ => String::new(),
    }
}

/// The pixel format a sample entry's config box states; empty for a codec that states none.
fn iso_pix_fmt(e: &Entry) -> String {
    let kids = children(&e.body[VISUAL_ENTRY_FIXED..]);
    let find = |n: &[u8; 4]| kids.iter().find(|(k, _)| k == n).map(|(_, p)| *p);
    match iso_codec(&e.fourcc) {
        "h264" => find(b"avcC").map_or_else(String::new, avcc_pix_fmt),
        "hevc" => find(b"hvcC").map_or_else(String::new, hvcc_pix_fmt),
        "vp9" => match find(b"vpcC").and_then(|p| p.get(6)) {
            Some(b) => {
                let chroma = ["420", "420", "422", "444"][((b >> 1) & 7).min(3) as usize];
                pix_fmt(chroma, ((b >> 4) as u32).max(8))
            }
            None => pix_fmt("420", 8),
        },
        "av1" => match find(b"av1C").and_then(|p| p.get(2)) {
            Some(b) => {
                let depth = if b & 0x20 != 0 {
                    12
                } else if b & 0x40 != 0 {
                    10
                } else {
                    8
                };
                let chroma = match (b & 0x10 != 0, b & 0x08 != 0, b & 0x04 != 0) {
                    (true, _, _) => "gray",
                    (_, true, true) => "420",
                    (_, true, false) => "422",
                    _ => "444",
                };
                pix_fmt(chroma, depth)
            }
            None => pix_fmt("420", 8),
        },
        "prores" if matches!(&e.fourcc, b"ap4h" | b"ap4x") => "yuv444p10le".into(),
        "prores" => "yuv422p10le".into(),
        "vp8" => pix_fmt("420", 8),
        _ => String::new(),
    }
}

/// The display rotation the track header's matrix states, in the degrees ffprobe prints.
fn matrix_degrees(m: &mp4parse::Matrix) -> f64 {
    -(m.b as f64).atan2(m.a as f64).to_degrees()
}

/// Seconds as ffprobe prints a duration: to the microsecond.
pub(crate) fn micros(seconds: f64) -> f64 {
    if seconds.is_finite() {
        (seconds * 1e6).round() / 1e6
    } else {
        0.0
    }
}

fn read_iso<R: Read + Seek>(r: &mut R, len: u64) -> Result<Option<Value>, Unsupported> {
    let moov = find_moov(r, len)?;
    // mp4parse wants a file: a plain `ftyp` ahead of the one box it needs.
    let mut file = b"\0\0\0\x14ftypisom\0\0\0\0isom".to_vec();
    file.extend_from_slice(&moov);
    let ctx = mp4parse::read_mp4(&mut Cursor::new(file)).map_err(|_| Unsupported)?;
    let movie_scale = ctx.timescale.map_or(0, |t| t.0);
    let longest = ctx
        .tracks
        .iter()
        .filter_map(|t| t.tkhd.as_ref().map(|h| h.duration))
        .max()
        .unwrap_or(0);
    let seconds = if movie_scale == 0 {
        0.0
    } else {
        micros(longest as f64 / movie_scale as f64)
    };
    let videos: Vec<_> = ctx
        .tracks
        .iter()
        .filter(|t| t.track_type == TrackType::Video)
        .collect();
    let entries = video_entries(&moov);
    if videos.len() != entries.len() {
        return Err(Unsupported);
    }
    let (Some(track), Some(entry)) = (videos.first(), entries.first()) else {
        return Ok(None);
    };
    let entry = entry.as_ref().ok_or(Unsupported)?;
    let (mut frames, mut ticks) = (0u128, 0u128);
    for s in track.stts.iter().flat_map(|t| t.samples.iter()) {
        frames += s.sample_count as u128;
        ticks += s.sample_count as u128 * s.sample_delta as u128;
    }
    let scale = track.timescale.as_ref().map_or(0, |t| t.0) as u128;
    let fps = if ticks == 0 {
        0.0
    } else {
        (frames * scale) as f64 / ticks as f64
    };
    let degrees = track
        .tkhd
        .as_ref()
        .map_or(0.0, |h| matrix_degrees(&h.matrix));
    Ok(Some(facts(
        iso_codec(&entry.fourcc),
        entry.width,
        entry.height,
        &iso_pix_fmt(entry),
        fps,
        seconds,
        ISO_CONTAINER,
        degrees,
    )))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn facts(
    codec: &str,
    w: u32,
    h: u32,
    pix: &str,
    fps: f64,
    seconds: f64,
    container: &str,
    degrees: f64,
) -> Value {
    let mut m = Map::new();
    m.insert("codec".into(), json!(codec));
    m.insert("width".into(), json_number(w as f64));
    m.insert("height".into(), json_number(h as f64));
    m.insert("pixFmt".into(), json!(pix));
    m.insert(
        "fps".into(),
        json_number(if fps.is_finite() { fps } else { 0.0 }),
    );
    m.insert("seconds".into(), json_number(seconds));
    m.insert("container".into(), json!(container));
    let turn = rotation_of(&json!({ "side_data_list": [{ "rotation": degrees }] }));
    if turn != 0 {
        m.insert("rotation".into(), json!(turn));
    }
    Value::Object(m)
}
