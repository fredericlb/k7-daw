// node_tap.rs — a rack node's taps: note, sample, add, insert, rename, mute, out, its chain and remove, after verbs/nodeTap.ts.
// A reader would get wrong where a tap lives: in the NODE's file, `rack.symbols`, never in the rack module's.

use super::node_shared::{library_files, split_args};
use super::shared::Done;
use crate::host::Host;
use k7x::edit::js_number;
use song_edit::js_text::str_to_number;
use song_edit::rack_taps::{TapChange, TapNear, add_tap, kit_of, remove_tap, rename_tap, set_tap, set_tap_out};
use song_edit::tap_chain::{add_tap_chain_node, move_tap_chain_node, remove_tap_chain_node};

/// `tapEdit`: one tap write, its line said and a reload, or the write's message refused.
fn tap_edit<T>(h: &mut Host, write: impl FnOnce(&mut Host) -> song_edit::Res<T>, said: String) -> Done {
    match write(h) {
        Ok(_) => {
            h.log([said]);
            h.reload();
        }
        Err(e) => h.refuse([format!("node tap: {e}")]),
    }
    Ok(())
}

fn word(w: &[String], i: usize) -> &str {
    w.get(i).map_or("", String::as_str)
}

fn is_int(n: f64) -> bool {
    n.is_finite() && n.fract() == 0.0
}

/// `node tap note <node> <symbol> <0-127>`.
pub fn tap_note(a: &str, h: &mut Host) -> Done {
    let w = split_args(a);
    let (node, symbol) = (word(&w, 0).to_owned(), word(&w, 1).to_owned());
    let note = w.get(2).map_or(f64::NAN, |s| str_to_number(s));
    if node.is_empty() || symbol.is_empty() || !is_int(note) || !(0.0..=127.0).contains(&note) {
        h.refuse(["usage: node tap note <node> <symbol> <0-127>".to_owned()]);
        return Ok(());
    }
    let said = format!("{node}: {symbol} on note {}", js_number(note));
    tap_edit(
        h,
        |h| {
            let change = TapChange {
                note: Some(note),
                ..TapChange::default()
            };
            set_tap(h.fs, &h.env, &h.song_path, &node, &symbol, &change)
        },
        said,
    )
}

/// `node tap sample <node> <symbol> <@library/path | none>` — a file the tap's picker offers, or none.
pub fn tap_sample(a: &str, h: &mut Host) -> Done {
    let w = split_args(a);
    let (node, symbol, file) = (word(&w, 0).to_owned(), word(&w, 1).to_owned(), word(&w, 2).to_owned());
    if node.is_empty() || symbol.is_empty() || file.is_empty() {
        h.refuse(["usage: node tap sample <node> <symbol> <@library/path | none>".to_owned()]);
        return Ok(());
    }
    let clearing = file == "none";
    if !clearing && !library_files(h, "samples").contains(&file) {
        h.refuse([format!(
            "no '{file}' in the libraries' or the project's samples/ — drop it into one first"
        )]);
        return Ok(());
    }
    let said = if clearing {
        format!("{node}: {symbol} has no sample")
    } else {
        format!("{node}: {symbol} plays {file}")
    };
    tap_edit(
        h,
        |h| {
            let change = TapChange {
                sample: Some((!clearing).then(|| file.clone())),
                ..TapChange::default()
            };
            set_tap(h.fs, &h.env, &h.song_path, &node, &symbol, &change)
        },
        said,
    )
}

fn add_said(h: &mut Host, verb: &str, node: &str, symbol: Option<&str>, near: Option<TapNear>, tail: &str) -> Done {
    let dir = h.song_path.clone();
    match add_tap(h.fs, &h.env, &dir, node, symbol, near.as_ref()) {
        Ok((made, note)) => {
            h.log([format!(
                "{node}: tap {made} on note {}{tail} — pick its sample",
                js_number(note)
            )]);
            h.reload();
        }
        Err(e) => h.refuse([format!("{verb}: {e}")]),
    }
    Ok(())
}

