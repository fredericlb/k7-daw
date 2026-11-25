// picture_interchange.rs — `picture interchange <otio|edl> [lane]` hands the cut over as a line; `picture import <file>` writes one as lanes.
// A reader would get wrong that nothing is saved: the cut is the FIRST output line, whole, and the app writes it where the person chooses.

use super::picture_cut::{to_edl, to_otio};
use super::picture_doc::{PicClip, load_pictures};
use super::picture_file::ms;
use super::picture_read_cut::{Cut, Imported, from_edl, from_otio};
use super::shared::{Done, join, split_words};
use crate::host::Host;
use k7x::edit::{PathSeg, StringifyOptions, Value};
use song_edit::chain_item_edit::lane_file_for;
use song_edit::lane_files::{PICTURE_TRACK_REF, create_picture_lane};
use song_edit::module_io::{entity_file_for, entity_files};
use song_edit::paths::{basename, climbs_out, dirname, relative, resolve};
use song_edit::project_root::load_project_root;
use song_edit::workspace_doc::{ws_data, ws_write_data};
use song_edit::yaml_edit::set_file_value;
use std::collections::HashMap;

const FORMATS: [&str; 2] = ["otio", "edl"];

fn project_name(h: &Host) -> String {
    let named = load_project_root(&*h.fs, &h.song_path, false).ok().and_then(|r| {
        match r.project().and_then(|p| p.get("name")) {
            Some(Value::Str(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        }
    });
    named.unwrap_or_else(|| {
        let base = basename(&h.song_path);
        let name = base.strip_suffix(".k7").unwrap_or(base);
        if name.is_empty() {
            "song".to_owned()
        } else {
            name.to_owned()
        }
    })
}

/// `picture interchange <otio|edl> [lane]` — the cut for another editor, said as the first output line and written nowhere.
pub fn picture_interchange(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    let Some(format) = w.first().copied().filter(|f| FORMATS.contains(f)) else {
        h.refuse([format!("usage: picture interchange <{}> [lane]", FORMATS.join("|"))]);
        return Ok(());
    };
    let doc = match load_pictures(h) {
        Ok(d) => d,
        Err(e) => {
            h.refuse([format!("picture interchange: {e}")]);
            return Ok(());
        }
    };
    if doc.lanes.is_empty() {
        h.refuse(["this song has no picture lane to hand over".to_owned()]);
        return Ok(());
    }
    let mut lengths: HashMap<String, f64> = HashMap::new();
    for lane in &doc.lanes {
        if let Some(last) = lane
            .clips
            .last()
            .filter(|c| !c.source.is_empty() && c.length <= 0.0 && !c.still)
            && let Ok(Some(f)) = h.probe(&last.source)
            && f.seconds != 0.0
        {
            lengths.insert(last.source.clone(), f.seconds);
        }
    }
    let natural = |c: &PicClip| lengths.get(&c.source).copied();
    let name = project_name(h);
    let (text, skipped) = if format == "otio" {
        let exports = resolve(&join(&h.song_path, "exports"));
        let url = |c: &PicClip| relative(&exports, &c.source).replace('\\', "/");
        to_otio(&doc, &name, &url, &natural)
    } else {
        let lane = match w.get(1) {
            Some(id) => doc.lanes.iter().find(|l| l.id == *id),
            None => doc.lanes.first(),
        };
        let Some(lane) = lane else {
            let ids: Vec<&str> = doc.lanes.iter().map(|l| l.id.as_str()).collect();
            h.refuse([format!(
                "no picture lane '{}' — this song's are {}",
                w.get(1).copied().unwrap_or(""),
                ids.join(", ")
            )]);
            return Ok(());
        };
        to_edl(
            lane,
            &name,
            doc.fps,
            &|c: &PicClip| basename(&c.source).to_owned(),
            &natural,
        )
    };
    let mut lines = vec![text];
    lines.extend(skipped.iter().map(|s| format!("left out — {s}")));
    h.log(lines);
    Ok(())
}

/// Finds, and where it must declares, the clip each imported shot's media is.
struct Media {
    from: String,
    by_source: HashMap<String, String>,
    taken: Vec<String>,
    in_clips: HashMap<String, String>,
    missing: Vec<String>,
}

fn decode_uri(s: &str) -> String {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let hex = |x: u8| char::from(x).to_digit(16);
        if b[i] == b'%'
            && i + 2 < b.len()
            && let (Some(hi), Some(lo)) = (hex(b[i + 1]), hex(b[i + 2]))
        {
            let byte = (hi * 16 + lo) as u8;
            // decodeURI leaves the reserved escapes as they are.
            if b";/?:@&=+$,#".contains(&byte) {
                out.extend_from_slice(&b[i..i + 3]);
            } else {
                out.push(byte);
            }
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_owned())
}

impl Media {
    fn new(h: &Host, from: &str) -> Media {
        let dir = join(&h.song_path, "clips");
        let mut by_source = HashMap::new();
        let mut taken = Vec::new();
        for (id, file) in entity_files(&*h.fs, &dir, "clips") {
            if let Ok(Value::Map(m)) = ws_data(&*h.fs, &file.path)
                && let Some((_, Value::Str(s))) = m.iter().find(|(k, _)| k == "source")
            {
                by_source.insert(s.clone(), id.clone());
            }
            taken.push(id);
        }
        let in_clips =
            h.fs.list(&dir)
                .map(|l| {
                    l.into_iter()
                        .map(|e| (e.name.to_lowercase(), format!("clips/{}", e.name)))
                        .collect()
                })
                .unwrap_or_default();
        Media {
            from: from.to_owned(),
            by_source,
            taken,
            in_clips,
            missing: Vec::new(),
        }
    }

    /// The project-relative path of `media`: as the file names it, else by name in clips/, else where it should be.
    fn locate(&mut self, h: &Host, media: &str) -> String {
        let path = decode_uri(media.strip_prefix("file://").unwrap_or(media));
        let abs = resolve(&join(&dirname(&self.from), &path));
        let rel = relative(&h.song_path, &abs);
        if !path.is_empty() && h.fs.exists(&abs) && !climbs_out(&rel) {
            return rel.replace('\\', "/");
        }
        let name = basename(&path).to_owned();
        if let Some(hit) = self.in_clips.get(&name.to_lowercase()) {
            return hit.clone();
        }
        if !self.missing.contains(&name) {
            self.missing.push(name.clone());
        }
        format!("clips/{name}")
    }

    /// The clip id for a shot's media, declaring `clips/<id>.k7x` the first time a file is met.
    fn clip_for(&mut self, h: &mut Host, shot: &Imported) -> Result<String, song_edit::Error> {
        let source = self.locate(h, if shot.media.is_empty() { &shot.name } else { &shot.media });
        if let Some(known) = self.by_source.get(&source) {
            return Ok(known.clone());
        }
        let base = basename(&source);
        let stem = base
            .rfind('.')
            .filter(|at| at + 1 < base.len())
            .map_or(base, |at| &base[..at]);
        let mut stem: String = {
            let mut out = String::new();
            let mut in_run = false;
            for c in stem.chars() {
                if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                    out.push(c);
                    in_run = false;
                } else if !in_run {
                    out.push('-');
                    in_run = true;
                }
            }
            out
        };
        if stem.is_empty() {
            stem = "shot".to_owned();
        }
        let mut id = stem.clone();
        let mut n = 2;
        while self.taken.contains(&id) {
            id = format!("{stem}-{n}");
            n += 1;
        }
        let file = join(&h.song_path, &format!("clips/{id}.k7x"));
        let data = Value::Map(vec![("source".to_owned(), Value::Str(source.clone()))]);
        h.fs.mkdir_all(&join(&h.song_path, "clips"))
            .map_err(|e| song_edit::error::Error::Msg(e.to_string()))?;
        ws_write_data(h.fs, &file, &data, &StringifyOptions::default())?;
        self.taken.push(id.clone());
        self.by_source.insert(source, id.clone());
        Ok(id)
    }
}

fn entry(ref_: &str, s: &Imported) -> Value {
    let mut f = vec![
        ("ref".to_owned(), Value::Str(ref_.to_owned())),
        ("start".to_owned(), Value::Num(ms(s.start))),
        ("length".to_owned(), Value::Num(ms(s.length))),
    ];
    if s.source_from > 1e-6 {
        f.push(("from".to_owned(), Value::Num(ms(s.source_from))));
    }
    if let Some(sp) = s.speed.filter(|v| *v != 1.0) {
        f.push(("speed".to_owned(), Value::Num(ms(sp))));
    }
    if s.fade_in != 0.0 {
        f.push(("fadeIn".to_owned(), Value::Num(ms(s.fade_in))));
    }
    Value::Map(f)
}

fn read_cut(h: &Host, abs: &str, file_arg: &str, fallback_fps: f64) -> Result<Cut, String> {
    let bytes =
        h.fs.read(abs, song::Span::All)
            .map_err(|e| format!("{file_arg}: {e}"))?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let lower = abs.to_lowercase();
    if lower.ends_with(".otio") {
        from_otio(&text).map_err(|e| format!("{file_arg}: {e}"))
    } else if lower.ends_with(".edl") {
        Ok(from_edl(&text, fallback_fps))
    } else {
        Err(format!("{file_arg}: an .otio or an .edl is what this reads"))
    }
}

/// `picture import <file> [prefix]` — a cut from another editor written as this project's own picture lanes.
pub fn picture_import(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    let Some(file_arg) = w.first().copied() else {
        h.refuse(["usage: picture import <file.otio|file.edl> [lane prefix]".to_owned()]);
        return Ok(());
    };
    let dir = h.song_path.clone();
    let abs = resolve(&join(&dir, file_arg));
    if climbs_out(&relative(&dir, &abs)) || !h.fs.exists(&abs) {
        h.refuse([format!(
            "no {file_arg} in this project — put the file in it, then name it from the project folder"
        )]);
        return Ok(());
    }
    let root = load_project_root(&*h.fs, &dir, false)?;
    let viewport: Vec<f64> = match root.project().and_then(|p| p.get("viewport")) {
        Some(Value::List(l)) => l.iter().map(song_edit::start_time::js_to_number).collect(),
        _ => Vec::new(),
    };
    let vp = |i: usize| viewport.get(i).copied().filter(|n| *n != 0.0 && !n.is_nan());
    let cut = match read_cut(h, &abs, file_arg, vp(2).unwrap_or(25.0)) {
        Ok(c) => c,
        Err(e) => {
            h.refuse([e]);
            return Ok(());
        }
    };
    let tracks: Vec<_> = cut.tracks.iter().filter(|t| !t.shots.is_empty()).collect();
    if tracks.is_empty() {
        let mut said = vec![format!("{file_arg} holds no picture to import")];
        said.extend(cut.skipped.iter().cloned());
        h.refuse(said);
        return Ok(());
    }
    let mut lines: Vec<String> = Vec::new();
    if (vp(0).is_none() || vp(1).is_none())
        && let Some(file) = entity_file_for(&*h.fs, &dir, "", "project")
    {
        let rate = js_round(cut.fps);
        let at = [PathSeg::Key("project".to_owned()), PathSeg::Key("viewport".to_owned())];
        let v = Value::List(vec![Value::Num(1920.0), Value::Num(1080.0), Value::Num(rate)]);
        set_file_value(h.fs, &file.path, &at, &v)?;
        lines.push(format!(
            "project: viewport [1920, 1080, {}] — the cut's rate",
            k7x::edit::js_number(rate)
        ));
    }
    let mut media = Media::new(h, &abs);
    let prefix = w.get(1).copied().unwrap_or("import-").to_owned();
    let mut n = 1;
    for track in tracks {
        while lane_file_for(&*h.fs, &dir, &format!("{prefix}{n}")).is_some() {
            n += 1;
        }
        let id = format!("{prefix}{n}");
        if let Err(e) = create_picture_lane(h.fs, &h.env, &dir, &id, h.new_id) {
            lines.push(format!("picture import: {e}"));
            h.refuse(lines);
            return Ok(());
        }
        let mut events = Vec::new();
        for s in &track.shots {
            let ref_ = media.clip_for(h, s)?;
            events.push(entry(&ref_, s));
        }
        let from = format!(
            "imported from {}{}",
            basename(&abs),
            if track.name.is_empty() {
                String::new()
            } else {
                format!(", track {}", track.name)
            }
        );
        let Some(lane) = lane_file_for(&*h.fs, &dir, &id) else {
            lines.push(format!("picture import: lane '{id}' has no file"));
            h.refuse(lines);
            return Ok(());
        };
        let data = Value::Map(vec![
            ("extends".to_owned(), Value::Str(PICTURE_TRACK_REF.to_owned())),
            ("events".to_owned(), Value::List(events)),
        ]);
        let header = [from];
        let inline =
            |path: &[PathSeg], _: &Value| path.len() == 2 && matches!(&path[0], PathSeg::Key(k) if k == "events");
        let opts = StringifyOptions {
            header: &header,
            inline: Some(&inline),
        };
        ws_write_data(h.fs, &lane.path, &data, &opts)?;
        lines.push(format!(
            "+ picture lane {id}: {} shot(s){}",
            track.shots.len(),
            if track.name.is_empty() {
                String::new()
            } else {
                format!(" from {}", track.name)
            }
        ));
    }
    if !media.missing.is_empty() {
        let mut names = media.missing.clone();
        names.sort();
        lines.push(format!(
            "not found, placed anyway — copy into clips/: {}",
            names.join(", ")
        ));
    }
    lines.extend(cut.skipped.iter().map(|s| format!("left out — {s}")));
    h.log(lines);
    h.reload();
    Ok(())
}

fn js_round(x: f64) -> f64 {
    song_edit::start_time::js_round(x)
}
