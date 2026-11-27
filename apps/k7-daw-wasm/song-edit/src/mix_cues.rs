// mix_cues.rs — the desk's cue sheet `mix.k7x` written: one song's cue points set in the sheet, comments and other cues kept.
// A reader would get wrong what a song with no cue left becomes: a bare ref, or nothing when the sheet never named it.

use crate::error::Res;
use k7x::edit::{PathSeg, Scope, Value, WriteOptions};
use song::files::desk::MARKER_SLOTS;

fn key(k: &str) -> PathSeg {
    PathSeg::Key(k.to_owned())
}

fn cue_at(i: usize, field: &str) -> Vec<PathSeg> {
    vec![key("cues"), PathSeg::Index(i), key(field)]
}

fn is_ref(item: &Value, r: &str) -> bool {
    match item {
        Value::Map(_) => matches!(item.get("ref"), Some(Value::Str(s)) if s == r),
        Value::Str(s) => s == r,
        _ => false,
    }
}

/// Sets one song's cue points in the sheet, appending a ref it does not name; a song left with none goes back to a bare ref.
///
/// # Errors
/// What the document's edits fail with.
pub fn with_markers(sc: &mut Scope, reference: &str, markers: &[Option<f64>]) -> Res<()> {
    let mut kept: Vec<Option<f64>> = markers.iter().take(MARKER_SLOTS).copied().collect();
    while kept.last().is_some_and(Option::is_none) {
        kept.pop();
    }
    let list = match sc.get_in(&[key("cues")])? {
        Some(Value::List(l)) => l,
        _ => {
            sc.set_in(&[key("cues")], &Value::List(vec![]), WriteOptions::default())?;
            vec![]
        }
    };
    let at = list.iter().position(|item| is_ref(item, reference));
    // Cues on one line: eight numbers down the page would bury the ref they belong to.
    let inline = WriteOptions { inline: true };
    let points = Value::List(kept.iter().map(|m| m.map_or(Value::Null, Value::Num)).collect());
    if let Some(i) = at
        && let Value::Map(entry) = &list[i]
    {
        let others = entry.iter().any(|(k, _)| k != "ref" && k != "markers");
        if !kept.is_empty() {
            sc.set_in(&cue_at(i, "markers"), &points, inline)?;
        } else if others {
            sc.delete_in(&cue_at(i, "markers"), false)?;
        } else {
            let bare = Value::Str(reference.to_owned());
            sc.set_in(&[key("cues"), PathSeg::Index(i)], &bare, WriteOptions::default())?;
        }
        return Ok(());
    }
    if kept.is_empty() {
        if at.is_none() {
            sc.add_in(&[key("cues")], &Value::Str(reference.to_owned()), WriteOptions::default())?;
        }
        return Ok(());
    }
    let cued = Value::Map(vec![
        ("ref".to_owned(), Value::Str(reference.to_owned())),
        ("markers".to_owned(), points),
    ]);
    match at {
        Some(i) => sc.set_in(&[key("cues"), PathSeg::Index(i)], &cued, inline)?,
        None => sc.add_in(&[key("cues")], &cued, inline)?,
    }
    Ok(())
}
