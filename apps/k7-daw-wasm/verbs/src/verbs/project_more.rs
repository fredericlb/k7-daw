// project_more.rs — the project verbs of M4: time splice, master, aspect, surfaces, tags and console, after project.ts and its kin.
// A reader would get wrong `project master` at 0 dB: unity is the key REMOVED, not written as 0.

use super::shared::{Done, join, meter, position, split_words};
use crate::host::{Host, Thrown};
use k7x::edit::{PathSeg, Value, js_number, print_key};
use song::Cache;
use song::console::console_systems;
use song::views::surfaces::project_surfaces;
use song_edit::insert_time::splice_project_time;
use song_edit::js_text::{js_trim, str_to_number};
use song_edit::loader::load_module_refs;
use song_edit::project_root::{MASTER_MAX_DB, MASTER_MIN_DB, load_project_root};
use song_edit::start_time::js_round;
use song_edit::time_splice::Mode;
use song_edit::video_outputs::ensure_video_run;
use song_edit::workspace_doc::ws_write_text;
use song_edit::workspace_file::ws_exists;
use song_edit::yaml_edit::{delete_file_value, set_file_value};
use song_schema::{TimeSig, parse_duration};

const TAG_MAX: usize = 32;

fn path(keys: &[&str]) -> Vec<PathSeg> {
    keys.iter().map(|k| PathSeg::Key((*k).to_owned())).collect()
}

fn project_file(h: &Host) -> String {
    join(&h.song_path, "project.k7x")
}

fn splice(a: &str, h: &mut Host, mode: Mode) -> Done {
    let verb = if mode == Mode::Insert {
        "project add"
    } else {
        "project remove"
    };
    let parts = split_words(a);
    if parts.len() != 2 {
        h.refuse([
            format!("usage: {verb} <at> <length>"),
            format!("       (e.g. {verb} 5:1:1 1:0 — a position, then a LENGTH)"),
        ]);
        return Ok(());
    }
    let p = h.project()?;
    let (num, den) = meter(&p);
    let at = position(parts[0], &p);
    let len = TimeSig::new(num as u32, den as u32)
        .and_then(|sig| parse_duration(parts[1], sig))
        .map_err(|e| e.to_string());
    let (at_beats, len_beats) = match (at, len) {
        (Ok(a), Ok(l)) => (a, l),
        (Err(m), _) | (_, Err(m)) => {
            h.refuse([format!("bad timecode: {m}")]);
            return Ok(());
        }
    };
    if len_beats.is_nan() || len_beats <= 0.0 {
        h.refuse([format!("a length of {} moves nothing", parts[1])]);
        return Ok(());
    }
    if mode == Mode::Delete && at_beats < 0.0 {
        h.refuse(["can't remove time from before the song starts".to_owned()]);
        return Ok(());
    }
    let dir = h.song_path.clone();
    match splice_project_time(h.fs, &dir, at_beats, len_beats, mode) {
        Ok(r) if r.files.is_empty() => h.refuse([format!(
            "nothing at or after {} to {} — the project is unchanged",
            parts[0],
            if mode == Mode::Insert { "push" } else { "pull" }
        )]),
        Ok(r) => {
            h.reload();
            let removed = if r.removed > 0 {
                format!(", {} removed", r.removed)
            } else {
                String::new()
            };
            h.log([format!(
                "{} {} at {}: {} moved{removed} across {} file{}",
                if mode == Mode::Insert { "inserted" } else { "removed" },
                parts[1],
                parts[0],
                r.moved,
                r.files.len(),
                if r.files.len() == 1 { "" } else { "s" }
            )]);
        }
        Err(e) => h.refuse([format!("{verb}: {e}")]),
    }
    Ok(())
}

/// `project add <at> <length>` — time inserted, everything after pushed.
pub fn time_add(a: &str, h: &mut Host) -> Done {
    splice(a, h, Mode::Insert)
}

/// `project remove <at> <length>` — time taken out, everything after pulled.
pub fn time_remove(a: &str, h: &mut Host) -> Done {
    splice(a, h, Mode::Delete)
}

