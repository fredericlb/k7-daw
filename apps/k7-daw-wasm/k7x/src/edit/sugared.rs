// sugared.rs — new data applied to a document that may hold short forms, each kept while its expansion is unchanged.
// A reader would get wrong that a short form is written back: one whose expansion changed goes, written in full (song-schema's docs/sugar.md).

use super::apply::apply_data;
use super::document::Document;
use super::layout::{Res, fail};
use super::value::{Value, same};
use crate::ast::PathSeg;

/// One short form of a kind: the key it lives in, the key it expands into, and what says it is written.
pub struct SugarPart {
    pub key: String,
    pub target: String,
    pub present: Box<dyn Fn(&Value) -> bool>,
}

/// A kind's short forms, as song-schema's sugar.ts states them.
pub trait Sugar {
    fn parts(&self) -> &[SugarPart];
    fn expand(&self, raw: &Value) -> Value;
}

fn entry<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    v.get(key)
}

fn same_opt(a: Option<&Value>, b: Option<&Value>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => same(x, y),
        _ => false,
    }
}

fn set(map: &mut Vec<(String, Value)>, key: &str, value: Option<Value>) {
    match (map.iter().position(|(k, _)| k == key), value) {
        (Some(i), Some(v)) => map[i].1 = v,
        (Some(i), None) => {
            map.remove(i);
        }
        (None, Some(v)) => map.push((key.to_owned(), v)),
        (None, None) => {}
    }
}

/// Makes the document's expanded data equal `after` with the fewest edits, keeping every short form whose expansion holds.
///
/// # Errors
/// An `after` that is not a map, or what the document's edits fail with.
pub fn apply_sugared(doc: &mut Document, after: &Value, sugar: Option<&dyn Sugar>) -> Res<()> {
    let Value::Map(after_map) = after else {
        return fail("k7x: a k7x file is a map at its root");
    };
    let raw = doc.to_js()?;
    let Some(sugar) = sugar else {
        return apply_data(doc, &raw, after, &[]);
    };
    let expanded = sugar.expand(&raw);
    if same(&expanded, after) {
        return Ok(());
    }
    let mut base = raw.clone();
    let mut target = after_map.clone();
    for part in sugar.parts() {
        if !(part.present)(&raw) {
            continue;
        }
        if same_opt(entry(&expanded, &part.target), entry(after, &part.target)) {
            set(&mut target, &part.key, entry(&raw, &part.key).cloned());
            if part.target != part.key {
                set(
                    &mut target,
                    &part.target,
                    entry(&raw, &part.target).cloned(),
                );
            }
        } else if part.target != part.key {
            doc.delete_in(&[PathSeg::Key(part.key.clone())], false)?;
            if let Value::Map(m) = &mut base {
                m.retain(|(k, _)| *k != part.key);
            }
        }
    }
    apply_data(doc, &base, &Value::object(target), &[])
}
