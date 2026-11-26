// transcribe.rs — `track transcribe <track_in> <track_out>`: whisper listens to the film a picture lane places, and its words land as subtitles.
// A reader would get wrong when whisper runs: everything is checked BEFORE the call, so a refused line costs no transcription.

use super::picture_file::{Shot, add_shot, list_shots, remove_at};
use super::picture_shared::picture_lane_ids;
use super::picture_sound::is_video_source;
use super::routes::TRANSCRIBE;
use super::shared::{Done, join, split_words};
use super::words::is_name;
use crate::host::{Host, Thrown};
use k7x::edit::{Document, PathSeg, Value};
use serde_json::{Value as Json, json};
use song::files::has_ext;
use song_edit::chain_item_edit::{lane_file_for, lane_folder};
use song_edit::confine::confine_to_project;
use song_edit::lane_files::create_picture_lane;
use song_edit::workspace_doc::{EditOpts, ws_data, ws_edit, ws_text};
use song_edit::workspace_file::{ws_exists, ws_write};
use song_edit::yaml_edit::set_file_value;

const USAGE: &str = "usage: track transcribe <track_in> <track_out>  (e.g. track transcribe cam lyrics)";
const NAME_HINT: &str = "letters, digits, '.', '_' and '-' only";
/// The sources a caption clip reads (song's `CAPTION_EXTS`).
const CAPTION_EXTS: &[&str] = &["srt", "vtt"];

/// The file of picture lane `name`, or None when the song has no picture lane of that name.
fn picture_lane_file(h: &Host, name: &str) -> Result<Option<String>, Thrown> {
    if !picture_lane_ids(h)?.iter().any(|i| i == name) {
        return Ok(None);
    }
    Ok(lane_file_for(&*h.fs, &h.song_path, name).map(|f| f.path))
}

/// A clip file's `source:`, None when the file is not there or says none.
fn source_of(h: &Host, file: &str) -> Result<Option<String>, Thrown> {
    if !ws_exists(&*h.fs, file) {
        return Ok(None);
    }
    Ok(match ws_data(&*h.fs, file)?.get("source") {
        Some(Value::Str(s)) => Some(s.clone()),
        _ => None,
    })
}

/// One `HH:MM:SS,mmm` (or `.mmm`) stamp in seconds: whisper.cpp writes `.` where openai-whisper writes `,`.
fn stamp_seconds(s: &str) -> Option<f64> {
    let (hms, ms) = s.split_once([',', '.'])?;
    let mut parts = hms.split(':');
    let (h, m, sec) = (parts.next()?, parts.next()?, parts.next()?);
    let digits =
        |x: &str, min: usize, max: usize| (min..=max).contains(&x.len()) && x.bytes().all(|b| b.is_ascii_digit());
    if parts.next().is_some() || !digits(h, 2, 9) || !digits(m, 2, 2) || !digits(sec, 2, 2) || !digits(ms, 1, 3) {
        return None;
    }
    let n = |x: &str| x.parse::<f64>().ok();
    Some(n(h)? * 3600.0 + n(m)? * 60.0 + n(sec)? + n(&format!("{ms:0<3}"))? / 1000.0)
}

/// Whether a line opens with a cue's timing, `00:01:02,480 --> 00:01:05,120`; what follows the second stamp is not read.
fn is_timing(line: &str) -> bool {
    let Some((from, to)) = line.split_once("-->") else {
        return false;
    };
    let to = to.trim_start();
    let end = to
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, ':' | ',' | '.')))
        .unwrap_or(to.len());
    let stamp = &to[..end];
    // The second stamp's milliseconds take one to three digits: the longest prefix that reads is it.
    let to_ok = (0..stamp.len()).any(|cut| stamp_seconds(&stamp[..stamp.len() - cut]).is_some());
    stamp_seconds(from.trim_end()).is_some() && to_ok
}

/// How many non-empty cues an SRT text holds: a cue is its timing line plus the text after it, a garbled index ignored.
fn cue_count(text: &str) -> usize {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut n = 0;
    for block in text.split("\n\n") {
        let lines: Vec<&str> = block.split('\n').filter(|l| !l.trim().is_empty()).collect();
        if let Some(at) = lines.iter().position(|l| is_timing(l))
            && at + 1 < lines.len()
        {
            n += 1;
        }
    }
    n
}

