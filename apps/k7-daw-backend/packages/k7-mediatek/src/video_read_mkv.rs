//! video_read_mkv.rs — `video_read`'s Matroska and WebM half: the first video track by `matroska-demuxer`, in ffprobe's names.
//! What a reader gets wrong: the rate is `av_reduce` of the default duration with ffmpeg's 30000 bound, not 1e9 / duration.

use std::io::{Read, Seek, SeekFrom};

use serde_json::Value;

use crate::video_read::{
    MATROSKA_CONTAINER, Unsupported, avcc_pix_fmt, facts, hvcc_pix_fmt, micros, pix_fmt,
};

/// Bytes the Matroska reader may take from the file in all (header, tracks, cues).
const MKV_BUDGET: u64 = 64 * 1024 * 1024;

/// A source that fails once it has been asked for more than the budget: a hostile file cannot make the parser read it all.
struct Budgeted<R> {
    inner: R,
    left: u64,
}

impl<R: Read> Read for Budgeted<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let want = (buf.len() as u64).min(self.left) as usize;
        if want == 0 && !buf.is_empty() {
            return Err(std::io::Error::other("read budget spent"));
        }
        let n = self.inner.read(&mut buf[..want])?;
        self.left -= n as u64;
        Ok(n)
    }
}

impl<R: Seek> Seek for Budgeted<R> {
    fn seek(&mut self, to: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(to)
    }
}

/// ffprobe's codec name for a Matroska codec id.
fn mkv_codec(id: &str) -> &'static str {
    match id {
        "V_MPEG4/ISO/AVC" => "h264",
        "V_MPEGH/ISO/HEVC" => "hevc",
        "V_VP8" => "vp8",
        "V_VP9" => "vp9",
        "V_AV1" => "av1",
        "V_PRORES" => "prores",
        "V_MPEG4/ISO/ASP" | "V_MPEG4/ISO/SP" | "V_MPEG4/ISO/AP" => "mpeg4",
        "V_MPEG2" => "mpeg2video",
        "V_MPEG1" => "mpeg1video",
        "V_MJPEG" => "mjpeg",
        _ => "",
    }
}

/// libavutil's `av_reduce`: the fraction closest to num/den whose parts stay within `max`.
fn reduce(num: u64, den: u64, max: u64) -> (u64, u64) {
    fn gcd(a: u64, b: u64) -> u64 {
        if b == 0 { a } else { gcd(b, a % b) }
    }
    let g = gcd(num, den).max(1);
    let (num, den) = (num / g, den / g);
    if num <= max && den <= max {
        return (num, den);
    }
    let (mut a0, mut a1) = ((0u128, 1u128), (1u128, 0u128));
    let (mut n, mut d) = (num as u128, den as u128);
    let max = max as u128;
    while d != 0 {
        let mut x = n / d;
        let next = n - x * d;
        let a2 = (x * a1.0 + a0.0, x * a1.1 + a0.1);
        if a2.0 > max || a2.1 > max {
            if let Some(q) = (max - a0.0).checked_div(a1.0) {
                x = q;
            }
            if let Some(q) = (max - a0.1).checked_div(a1.1) {
                x = x.min(q);
            }
            if d * (2 * x * a1.1 + a0.1) > n * a1.1 {
                a1 = (x * a1.0 + a0.0, x * a1.1 + a0.1);
            }
            break;
        }
        a0 = a1;
        a1 = a2;
        n = d;
        d = next;
    }
    (a1.0 as u64, a1.1 as u64)
}

pub(crate) fn read_matroska<R: Read + Seek>(mut r: R) -> Result<Option<Value>, Unsupported> {
    r.seek(SeekFrom::Start(0)).map_err(|_| Unsupported)?;
    let src = Budgeted {
        inner: r,
        left: MKV_BUDGET,
    };
    let file = matroska_demuxer::MatroskaFile::open(src).map_err(|_| Unsupported)?;
    let Some(track) = file
        .tracks()
        .iter()
        .find(|t| t.track_type() == matroska_demuxer::TrackType::Video)
    else {
        return Ok(None);
    };
    let Some(video) = track.video() else {
        return Ok(None);
    };
    let codec = mkv_codec(track.codec_id());
    let private = track.codec_private().unwrap_or(&[]);
    let pix = match codec {
        "h264" => avcc_pix_fmt(private),
        "hevc" => hvcc_pix_fmt(private),
        "prores" => "yuv422p10le".into(),
        _ => {
            let colour = video.colour();
            let depth = colour.and_then(|c| c.bits_per_channel()).unwrap_or(8) as u32;
            let sub = colour.map(|c| (c.chroma_subsampling_horz(), c.chroma_subsampling_vert()));
            let chroma = match sub {
                Some((Some(1), Some(0))) => "422",
                Some((Some(0), Some(0))) => "444",
                _ => "420",
            };
            pix_fmt(chroma, depth.clamp(8, 16))
        }
    };
    let fps = match track.default_duration() {
        Some(ns) => {
            let (n, d) = reduce(1_000_000_000, ns.get(), 30000);
            if d == 0 { 0.0 } else { n as f64 / d as f64 }
        }
        None => 0.0,
    };
    let info = file.info();
    let seconds = info.duration().map_or(0.0, |ticks| {
        micros(ticks * info.timestamp_scale().get() as f64 / 1e9)
    });
    Ok(Some(facts(
        codec,
        video.pixel_width().get().min(u32::MAX as u64) as u32,
        video.pixel_height().get().min(u32::MAX as u64) as u32,
        &pix,
        fps,
        seconds,
        MATROSKA_CONTAINER,
        0.0,
    )))
}