/// `project master <dB>` — `project.masterVolume`, the song's output level, pushed to the engine.
pub fn project_master(a: &str, h: &mut Host) -> Done {
    let raw = js_trim(a);
    let db = str_to_number(raw);
    if raw.is_empty() || !db.is_finite() {
        h.refuse([format!(
            "usage: project master <dB>  (0 is unity, {}..+{})",
            js_number(MASTER_MIN_DB),
            js_number(MASTER_MAX_DB)
        )]);
        return Ok(());
    }
    if !(MASTER_MIN_DB..=MASTER_MAX_DB).contains(&db) {
        let mut lines = vec![format!(
            "{} dB is outside the fader — it runs {}..+{} dB",
            js_number(db),
            js_number(MASTER_MIN_DB),
            js_number(MASTER_MAX_DB)
        )];
        if db > MASTER_MAX_DB {
            lines.push("a mix that needs more than +12 on its output has its headroom in the STRIPS — see apps/k7-daw-wasm/song/docs/masterVolume.md".to_owned());
        }
        h.refuse(lines);
        return Ok(());
    }
    let file = project_file(h);
    if db == 0.0 {
        delete_file_value(h.fs, &file, &path(&["project", "masterVolume"]))?;
    } else {
        set_file_value(h.fs, &file, &path(&["project", "masterVolume"]), &Value::Num(db))?;
    }
    h.apply_project();
    h.log([if db == 0.0 {
        "master at unity (0 dB) — the line is removed, which is what unity means".to_owned()
    } else {
        format!("master {}{} dB", if db > 0.0 { "+" } else { "" }, js_number(db))
    }]);
    Ok(())
}

const ASPECTS: [(&str, f64, f64); 2] = [("16:9", 1920.0, 1080.0), ("9:16", 1080.0, 1920.0)];

/// `project aspect <ratio>` — `project.viewport` reshaped, its long edge and export fps kept.
pub fn project_aspect(a: &str, h: &mut Host) -> Done {
    let want = js_trim(a);
    let Some(&(name, ww, wh)) = ASPECTS.iter().find(|x| x.0 == want) else {
        let names: Vec<&str> = ASPECTS.iter().map(|x| x.0).collect();
        h.refuse([format!("\"{want}\" is not a frame shape — {}", names.join(" or "))]);
        return Ok(());
    };
    let root = load_project_root(&*h.fs, &h.song_path, false)?;
    let now: Vec<f64> = match root.data.get("project").and_then(|p| p.get("viewport")) {
        Some(Value::List(l)) => l
            .iter()
            .map(|v| match v {
                Value::Num(n) => *n,
                _ => f64::NAN,
            })
            .collect(),
        _ => Vec::new(),
    };
    let fps = now.get(2).copied().filter(|f| *f > 0.0);
    let long = match (now.first(), now.get(1)) {
        (Some(w), Some(hh)) if *w != 0.0 && *hh != 0.0 && !w.is_nan() && !hh.is_nan() => w.max(*hh),
        _ => 0.0,
    };
    let (vw, vh) = if long == 0.0 {
        (ww, wh)
    } else if ww >= wh {
        (long, js_round(long * wh / ww))
    } else {
        (js_round(long * ww / wh), long)
    };
    let mut size = vec![Value::Num(vw), Value::Num(vh)];
    if let Some(f) = fps {
        size.push(Value::Num(f));
    }
    let file = project_file(h);
    set_file_value(h.fs, &file, &path(&["project", "viewport"]), &Value::List(size))?;
    h.log([format!("frame is {name} ({}×{})", js_number(vw), js_number(vh))]);
    h.reload();
    Ok(())
}

