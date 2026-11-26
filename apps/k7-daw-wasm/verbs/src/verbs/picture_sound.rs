// picture_sound.rs — `picture sound`: a video's sound extracted and played; and the shots an `event remove` on a sounding lane silences.
// A reader would get wrong the extraction: it is an ask, so `sound:` names a file the caller writes after the verb's writes land.

use super::picture_file::{list_shots, set_silent_at};
use super::picture_shared::{LaneAt, lane_file, layer_file, put};
use super::shared::{Done, join, split_words};
use crate::host::{Host, Thrown};
use crate::host_media::Extracted;
use k7x::edit::{Document, PathSeg, Value, js_number};
use song::lanes::lanes_of;
use song::pictures::{ShotIn, shots_of};
use song_edit::chain_item_edit::is_chain_project;
use song_edit::confine::{confine_to, confine_to_project, workspace_dir_of};
use song_edit::format_level::PICTURE_SOUND_LEVEL;
use song_edit::format_stamp::stamp_format_level;
use song_edit::lane_files::create_track_file;
use song_edit::lane_strip_plan::strip_new_lane;
use song_edit::loader::resolve_all;
use song_edit::paths::resolve;
use song_edit::workspace_doc::{ws_data, ws_text};
use song_edit::workspace_file::ws_exists;
use song_edit::yaml_edit::{delete_file_value, set_file_value};

const TRACK_REF: &str = "@meta/track";

/// `VIDEO_SOURCE`: a file the picture plays as video.
pub fn is_video_source(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    [".mp4", ".mov", ".m4v", ".webm", ".mkv"]
        .iter()
        .any(|e| lower.ends_with(e))
}

/// Where a video's sound is written: beside it, under a name no clip of its own is likely to hold.
fn sound_path_of(source: &str) -> String {
    let cut = source.rfind('.').unwrap_or(source.len());
    format!("{}.sound.wav", &source[..cut])
}

fn key(k: &str) -> Vec<PathSeg> {
    vec![PathSeg::Key(k.to_owned())]
}

fn text_field(clip: &Value, k: &str) -> String {
    match clip.get(k) {
        Some(Value::Str(s)) => s.clone(),
        _ => String::new(),
    }
}

/// What one clip came to: a line to log (or none), and whether the shot now HAS a sound.
struct Sounded {
    line: Option<String>,
    heard: bool,
}

fn silent(line: String) -> Sounded {
    Sounded {
        line: Some(line),
        heard: false,
    }
}

/// Gives one clip its `sound:`, asking for the extraction when the file is not there yet.
fn sound_clip(h: &mut Host, ref_: &str) -> Result<Sounded, Thrown> {
    let file = join(&h.song_path, &format!("clips/{ref_}.k7x"));
    let clip = ws_data(&*h.fs, &file)?;
    let source = text_field(&clip, "source");
    if !is_video_source(&source) {
        return Ok(silent(format!("{ref_}: not a video clip — nothing to hear")));
    }
    let sound = text_field(&clip, "sound");
    let want = if sound.is_empty() {
        sound_path_of(&source)
    } else {
        sound.clone()
    };
    let dir = h.song_path.clone();
    let video = confine_to_project(&*h.fs, &h.env, &dir, &source, None);
    let roots = [dir.clone(), workspace_dir_of(&h.env, &dir)];
    let abs = confine_to(&*h.fs, &roots, &resolve(&join(&dir, &want)));
    let (Some(video), Some(abs)) = (video.as_ref(), abs.as_ref()) else {
        let which = if video.is_none() { &source } else { &want };
        return Ok(silent(format!("{ref_}: {which} is outside the project")));
    };
    let mut made = String::new();
    if !h.fs.exists(abs) {
        match h.extract_sound(video, abs) {
            Ok(Extracted::NoSound) => {
                return Ok(silent(format!("{ref_}: {source} has no sound — the shot stays silent")));
            }
            Ok(Extracted::Made {
                frames,
                sample_rate,
                channels,
            }) => {
                made = format!(
                    "{ref_}: + {want} ({:.1}s, {} ch)",
                    frames / sample_rate,
                    js_number(channels)
                );
            }
            Err(e) => return Ok(silent(format!("{ref_}: could not extract its sound — {}", e.message))),
        }
    }
    if sound == want {
        return Ok(Sounded {
            line: (!made.is_empty()).then_some(made),
            heard: true,
        });
    }
    set_file_value(h.fs, &file, &key("sound"), &Value::Str(want.clone()))?;
    Ok(Sounded {
        line: Some(if made.is_empty() {
            format!("{ref_}: sound {want}")
        } else {
            made
        }),
        heard: true,
    })
}

/// The audio lane sounding `lane`: the one that already does, else a new one named `track`.
fn sound_lane(h: &mut Host, lane: &str, track: &str) -> Result<Vec<String>, song_edit::Error> {
    let already = super::graph_facts::graph_facts(h)
        .and_then(|f| f.video_source.iter().find(|(_, v)| *v == lane).map(|(k, _)| k.clone()));
    if let Some(name) = already {
        return Ok(vec![format!("{name} already plays {lane}'s sound")]);
    }
    let dir = h.song_path.clone();
    let written = create_track_file(h.fs, &dir, track, TRACK_REF, h.new_id)?;
    let file = join(&dir, &written[0]);
    delete_file_value(h.fs, &file, &key("events"))?;
    set_file_value(h.fs, &file, &key("video_source"), &Value::Str(lane.to_owned()))?;
    let (strip, _) = strip_new_lane(h.fs, &h.env, &dir, track, h.new_id);
    let mut all: Vec<String> = Vec::new();
    for f in written.into_iter().chain(strip) {
        if !all.contains(&f) {
            all.push(f);
        }
    }
    Ok(vec![format!(
        "+ track {track} plays {lane}'s sound ({})",
        all.join(", ")
    )])
}

