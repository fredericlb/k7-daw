// picture_media.rs — `picture prepare <lane>`: each shot's video made fit for the picture, the copies made by the host in the background.
// A reader would get wrong that a host with no reader is not a failure: a file a browser plays as it is needs nothing, and the rest is said once.

use super::picture_shared::{lane_file, shots_of};
use super::picture_sound::is_video_source;
use super::shared::{Done, join, split_words};
use crate::host::{Host, Thrown};
use crate::host_media::{PrepareAsk, Probe, Refusal};
use k7x::edit::Value;
use song_edit::module_io::entity_file_for;
use song_edit::paths::resolve;
use song_edit::workspace_doc::ws_data;

/// The codecs every browser this app runs in decodes, 8-bit 4:2:0 only: ffprobe's names.
const PLAYABLE_CODECS: [&str; 4] = ["h264", "vp8", "vp9", "av1"];
const PLAYABLE_PIX_FMTS: [&str; 2] = ["yuv420p", "yuvj420p"];
/// The tallest picture previewed from its source; above it a proxy is made.
const PREVIEW_MAX_HEIGHT: f64 = 1080.0;

/// What one file needs: an optimized copy (a stream copy when only the container is wrong), a proxy, or neither.
#[derive(Clone, Debug, PartialEq)]
pub struct VideoPlan {
    pub optimized: Optimized,
    pub proxy: bool,
    pub reason: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Optimized {
    None,
    Remux,
    Transcode,
}

fn playable_container(c: &str) -> bool {
    let has = |w: &str| c.split(|ch: char| !ch.is_alphanumeric() && ch != '_').any(|x| x == w);
    has("mp4") || has("webm")
}

/// The plan for one probed video, decided from its facts alone.
pub fn plan_video(f: &Probe) -> VideoPlan {
    let codec_ok = PLAYABLE_CODECS.contains(&f.codec.as_str());
    let pix_ok = PLAYABLE_PIX_FMTS.contains(&f.pix_fmt.as_str());
    let proxy = f.height > PREVIEW_MAX_HEIGHT;
    let size = format!("{}×{}", num(f.width), num(f.height));
    let and_proxy = if proxy { ", and a proxy to preview" } else { "" };
    if let Some(rot) = f.rotation.filter(|r| *r != 0.0) {
        return VideoPlan {
            optimized: Optimized::Transcode,
            proxy,
            reason: format!(
                "{size} is turned {}° on display, which not every browser applies — an upright optimized copy{and_proxy}",
                num(rot)
            ),
        };
    }
    if !codec_ok || !pix_ok {
        let what = if !codec_ok {
            if f.codec.is_empty() {
                "an unknown codec".to_owned()
            } else {
                f.codec.clone()
            }
        } else {
            format!("{} in {}", f.codec, f.pix_fmt)
        };
        return VideoPlan {
            optimized: Optimized::Transcode,
            proxy,
            reason: format!("{what} is not something a browser decodes — an optimized copy at {size}{and_proxy}"),
        };
    }
    if !playable_container(&f.container) {
        return VideoPlan {
            optimized: Optimized::Remux,
            proxy,
            reason: format!(
                "{} is playable but its container is not — copied into mp4 without re-encoding{and_proxy}",
                f.codec
            ),
        };
    }
    if proxy {
        return VideoPlan {
            optimized: Optimized::None,
            proxy: true,
            reason: format!(
                "{size} is heavier than the preview needs — a proxy to preview, the export reads the source"
            ),
        };
    }
    VideoPlan {
        optimized: Optimized::None,
        proxy: false,
        reason: "ready as it is".to_owned(),
    }
}

fn num(n: f64) -> String {
    k7x::edit::js_number(n)
}

/// What a browser plays as it is, so a machine with no ffmpeg needs do nothing for it.
fn browser_plays(source: &str) -> bool {
    let lower = source.to_ascii_lowercase();
    [".mp4", ".m4v", ".webm"].iter().any(|e| lower.ends_with(e))
}

/// One clip's say and the ask that makes its copies, or why nothing can be said.
struct Work {
    say: String,
    ask: Option<PrepareAsk>,
    unconverted: bool,
}

fn clip_work(h: &mut Host, ref_: &str, clip: &Value) -> Result<Option<Work>, Refusal> {
    let source = match clip.get("source") {
        Some(Value::Str(s)) => s.clone(),
        _ => String::new(),
    };
    let abs = resolve(&join(&h.song_path, &source));
    let Some(facts) = h.probe(&abs)? else {
        return Ok(Some(Work {
            say: format!("{ref_}: {source} has no picture"),
            ask: None,
            unconverted: false,
        }));
    };
    let plan = plan_video(&facts);
    let have = |h: &Host, k: &str| match clip.get(k) {
        Some(Value::Str(rel)) if !rel.is_empty() => h.fs.exists(&resolve(&join(&h.song_path, rel))),
        _ => false,
    };
    let want_optimized = plan.optimized != Optimized::None && !have(h, "optimized");
    let want_proxy = plan.proxy && !have(h, "proxy");
    if facts.no_ffmpeg && want_optimized {
        let codec = if facts.codec.is_empty() {
            "an unknown codec"
        } else {
            &facts.codec
        };
        let pix = if facts.pix_fmt.is_empty() {
            "an unknown format"
        } else {
            &facts.pix_fmt
        };
        return Ok(Some(Work {
            say: format!("{ref_}: {codec} in {pix}, {}×{}", num(facts.width), num(facts.height)),
            ask: None,
            unconverted: true,
        }));
    }
    if facts.no_ffmpeg && want_proxy {
        return Ok(Some(Work {
            say: format!("{ref_}: previewed from its source — no ffmpeg here for a proxy"),
            ask: None,
            unconverted: false,
        }));
    }
    if !want_optimized && !want_proxy {
        let said = if plan.optimized == Optimized::None && !plan.proxy {
            "ready as it is"
        } else {
            "already prepared"
        };
        return Ok(Some(Work {
            say: format!("{ref_}: {said}"),
            ask: None,
            unconverted: false,
        }));
    }
    let optimized = want_optimized.then_some(if plan.optimized == Optimized::Remux {
        "remux"
    } else {
        "transcode"
    });
    Ok(Some(Work {
        say: format!("{ref_}: {}", plan.reason),
        ask: Some(PrepareAsk {
            clip: ref_.to_owned(),
            source,
            optimized,
            proxy: want_proxy,
        }),
        unconverted: false,
    }))
}

/// `picture prepare <lane>` — each shot's video made fit for the picture: a probe says what, the host makes it.
pub fn picture_prepare(a: &str, h: &mut Host) -> Done {
    let Some(lane_arg) = split_words(a).first().copied() else {
        h.refuse(["usage: picture prepare <lane>  (e.g. picture prepare main)".to_owned()]);
        return Ok(());
    };
    let Some(found) = lane_file(h, lane_arg)? else {
        return Ok(());
    };
    let mut refs: Vec<String> = Vec::new();
    for s in shots_of(h, &found)? {
        if !s.ref_.is_empty() && !refs.contains(&s.ref_) {
            refs.push(s.ref_);
        }
    }
    let mut lines: Vec<String> = Vec::new();
    let mut unconverted: Vec<String> = Vec::new();
    for ref_ in &refs {
        let dir = join(&h.song_path, "clips");
        let Some(file) = entity_file_for(&*h.fs, &dir, "clips", ref_) else {
            continue;
        };
        let clip = ws_data(&*h.fs, &file.path).map_err(Thrown::from)?;
        let source = match clip.get("source") {
            Some(Value::Str(s)) => s.clone(),
            _ => String::new(),
        };
        if !is_video_source(&source) {
            continue;
        }
        let work = match clip_work(h, ref_, &clip) {
            Ok(Some(w)) => w,
            Ok(None) => continue,
            Err(e) if e.no_ffmpeg => {
                if !browser_plays(&source) {
                    unconverted.push(ref_.clone());
                }
                continue;
            }
            Err(e) => {
                lines.push(format!("{ref_}: could not read it — {}", e.message));
                continue;
            }
        };
        lines.push(work.say);
        if work.unconverted {
            unconverted.push(ref_.clone());
        }
        let Some(ask) = work.ask else {
            continue;
        };
        match h.prepare(&ask) {
            Ok(made) => lines.push(made.line.unwrap_or_else(|| {
                if made.started {
                    format!("{ref_}: preparing in the background — the lane picks it up when it is written")
                } else {
                    format!("{ref_}: already being prepared")
                }
            })),
            Err(e) => lines.push(format!("{ref_}: could not prepare it — {}", e.message)),
        }
    }
    if !unconverted.is_empty() {
        lines.push(format!(
            "{}: this video may not play in the browser (an iPhone .mov, say) — convert it to MP4 (H.264), or install ffmpeg so k7 converts it for you",
            unconverted.join(", ")
        ));
    }
    if lines.is_empty() {
        lines.push(format!("nothing on {lane_arg} to prepare"));
    }
    h.log(lines);
    h.reload();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe(codec: &str, height: f64, container: &str) -> Probe {
        Probe {
            codec: codec.to_owned(),
            width: height * 16.0 / 9.0,
            height,
            pix_fmt: "yuv420p".to_owned(),
            seconds: 4.0,
            container: container.to_owned(),
            rotation: None,
            no_ffmpeg: false,
        }
    }

    #[test]
    fn a_playable_video_in_a_browser_container_needs_nothing() {
        let p = plan_video(&probe("h264", 720.0, "mov,mp4,m4a,3gp,3g2,mj2"));
        assert_eq!(
            (p.optimized, p.proxy, p.reason.as_str()),
            (Optimized::None, false, "ready as it is")
        );
    }

    #[test]
    fn a_playable_stream_in_the_wrong_box_is_copied_not_encoded() {
        let p = plan_video(&probe("h264", 720.0, "matroska,webm"));
        assert_eq!(p.optimized, Optimized::None);
        let p = plan_video(&probe("h264", 720.0, "mov"));
        assert_eq!(p.optimized, Optimized::Remux);
    }

    #[test]
    fn an_unplayable_codec_is_transcoded_and_a_tall_picture_gets_a_proxy() {
        let p = plan_video(&probe("hevc", 2160.0, "mov,mp4,m4a,3gp,3g2,mj2"));
        assert_eq!((p.optimized, p.proxy), (Optimized::Transcode, true));
        assert!(
            p.reason
                .starts_with("hevc is not something a browser decodes — an optimized copy at 3840×2160")
        );
    }
}