/// The surface types, their one layout and the components a new file lists unmapped.
fn surface_type(t: &str) -> Option<(&'static str, Vec<(String, String)>)> {
    let deck = |d: &str| -> Vec<(String, String)> {
        [
            ("tempo", format!("a tempo module — deck {d}'s stretch")),
            ("trim", format!("a trim module — deck {d}'s input gain")),
            ("eqLow", format!("an eq module — deck {d}'s lows")),
            ("eqMid", "the SAME eq module — its mids".to_owned()),
            ("eqHigh", "the SAME eq module — its highs".to_owned()),
            ("filter", format!("a filter module — deck {d}'s filter")),
            ("level", format!("a level module — deck {d}'s channel fader")),
            ("echo", format!("an echo module — deck {d}'s echo")),
            ("stemDrums", format!("a stem module — deck {d}'s drums")),
            ("stemBass", format!("a stem module — deck {d}'s bass")),
            ("stemOther", format!("a stem module — deck {d}'s other")),
            ("stemVocals", format!("a stem module — deck {d}'s vocals")),
        ]
        .into_iter()
        .map(|(k, v)| (format!("deck{d}.{k}"), v))
        .collect()
    };
    let same = |s: &str| format!("the SAME module — {s}");
    Some(match t {
        "timeline" => ("split", Vec::new()),
        "viewport" => ("floating", Vec::new()),
        "turntables" => {
            let mut m = vec![("crossfade".to_owned(), "a crossfade module — its mix param".to_owned())];
            m.extend(deck("A"));
            m.extend(deck("B"));
            ("split", m)
        }
        "mastering" => (
            "split",
            vec![
                (
                    "comp.lowCut",
                    "the quick-master module — its low cut on the mid, before it compresses".to_owned(),
                ),
                ("comp.low", same("how much it compresses the lows")),
                ("comp.mid", same("how much on the mids")),
                ("comp.high", same("how much on the highs")),
                ("comp.lowFreq", same("the crossover between lows and mids")),
                ("comp.highFreq", same("the crossover between mids and highs")),
                ("spread.width", same("the stereo width")),
                (
                    "spread.bassMono",
                    same("the frequency below which the bass is brought to the centre"),
                ),
                (
                    "limiter.hfLimit",
                    same("its HF limit, what it takes off the fastest highs"),
                ),
                ("limiter.drive", same("the limiter's input gain")),
                ("limiter.ceiling", same("its ceiling")),
                ("limiter.release", same("its release time")),
                (
                    "limiter.output",
                    same("its output word length, float, 24-bit or 16-bit"),
                ),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect(),
        ),
        _ => return None,
    })
}

/// The components a new surface file of type `t` lists unmapped, `[id, note]`, in the signal's order.
#[must_use]
pub fn surface_skeleton(t: &str) -> Option<Vec<(String, String)>> {
    surface_type(t).map(|(_, skeleton)| skeleton)
}

pub(crate) const SURFACE_TYPES: [&str; 4] = ["turntables", "timeline", "viewport", "mastering"];

/// `createSurfaceFile`: `surfaces/<id>.k7x`, refused when the name or the one drawn slot is taken.
fn create_surface(h: &mut Host, kind: &str, id: &str) -> Result<String, Thrown> {
    let Some((layout, skeleton)) = surface_type(kind) else {
        return Err(Thrown(format!(
            "unknown surface type '{kind}' — {}",
            SURFACE_TYPES.join(", ")
        )));
    };
    if id.is_empty()
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
    {
        return Err(Thrown(format!(
            "invalid surface name '{id}' — letters, digits, '_' and '-' only"
        )));
    }
    let rel = format!("surfaces/{id}.k7x");
    let dir = h.song_path.clone();
    if ws_exists(&*h.fs, &join(&dir, &rel)) {
        return Err(Thrown(format!("surface '{id}' already exists ({rel})")));
    }
    let s = project_surfaces(&*h.fs, &mut Cache::new(), &dir).map_err(|e| Thrown(e.to_string()))?;
    let name_of = |v: serde_json::Result<serde_json::Value>| v.ok().and_then(|j| j.as_str().map(str::to_owned));
    let taken = s.surfaces.iter().find(|p| {
        name_of(serde_json::to_value(p.surface.kind)).as_deref() == Some(kind)
            && name_of(serde_json::to_value(p.surface.layout)).as_deref() == Some(layout)
    });
    if let Some(t) = taken {
        return Err(Thrown(format!(
            "this project already declares a {kind} surface ({}), and only one is drawn",
            t.file
        )));
    }
    let mapping = if skeleton.is_empty() {
        String::new()
    } else {
        let lines: String = skeleton
            .iter()
            .map(|(k, why)| format!("  {}: null, // {why}\n", print_key(k)))
            .collect();
        format!("mapping({{\n{lines}}})\n")
    };
    h.fs.mkdir_all(&join(&dir, "surfaces"))
        .map_err(|e| Thrown(e.to_string()))?;
    ws_write_text(
        h.fs,
        &join(&dir, &rel),
        &format!("type(\"{kind}\")\nlayout(\"{layout}\")\n{mapping}"),
    )?;
    if kind == "viewport" {
        ensure_video_run(h.fs, &h.env, &dir);
    }
    Ok(rel)
}

/// `project surface new <type> [id]` — a special view, its file listing what it wants mapped.
pub fn surface_create(a: &str, h: &mut Host) -> Done {
    let usage = format!(
        "usage: project surface new <{}> [id]  (e.g. project surface new turntables desk)",
        SURFACE_TYPES.join("|")
    );
    let w = split_words(a);
    let kind = w.first().copied().unwrap_or("");
    if !SURFACE_TYPES.contains(&kind) {
        let first = if kind.is_empty() {
            usage.clone()
        } else {
            format!("unknown surface type '{kind}'")
        };
        h.refuse([first, usage]);
        return Ok(());
    }
    let id = w.get(1).copied().unwrap_or(kind);
    match create_surface(h, kind, id) {
        Ok(rel) => {
            h.reload();
            h.log([format!("created {rel}")]);
        }
        Err(e) => h.refuse([format!("project surface new: {}", e.0), usage]),
    }
    Ok(())
}

/// `normalizeTag`: trimmed, inner spaces made one, at most 32 UTF-16 units.
fn normalize_tag(raw: &str) -> Option<String> {
    let one = split_words(raw).join(" ");
    let units: Vec<u16> = one.encode_utf16().take(TAG_MAX).collect();
    let t = String::from_utf16_lossy(&units);
    let t = js_trim(&t);
    (!t.is_empty()).then(|| t.to_owned())
}

fn edit_tags(a: &str, h: &mut Host, add: bool) -> Done {
    let root = load_project_root(&*h.fs, &h.song_path, false)?;
    let current: Vec<String> = match root.data.get("project").and_then(|p| p.get("tags")) {
        Some(Value::List(l)) => l.iter().map(song_edit::js_text::js_string).collect(),
        _ => Vec::new(),
    };
    let Some(tag) = normalize_tag(a) else {
        if add {
            h.log([if current.is_empty() {
                "no tags".to_owned()
            } else {
                current.join(", ")
            }]);
        } else {
            h.refuse(["usage: project untag <tag>".to_owned()]);
        }
        return Ok(());
    };
    let same = |t: &String| t.to_lowercase() == tag.to_lowercase();
    let next: Vec<String> = if add {
        let mut n = current.clone();
        if !current.iter().any(same) {
            n.push(tag.clone());
        }
        n
    } else {
        current.into_iter().filter(|t| !same(t)).collect()
    };
    let file = project_file(h);
    let done = if next.is_empty() {
        delete_file_value(h.fs, &file, &path(&["project", "tags"]))
    } else {
        let list = Value::List(next.iter().cloned().map(Value::Str).collect());
        set_file_value(h.fs, &file, &path(&["project", "tags"]), &list)
    };
    if let Err(e) = done {
        h.refuse([format!("project {}: {e}", if add { "tag" } else { "untag" })]);
        return Ok(());
    }
    h.log([if next.is_empty() {
        "no tags".to_owned()
    } else {
        format!("tags: {}", next.join(", "))
    }]);
    Ok(())
}

/// `project tag [tag]` — adds a tag, or lists them bare.
pub fn project_tag(a: &str, h: &mut Host) -> Done {
    edit_tags(a, h, true)
}

/// `project untag <tag>` — whatever its case.
pub fn project_untag(a: &str, h: &mut Host) -> Done {
    edit_tags(a, h, false)
}

/// `project console [system|none]` — the summing console over the mix; bare, the systems there are.
pub fn project_console(a: &str, h: &mut Host) -> Done {
    let want = split_words(a).first().map(|s| (*s).to_owned());
    let vocab = load_module_refs(&*h.fs, &h.env, &h.song_path)?;
    let systems = console_systems(&vocab);
    let Some(want) = want else {
        if systems.is_empty() {
            h.log(["no console in this account's libraries — a system is a library's console/<name>/ with channel/ and buss/".to_owned()]);
        } else {
            h.log(systems.iter().map(|s| format!("{}  {}", s.ref_, s.label)));
        }
        return Ok(());
    };
    let file = project_file(h);
    if want == "none" {
        delete_file_value(h.fs, &file, &path(&["project", "console"]))?;
        h.log(["console: none — the lanes sum as they are".to_owned()]);
        h.reload();
        return Ok(());
    }
    if !systems.iter().any(|s| s.ref_ == want) {
        let there = if systems.is_empty() {
            String::new()
        } else {
            let refs: Vec<&str> = systems.iter().map(|s| s.ref_.as_str()).collect();
            format!(" — there are {}", refs.join(", "))
        };
        h.refuse([format!("no console '{want}'{there}")]);
        return Ok(());
    }
    set_file_value(h.fs, &file, &path(&["project", "console"]), &Value::Str(want.clone()))?;
    h.log([format!(
        "console: {want} — a channel after every strip, the buss first on the master"
    )]);
    h.reload();
    Ok(())
}