/// `picture sound <lane> [track]` — the shots' sound, extracted once and played by the lane's own `#audio` half.
pub fn picture_sound(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    let Some(lane_arg) = w.first().copied() else {
        h.refuse(["usage: picture sound <lane> [track]  (e.g. picture sound main dialog)".to_owned()]);
        return Ok(());
    };
    let Some(found) = layer_file(h, lane_arg, "sound")? else {
        return Ok(());
    };
    let lane = found
        .rel
        .rsplit('/')
        .next()
        .unwrap_or("")
        .trim_end_matches(".k7x")
        .to_owned();
    let mut doc = Document::new(&ws_text(&*h.fs, &found.abs)?);
    let mut refs: Vec<String> = Vec::new();
    for s in list_shots(&doc.at(&found.base)) {
        if !s.ref_.is_empty() && !refs.contains(&s.ref_) {
            refs.push(s.ref_);
        }
    }
    let mut lines: Vec<String> = Vec::new();
    let mut heard = false;
    for ref_ in &refs {
        if !ws_exists(&*h.fs, &join(&h.song_path, &format!("clips/{ref_}.k7x"))) {
            lines.push(format!("{ref_}: no clips/{ref_}.k7x — skipped"));
            continue;
        }
        let done = sound_clip(h, ref_)?;
        lines.extend(done.line);
        heard |= done.heard;
    }
    if !heard {
        lines.push(format!("nothing on {lane} has a sound"));
        h.log(lines);
        h.reload();
        return Ok(());
    }
    if is_chain_project(&*h.fs, &h.song_path) {
        let track = w.get(1).map_or_else(|| format!("{lane}-sound"), |t| (*t).to_owned());
        match sound_lane(h, &lane, &track) {
            Ok(l) => lines.extend(l),
            Err(e) => {
                lines.push(format!("picture sound: {e}"));
                h.refuse(lines);
                return Ok(());
            }
        }
    } else {
        let dir = h.song_path.clone();
        stamp_format_level(h.fs, &dir, PICTURE_SOUND_LEVEL);
        lines.push(format!(
            "{lane}#audio plays {lane}'s sound — to the mix, or wherever `{lane}#audio` is wired"
        ));
    }
    h.log(lines);
    h.reload();
    Ok(())
}

/// A shot whose sound a removed clip was: its lane's file, its index in `events:` and its ref.
pub struct SoundedShot {
    pub lane: LaneAt,
    pub index: usize,
    pub ref_: String,
    pub start: f64,
}

/// The shot whose sound starts at `at_beats` on `picture`, as the clip `(name, start)` of the lane sounding it says, or why there is none.
pub fn shot_sounding_at(
    h: &mut Host,
    picture: &str,
    tempo: f64,
    clip: Option<(&str, f64)>,
) -> Result<Result<SoundedShot, String>, Thrown> {
    let Some(found) = lane_file(h, picture)? else {
        return Ok(Err(format!("no picture lane '{picture}'")));
    };
    let none = || Err(format!("no shot of {picture} sounds there"));
    let Some((name, beats)) = clip else {
        return Ok(none());
    };
    let resolved = resolve_all(&*h.fs, &h.env, &h.song_path)?;
    let lanes = lanes_of(&resolved);
    let Some(lane) = lanes.picture.get(picture) else {
        return Ok(none());
    };
    let own: Vec<ShotIn> = lane.events.iter().map(ShotIn::own).collect();
    let seconds = beats / (tempo / 60.0);
    let eps = 1e-6;
    let hit = shots_of(&own)
        .into_iter()
        .rfind(|s| s.ref_ == name && s.start - eps <= seconds && seconds < s.end);
    let Some(hit) = hit else {
        return Ok(none());
    };
    let shots = super::picture_shared::shots_of(h, &found)?;
    let Some(shot) = shots.get(hit.index) else {
        return Ok(none());
    };
    Ok(Ok(SoundedShot {
        lane: found.clone(),
        index: hit.index,
        ref_: shot.ref_.clone(),
        start: shot.start,
    }))
}

/// Silences every shot named, one edit per lane file; false once one was refused.
pub fn silence_shots(h: &mut Host, shots: &[SoundedShot]) -> bool {
    let mut files: Vec<&str> = Vec::new();
    for s in shots {
        if !files.contains(&s.lane.abs.as_str()) {
            files.push(&s.lane.abs);
        }
    }
    for abs in files {
        let mine: Vec<&SoundedShot> = shots.iter().filter(|s| s.lane.abs == abs).collect();
        let lane = mine[0].lane.clone();
        let silence = |sc: &mut k7x::edit::Scope| {
            for s in &mine {
                set_silent_at(sc, s.index, true)?;
            }
            Ok(true)
        };
        if !put(h, &lane, "silence", None, silence) {
            return false;
        }
    }
    true
}

/// `event remove` on a sounding lane: the shot under the clip is silenced, its picture kept.
pub fn silence_sound_at(h: &mut Host, track: &str, picture: &str, tempo: f64, clip: Option<(&str, f64)>) -> Done {
    let shot = match shot_sounding_at(h, picture, tempo, clip)? {
        Ok(s) => s,
        Err(why) => {
            h.refuse([why]);
            return Ok(());
        }
    };
    if !silence_shots(h, std::slice::from_ref(&shot)) {
        return Ok(());
    }
    h.log([format!(
        "{track}: {}'s sound silenced — the shot stays on {picture} (picture silence {picture} {} brings it back)",
        shot.ref_,
        js_number(shot.start)
    )]);
    h.reload();
    Ok(())
}
