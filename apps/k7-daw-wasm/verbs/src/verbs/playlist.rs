// playlist.rs — the workspace's set lists under `_playlists/`: new, song add/remove/move and stems, after verbs/playlist.ts.
// A reader would get wrong `playlist stems`: the separation is the caller's, asked as `playlistStems` with the chain unresolved.

use super::shared::{Done, join, split_words};
use crate::host::Host;
use k7x::edit::{PathSeg, Scope, StringifyOptions, Value, WriteOptions, stringify};
use song_edit::confine::workspace_dir_of;
use song_edit::js_text::{js_trim, str_to_number};
use song_edit::paths::{is_within, resolve};
use song_edit::preferences_file::load_preferences;
use song_edit::workspace_doc::{EditOpts, ws_edit, ws_text, ws_write_text};
use song_edit::workspace_file::{k7x_data, ws_exists};

const PLAYLISTS_DIR: &str = "_playlists";

struct Item {
    song: String,
    variant: Option<String>,
}

fn label(i: &Item) -> String {
    match &i.variant {
        Some(v) => format!("{} ({v})", i.song),
        None => i.song.clone(),
    }
}

fn str_of(v: Option<&Value>) -> Option<String> {
    match v {
        Some(Value::Str(s)) if !js_trim(s).is_empty() => Some(js_trim(s).to_owned()),
        _ => None,
    }
}

/// `parsePlaylist`: its items, or None for a text that does not read as a map.
fn parse_playlist(path: &str, text: &str) -> Option<Vec<Item>> {
    let d = k7x_data(path, text).ok()?;
    if !matches!(d, Value::Map(_)) {
        return None;
    }
    let Some(Value::List(lines)) = d.get("items") else {
        return Some(Vec::new());
    };
    Some(
        lines
            .iter()
            .filter_map(|l| match str_of(Some(l)) {
                Some(song) => Some(Item { song, variant: None }),
                None => str_of(l.get("song")).map(|song| Item {
                    song,
                    variant: str_of(l.get("variant")),
                }),
            })
            .collect(),
    )
}

/// `playlistFile`: where a playlist name lands in the workspace, or None with the refusal said.
fn playlist_file(name: &str, h: &mut Host) -> Option<(String, String)> {
    let t = js_trim(name);
    let lower = t.to_ascii_lowercase();
    if lower.ends_with(".yaml") || lower.ends_with(".yml") {
        h.refuse([format!("'{name}': a playlist is a .k7x file — name it without .yaml")]);
        return None;
    }
    let clean = t.trim_matches('/');
    let plain = !clean.is_empty() && clean.split('/').all(|s| !s.is_empty() && s != "." && s != "..");
    let rel = plain.then(|| {
        let file = if lower.ends_with(".k7x") {
            clean.to_owned()
        } else {
            format!("{clean}.k7x")
        };
        format!("{PLAYLISTS_DIR}/{file}")
    });
    let root = workspace_dir_of(&h.env, &h.song_path);
    let dir = resolve(&join(&root, PLAYLISTS_DIR));
    match rel {
        Some(rel) if is_within(&dir, &resolve(&join(&root, &rel))) => {
            let abs = resolve(&join(&root, &rel));
            Some((rel, abs))
        }
        _ => {
            h.refuse([format!(
                "'{name}' is not a playlist name — it would leave {PLAYLISTS_DIR}/"
            )]);
            None
        }
    }
}

/// `/^("[^"]*"|\S+)\s+(.+)$/` on the trimmed line: the playlist, unquoted, and the rest.
fn head_and_rest(a: &str) -> Option<(String, String)> {
    let s = js_trim(a);
    let split = |head: &str, rest: &str| {
        let t = rest.trim_start_matches(song_edit::js_text::is_js_space);
        (t.len() < rest.len() && !t.is_empty()).then(|| (head.to_owned(), t.to_owned()))
    };
    let quoted = s
        .strip_prefix('"')
        .and_then(|inner| inner.find('"').map(|end| (&inner[..end], &inner[end + 1..])))
        .and_then(|(head, rest)| split(head, rest));
    if quoted.is_some() {
        return quoted;
    }
    let end = s.find(song_edit::js_text::is_js_space)?;
    let (head, rest) = (&s[..end], &s[end..]);
    let head = head
        .strip_prefix('"')
        .and_then(|x| x.strip_suffix('"'))
        .filter(|_| head.len() >= 2)
        .unwrap_or(head);
    split(head, rest)
}

/// Runs `edit` on the playlist; false is the file's shape refusing it, refused with `refusal`.
fn write(
    h: &mut Host,
    abs: &str,
    verb: &str,
    refusal: &str,
    edit: impl FnOnce(&mut Scope) -> Result<bool, k7x::edit::EditError>,
) -> bool {
    let mut shaped = true;
    let opts = EditOpts {
        create: false,
        raw: false,
    };
    let done = ws_edit(h.fs, abs, opts, |doc| {
        shaped = edit(&mut doc.at(&[]))?;
        Ok(())
    });
    match done {
        Ok(_) if shaped => true,
        Ok(_) => {
            h.refuse([refusal.to_owned()]);
            false
        }
        Err(e) => {
            h.refuse([format!("{verb}: {e}")]);
            false
        }
    }
}