/// `track transcribe <track_in> <track_out>` — the transcript as `clips/<out>.srt`, a caption clip, placed on picture lane <out> where the shot is.
pub fn track_transcribe(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    let [track_in, track_out] = w.as_slice() else {
        h.refuse([USAGE.to_owned()]);
        return Ok(());
    };
    let (track_in, track_out) = (*track_in, *track_out);
    if !is_name(track_in) {
        h.refuse([format!("'{track_in}' is not a lane name — {NAME_HINT}")]);
        return Ok(());
    }
    let Some(lane_in) = picture_lane_file(h, track_in)? else {
        h.refuse([format!("no picture lane \"{track_in}\" in this song")]);
        return Ok(());
    };
    // The FIRST shot decides what is listened to: one video, timed in its own seconds.
    let mut doc = Document::new(&ws_text(&*h.fs, &lane_in)?);
    let Some(shot) = list_shots(&doc.at(&[])).into_iter().find(|s| !s.ref_.is_empty()) else {
        h.refuse([format!("'{track_in}' places no shot — there is no video to transcribe")]);
        return Ok(());
    };
    if shot.ref_.contains(['/', '\\']) || shot.ref_.contains("..") {
        h.refuse([format!(
            "'{}' is not a clip id — the stem of a file in clips/",
            shot.ref_
        )]);
        return Ok(());
    }
    let dir = h.song_path.clone();
    let clip_file = join(&dir, &format!("clips/{}.k7x", shot.ref_));
    let Some(source) = source_of(h, &clip_file)?.filter(|s| !s.is_empty()) else {
        h.refuse([format!("clip '{0}' says no source (clips/{0}.k7x)", shot.ref_)]);
        return Ok(());
    };
    if !is_video_source(&source) {
        h.refuse([format!(
            "clip '{}' is not a video ({source}) — transcribe listens to picture, whose sound is the speech",
            shot.ref_
        )]);
        return Ok(());
    }
    // Text a clip file holds, so it may not name a file past the project's roots.
    let Some(video) = confine_to_project(&*h.fs, &h.env, &dir, &source, None) else {
        h.refuse([format!(
            "clip '{}' names a file outside the project: {source}",
            shot.ref_
        )]);
        return Ok(());
    };
    if !h.fs.exists(&video) {
        h.refuse([format!("the video is not on this machine: {source}")]);
        return Ok(());
    }
    if !is_name(track_out) {
        h.refuse([format!("'{track_out}' is not a track name — {NAME_HINT}")]);
        return Ok(());
    }
    if track_out == track_in {
        h.refuse([format!(
            "the subtitles need a lane of their own — '{track_in}' holds the video"
        )]);
        return Ok(());
    }
    let out_lane = picture_lane_file(h, track_out)?;
    let node_file = join(&dir, &format!("{}/{track_out}.k7x", lane_folder(&*h.fs, &dir)));
    if out_lane.is_none() && ws_exists(&*h.fs, &node_file) {
        h.refuse([format!(
            "'{track_out}' is a sound lane — subtitles are drawn on a picture lane: name a new one, or delete it first"
        )]);
        return Ok(());
    }
    // The caption clip shares the lane's name; one already there must be a subtitle file, never a shot.
    let srt_rel = format!("clips/{track_out}.srt");
    let clip_out = join(&dir, &format!("clips/{track_out}.k7x"));
    if ws_exists(&*h.fs, &clip_out) && !source_of(h, &clip_out)?.is_some_and(|s| has_ext(&s, CAPTION_EXTS)) {
        h.refuse([format!(
            "clip '{track_out}' is not a subtitle file (clips/{track_out}.k7x) — name another lane"
        )]);
        return Ok(());
    }
    h.log([format!(
        "whisper is listening to {source} — this can take a few minutes"
    )]);
    let answer = match h.call(TRANSCRIBE, json!({ "video": video }))? {
        Ok(v) => v,
        Err(f) => {
            h.refuse([format!("track transcribe: {}", f.error)]);
            return Ok(());
        }
    };
    let srt = answer.get("srt").and_then(Json::as_str).unwrap_or_default().to_owned();
    let cues = cue_count(&srt);
    if cues == 0 {
        h.refuse(["whisper heard no speech in this video".to_owned()]);
        return Ok(());
    }
    ws_write(h.fs, &join(&dir, &srt_rel), srt.as_bytes())?;
    let source_key = [PathSeg::Key("source".to_owned())];
    set_file_value(h.fs, &clip_out, &source_key, &Value::Str(srt_rel.clone()))?;
    let lane_out = match &out_lane {
        Some(f) => f.clone(),
        None => {
            create_picture_lane(h.fs, &h.env, &dir, track_out, h.new_id)?;
            lane_file_for(&*h.fs, &dir, track_out).map_or(node_file, |f| f.path)
        }
    };
    place(h, &lane_out, track_out, &shot)?;
    h.reload();
    let mut lines = vec![format!(
        "{cues} subtitles into {srt_rel}, placed on picture lane {track_out} under {}",
        shot.ref_
    )];
    if out_lane.is_some() {
        lines.push("the lane already existed, so its subtitles were replaced".to_owned());
    }
    h.log(lines);
    Ok(())
}

/// Replaces the lane's placements of `ref_` with one where the shot is and from where in its source.
fn place(h: &mut Host, lane: &str, ref_: &str, shot: &Shot) -> Result<(), Thrown> {
    let opts = EditOpts {
        create: false,
        raw: false,
    };
    ws_edit(h.fs, lane, opts, |doc| {
        let mut sc = doc.at(&[]);
        let mine: Vec<usize> = list_shots(&sc)
            .iter()
            .enumerate()
            .filter(|(_, p)| p.ref_ == ref_)
            .map(|(i, _)| i)
            .collect();
        for i in mine.into_iter().rev() {
            remove_at(&mut sc, i)?;
        }
        add_shot(&mut sc, ref_, shot.start, shot.length, shot.source_from)?;
        Ok(())
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{cue_count, stamp_seconds};

    #[test]
    fn an_srt_is_counted_by_its_timing_lines_in_either_dialect() {
        assert_eq!(stamp_seconds("00:01:02,48"), Some(62.48));
        assert_eq!(stamp_seconds("00:01:02.480"), Some(62.48));
        let srt = "1\n00:00:00,000 --> 00:00:00,900\nhello\n\n2\n00:00:01.000 --> 00:00:02.000\n\n\nx\n00:00:03,000 --> 00:00:04,000 X1:0\nthere\n";
        assert_eq!(cue_count(srt), 2);
        assert_eq!(cue_count("1\r\n00:00:00,000 --> 00:00:00,900\r\nhi\r\n"), 1);
        assert_eq!(cue_count("no cues at all"), 0);
    }
}