/// `node tap add <node> [symbol]` — after the kit's highest note.
pub fn tap_add(a: &str, h: &mut Host) -> Done {
    let w = split_args(a);
    let node = word(&w, 0).to_owned();
    if node.is_empty() {
        h.refuse(["usage: node tap add <node> [symbol]".to_owned()]);
        return Ok(());
    }
    add_said(h, "node tap add", &node, w.get(1).map(String::as_str), None, "")
}

/// `node tap at <node> <note> [symbol]` — an empty cell of the octave grid.
pub fn tap_at(a: &str, h: &mut Host) -> Done {
    let w = split_args(a);
    let node = word(&w, 0).to_owned();
    let note = w.get(1).map_or(f64::NAN, |s| str_to_number(s));
    if node.is_empty() || !is_int(note) {
        h.refuse(["usage: node tap at <node> <note> [symbol]".to_owned()]);
        return Ok(());
    }
    add_said(
        h,
        "node tap at",
        &node,
        w.get(2).map(String::as_str),
        Some(TapNear::Note(note)),
        "",
    )
}

/// `node tap insert <node> <before|after> <tap> [symbol]` — the new pad beside another.
pub fn tap_insert(a: &str, h: &mut Host) -> Done {
    let w = split_args(a);
    let (node, side, beside) = (word(&w, 0).to_owned(), word(&w, 1).to_owned(), word(&w, 2).to_owned());
    if node.is_empty() || (side != "before" && side != "after") || beside.is_empty() {
        h.refuse(["usage: node tap insert <node> <before|after> <tap> [symbol]".to_owned()]);
        return Ok(());
    }
    let near = TapNear::Beside {
        symbol: beside.clone(),
        before: side == "before",
    };
    let tail = format!(", {side} {beside}");
    add_said(
        h,
        "node tap insert",
        &node,
        w.get(3).map(String::as_str),
        Some(near),
        &tail,
    )
}

/// `node tap rename <node> <symbol> <new name>`.
pub fn tap_rename(a: &str, h: &mut Host) -> Done {
    let w = split_args(a);
    let (node, symbol, name) = (word(&w, 0).to_owned(), word(&w, 1).to_owned(), word(&w, 2).to_owned());
    if node.is_empty() || symbol.is_empty() || name.is_empty() {
        h.refuse(["usage: node tap rename <node> <symbol> <new name>".to_owned()]);
        return Ok(());
    }
    let said = format!("{node}: tap {symbol} is now {name}");
    tap_edit(
        h,
        |h| rename_tap(h.fs, &h.env, &h.song_path, &node, &symbol, &name),
        said,
    )
}

/// `node tap mute <node> <symbol> [on|off]` — a toggle without the word; the tap stays, silent.
pub fn tap_mute(a: &str, h: &mut Host) -> Done {
    let w = split_args(a);
    let (node, symbol, said_word) = (word(&w, 0).to_owned(), word(&w, 1).to_owned(), word(&w, 2).to_owned());
    if node.is_empty() || symbol.is_empty() || (!said_word.is_empty() && said_word != "on" && said_word != "off") {
        h.refuse(["usage: node tap mute <node> <symbol> [on|off]".to_owned()]);
        return Ok(());
    }
    let on = if said_word.is_empty() {
        match kit_of(&*h.fs, &h.env, &h.song_path, &node) {
            Ok(kit) => !kit.symbols.get(&symbol).and_then(|s| s.mute).unwrap_or(false),
            Err(e) => {
                h.refuse([format!("node tap mute: {e}")]);
                return Ok(());
            }
        }
    } else {
        said_word == "on"
    };
    let said = format!("{node}: {symbol} {}", if on { "muted" } else { "unmuted" });
    tap_edit(
        h,
        |h| {
            let change = TapChange {
                mute: Some(on),
                ..TapChange::default()
            };
            set_tap(h.fs, &h.env, &h.song_path, &node, &symbol, &change)
        },
        said,
    )
}