fn items_key() -> Vec<PathSeg> {
    vec![PathSeg::Key("items".to_owned())]
}

fn items_len(doc: &Scope) -> Result<Option<usize>, k7x::edit::EditError> {
    Ok(match doc.get_in(&items_key())? {
        Some(Value::List(l)) => Some(l.len()),
        _ => None,
    })
}

/// `playlist new <name>` — the stub that parses, never an empty file.
pub fn playlist_new(a: &str, h: &mut Host) -> Done {
    let name = js_trim(a);
    if name.is_empty() {
        h.refuse(["usage: playlist new <name>".to_owned()]);
        return Ok(());
    }
    let Some((rel, abs)) = playlist_file(name, h) else {
        return Ok(());
    };
    if ws_exists(&*h.fs, &abs) {
        h.refuse([format!("{rel} is already there")]);
        return Ok(());
    }
    let base = rel.rsplit('/').next().unwrap_or("");
    let title = song_edit::sugar::strip_doc_ext(base).to_owned();
    let body = Value::Map(vec![
        ("title".to_owned(), Value::Str(title.clone())),
        ("items".to_owned(), Value::List(Vec::new())),
    ]);
    let header = [title];
    let opts = StringifyOptions {
        header: &header,
        inline: None,
    };
    let made = stringify(&body, &opts)
        .map_err(song_edit::Error::from)
        .and_then(|text| {
            h.fs.mkdir_all(&song_edit::paths::dirname(&abs))
                .map_err(|e| song_edit::Error::Msg(e.to_string()))?;
            ws_write_text(h.fs, &abs, &text)
        });
    if let Err(e) = made {
        h.refuse([format!("playlist new: {e}")]);
        return Ok(());
    }
    h.log([format!("+ playlist {rel}")]);
    Ok(())
}

/// `playlist song add <playlist> <song> [variant]` — quote the song to name a variant.
pub fn song_add(a: &str, h: &mut Host) -> Done {
    let Some((list, rest)) = head_and_rest(a) else {
        h.refuse([
            "usage: playlist song add <playlist> <song> [variant]  (quote the song to name a variant)".to_owned(),
        ]);
        return Ok(());
    };
    let Some((rel, abs)) = playlist_file(&list, h) else {
        return Ok(());
    };
    if !ws_exists(&*h.fs, &abs) {
        h.refuse([format!("no playlist {rel} — make one with: playlist new <name>")]);
        return Ok(());
    }
    let rest = js_trim(&rest);
    let (song, variant) = match rest
        .strip_prefix('"')
        .and_then(|r| r.find('"').map(|e| (&r[..e], &r[e + 1..])))
    {
        Some((s, v)) => (s.to_owned(), js_trim(v).to_owned()),
        None => (rest.to_owned(), String::new()),
    };
    if song.is_empty() {
        h.refuse(["a playlist line names a song".to_owned()]);
        return Ok(());
    }
    let item = if variant.is_empty() {
        Value::Str(song.clone())
    } else {
        Value::Map(vec![
            ("song".to_owned(), Value::Str(song.clone())),
            ("variant".to_owned(), Value::Str(variant.clone())),
        ])
    };
    let extend =
        format!("{rel}: the way `items` is written there is not a shape this can extend — add the line by hand");
    let ok = write(h, &abs, "playlist song add", &extend, |doc| {
        if doc.get_in(&items_key())?.is_some() && items_len(doc)?.is_none() {
            return Ok(false);
        }
        doc.add_in(&items_key(), &item, WriteOptions::default())?;
        Ok(true)
    });
    if !ok {
        return Ok(());
    }
    let count = ws_text(&*h.fs, &abs)
        .ok()
        .and_then(|t| parse_playlist(&abs, &t))
        .map_or(0, |i| i.len());
    let v = if variant.is_empty() {
        String::new()
    } else {
        format!(" ({variant})")
    };
    let n = match count {
        0 => String::new(),
        1 => " — 1 song".to_owned(),
        c => format!(" — {c} songs"),
    };
    h.log([format!("{rel}: + {song}{v}{n}")]);
    Ok(())
}

struct Open {
    rel: String,
    abs: String,
    items: Vec<Item>,
    args: Vec<String>,
}

