// playlist_edit.rs — what a playlist's text says as a k7x edit: a stub, and adding, dropping or reordering one line.
// A reader would get wrong that a refused edit is `Ok(false)` and writes nothing: an `items` that is no list, or a position that is no line.

use crate::error::Res;
use k7x::edit::{PathSeg, Scope, StringifyOptions, Value, WriteOptions, stringify};

fn items_key() -> [PathSeg; 1] {
    [PathSeg::Key("items".to_owned())]
}

/// What a playlist file that does not exist yet says: a stub that PARSES, its name as the header.
///
/// # Errors
/// A value that does not print.
pub fn playlist_stub(name: &str) -> Res<String> {
    let value = Value::object(vec![
        ("title".to_owned(), Value::Str(name.to_owned())),
        ("items".to_owned(), Value::List(vec![])),
    ]);
    let header = [name.to_owned()];
    Ok(stringify(
        &value,
        &StringifyOptions {
            header: &header,
            inline: None,
        },
    )?)
}

/// An item as it is written: the bare song when it names no variant, which is what anyone types.
fn item_value(song: &str, variant: Option<&str>) -> Value {
    match variant.filter(|v| !v.is_empty()) {
        Some(v) => Value::Map(vec![
            ("song".to_owned(), Value::Str(song.to_owned())),
            ("variant".to_owned(), Value::Str(v.to_owned())),
        ]),
        None => Value::Str(song.to_owned()),
    }
}

/// The `items` list as read: Err(()) for a value that is no list, an empty list for none at all.
fn items_of(sc: &Scope) -> Res<Result<Vec<Value>, ()>> {
    Ok(match sc.get_in(&items_key())? {
        None => Ok(vec![]),
        Some(Value::List(l)) => Ok(l),
        Some(_) => Err(()),
    })
}

/// One more line on the end of `items`, made when missing; false when `items` is something other than a list.
///
/// # Errors
/// What the document's edit fails with.
pub fn append_item(sc: &mut Scope, song: &str, variant: Option<&str>) -> Res<bool> {
    if items_of(sc)?.is_err() {
        return Ok(false);
    }
    sc.add_in(&items_key(), &item_value(song, variant), WriteOptions::default())?;
    Ok(true)
}

/// Drops one line by its 0-based position; false when `items` is no list or holds no such line.
///
/// # Errors
/// What the document's edit fails with.
pub fn remove_item(sc: &mut Scope, index: i64) -> Res<bool> {
    let Ok(items) = items_of(sc)? else {
        return Ok(false);
    };
    let Some(i) = usize::try_from(index).ok().filter(|i| *i < items.len()) else {
        return Ok(false);
    };
    Ok(sc.delete_in(&[items_key()[0].clone(), PathSeg::Index(i)], false)?)
}

/// Moves one line to another 0-based position, its comments with it; false when either is not a line.
///
/// # Errors
/// What the document's edit fails with.
pub fn move_item(sc: &mut Scope, from: i64, to: i64) -> Res<bool> {
    let Ok(items) = items_of(sc)? else {
        return Ok(false);
    };
    let ok = |i: i64| usize::try_from(i).ok().filter(|i| *i < items.len());
    let (Some(from), Some(to)) = (ok(from), ok(to)) else {
        return Ok(false);
    };
    sc.move_in(&items_key(), from, to)?;
    Ok(true)
}