/// `node tap out <node> <tap> <subchain|none>` — the tap leaves by an output of its own.
pub fn tap_out(a: &str, h: &mut Host) -> Done {
    let w = split_args(a);
    let (node, symbol, sub) = (word(&w, 0).to_owned(), word(&w, 1).to_owned(), word(&w, 2).to_owned());
    if node.is_empty() || symbol.is_empty() || sub.is_empty() {
        h.refuse(["usage: node tap out <node> <tap> <subchain|none>".to_owned()]);
        return Ok(());
    }
    let to = (sub != "none").then_some(sub);
    let said = match &to {
        Some(s) => format!("{node}: {symbol} leaves for {s}"),
        None => format!("{node}: {symbol} back on the main output"),
    };
    tap_edit(
        h,
        |h| set_tap_out(h.fs, &h.env, &h.song_path, &node, &symbol, to.as_deref()),
        said,
    )
}

/// `node tap chain add <node> <tap> <module>` — the tap's own chain grows by one, made the first time.
pub fn tap_chain_add(a: &str, h: &mut Host) -> Done {
    let w = split_args(a);
    let (node, symbol, r) = (word(&w, 0).to_owned(), word(&w, 1).to_owned(), word(&w, 2).to_owned());
    if node.is_empty() || symbol.is_empty() || r.is_empty() {
        h.refuse(["usage: node tap chain add <node> <tap> <module>".to_owned()]);
        return Ok(());
    }
    let said = format!("{node}: {symbol} goes through {r}");
    tap_edit(
        h,
        |h| add_tap_chain_node(h.fs, &h.env, &h.song_path, &node, &symbol, &r, h.new_id),
        said,
    )
}

/// `node tap chain remove <node> <tap> <node-id>` — the last one takes the subchain with it.
pub fn tap_chain_remove(a: &str, h: &mut Host) -> Done {
    let w = split_args(a);
    let (node, symbol, id) = (word(&w, 0).to_owned(), word(&w, 1).to_owned(), word(&w, 2).to_owned());
    if node.is_empty() || symbol.is_empty() || id.is_empty() {
        h.refuse(["usage: node tap chain remove <node> <tap> <node-id>".to_owned()]);
        return Ok(());
    }
    let dir = h.song_path.clone();
    match remove_tap_chain_node(h.fs, &h.env, &dir, &node, &symbol, &id) {
        Ok(emptied) => {
            h.log([format!("{node}: removed from {symbol}'s chain")]);
            h.reload();
            if emptied {
                h.log([format!(
                    "{node}: {symbol}'s chain is empty — the tap is back on the main output"
                )]);
            }
        }
        Err(e) => h.refuse([format!("node tap: {e}")]),
    }
    Ok(())
}

/// `node tap chain move <node> <tap> <node-id> <up|down>`.
pub fn tap_chain_move(a: &str, h: &mut Host) -> Done {
    let w = split_args(a);
    let (node, symbol, id, way) = (
        word(&w, 0).to_owned(),
        word(&w, 1).to_owned(),
        word(&w, 2).to_owned(),
        word(&w, 3),
    );
    if node.is_empty() || symbol.is_empty() || id.is_empty() || (way != "up" && way != "down") {
        h.refuse(["usage: node tap chain move <node> <tap> <node-id> <up|down>".to_owned()]);
        return Ok(());
    }
    let by = if way == "up" { -1 } else { 1 };
    let said = format!("{node}: {symbol}'s chain reordered");
    tap_edit(
        h,
        |h| move_tap_chain_node(h.fs, &h.env, &h.song_path, &node, &symbol, &id, by),
        said,
    )
}

/// `node tap remove <node> <symbol>`.
pub fn tap_remove(a: &str, h: &mut Host) -> Done {
    let w = split_args(a);
    let (node, symbol) = (word(&w, 0).to_owned(), word(&w, 1).to_owned());
    if node.is_empty() || symbol.is_empty() {
        h.refuse(["usage: node tap remove <node> <symbol>".to_owned()]);
        return Ok(());
    }
    let said = format!("{node}: tap {symbol} removed");
    tap_edit(h, |h| remove_tap(h.fs, &h.env, &h.song_path, &node, &symbol), said)
}