/// The playlist the line names, its items and the words after its name; None with the refusal said.
fn open_list(a: &str, h: &mut Host, verb: &str, tail: &str) -> Option<Open> {
    let Some((list, rest)) = head_and_rest(a) else {
        h.refuse([format!("usage: playlist song {verb} <playlist> {tail}")]);
        return None;
    };
    let (rel, abs) = playlist_file(&list, h)?;
    let Ok(text) = ws_text(&*h.fs, &abs) else {
        h.refuse([format!("no playlist {rel} — make one with: playlist new <name>")]);
        return None;
    };
    let items = parse_playlist(&abs, &text).unwrap_or_default();
    if items.is_empty() {
        h.refuse([format!("{rel} names no song")]);
        return None;
    }
    let args = split_words(&rest).into_iter().map(str::to_owned).collect();
    Some(Open { rel, abs, items, args })
}

/// A 1-based position the list has, as a 0-based index.
fn index_of(word: Option<&String>, count: usize) -> Option<usize> {
    let w = word?;
    let n = str_to_number(w);
    (n.fract() == 0.0 && n >= 1.0 && n <= count as f64).then(|| n as usize - 1)
}

fn songs(n: usize) -> String {
    format!("{n} song{}", if n == 1 { "" } else { "s" })
}

fn shape_refusal(rel: &str) -> String {
    format!("{rel}: the way `items` is written there is not a shape this can edit — change the line by hand")
}

/// `playlist song remove <playlist> <n>` — by position.
pub fn song_remove(a: &str, h: &mut Host) -> Done {
    let Some(o) = open_list(a, h, "remove", "<n>") else {
        return Ok(());
    };
    let count = o.items.len();
    let Some(n) = index_of(o.args.first(), count) else {
        h.refuse([format!(
            "{} has {} — name a position between 1 and {count}",
            o.rel,
            songs(count)
        )]);
        return Ok(());
    };
    let ok = write(
        h,
        &o.abs,
        "playlist song remove",
        &shape_refusal(&o.rel),
        |doc| match items_len(doc)? {
            Some(len) if n < len => doc.delete_in(&[PathSeg::Key("items".to_owned()), PathSeg::Index(n)], false),
            _ => Ok(false),
        },
    );
    if ok {
        h.log([format!("{}: − {}", o.rel, label(&o.items[n]))]);
    }
    Ok(())
}

/// `playlist song move <playlist> <from> <to>` — both 1-based.
pub fn song_move(a: &str, h: &mut Host) -> Done {
    let Some(o) = open_list(a, h, "move", "<from> <to>") else {
        return Ok(());
    };
    let count = o.items.len();
    let (Some(from), Some(to)) = (index_of(o.args.first(), count), index_of(o.args.get(1), count)) else {
        h.refuse([format!(
            "{} has {} — both positions go from 1 to {count}",
            o.rel,
            songs(count)
        )]);
        return Ok(());
    };
    let ok = write(
        h,
        &o.abs,
        "playlist song move",
        &shape_refusal(&o.rel),
        |doc| match items_len(doc)? {
            Some(len) if from < len && to < len => {
                doc.move_in(&items_key(), from, to)?;
                Ok(true)
            }
            _ => Ok(false),
        },
    );
    if ok {
        h.log([format!("{}: {} → {}/{count}", o.rel, o.items[from].song, to + 1)]);
    }
    Ok(())
}

/// `playlist stems <playlist> [chain]` — each record separated by the caller, `playlistStems` asked.
pub fn playlist_stems(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    let named = w.get(1..).map(|r| r.join(" ")).unwrap_or_default();
    let chain = if named.is_empty() {
        load_preferences(&*h.fs, &h.env).stems_chain.unwrap_or_default()
    } else {
        named
    };
    let Some(first) = w.first() else {
        h.refuse([
            "usage: playlist stems <playlist> [chain]".to_owned(),
            "       (e.g. playlist stems ete-2026 @k7-core/stems)".to_owned(),
        ]);
        return Ok(());
    };
    if chain.is_empty() {
        h.refuse(["no separating chain: name one, or set this machine's with `system stems <chaîne>`".to_owned()]);
        return Ok(());
    }
    let unquoted = first
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(first)
        .to_owned();
    let Some((rel, abs)) = playlist_file(&unquoted, h) else {
        return Ok(());
    };
    let Ok(text) = ws_text(&*h.fs, &abs) else {
        h.refuse([format!("no playlist {rel} — make one with: playlist new <name>")]);
        return Ok(());
    };
    let items = parse_playlist(&abs, &text).unwrap_or_default();
    if items.is_empty() {
        h.refuse([format!("{rel} names no song")]);
        return Ok(());
    }
    let root = workspace_dir_of(&h.env, &h.song_path);
    let songs: Vec<String> = items.into_iter().map(|i| i.song).collect();
    let n = songs.len();
    h.ask(
        "playlistStems",
        serde_json::json!([{ "root": root, "songs": songs, "chain": chain }]),
    );
    h.log([format!(
        "{rel} through '{chain}': {n} record(s) handed to the separator"
    )]);
    Ok(())
}
