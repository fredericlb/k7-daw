//! text.rs — the merge of one file's text: a `.k7x` as key-path ops settled against mine, anything else by lines.
//! What a reader gets wrong: the `.k7x` merge is the relay's own (`settle_k7x`), so a second merge cannot disagree
//! with it; a change that is a whole text, or an op set that does not replay, is merged by lines and read back.

use serde_json::Value;

use super::pending::Seg;
use super::{Conflict, Kind};

/// A text merged: settled, or a plan with conflicts to choose.
pub enum Merged {
    Clean(String),
    K7x {
        start: String,
        aside: Vec<(String, Value)>,
        conflicts: Vec<Conflict>,
    },
    Lines {
        segs: Vec<Seg>,
        conflicts: Vec<Conflict>,
    },
}

fn dotted(op: &Value) -> String {
    op.get("path")
        .and_then(Value::as_array)
        .map(|p| {
            p.iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(".")
        })
        .unwrap_or_default()
}

/// `theirs`'s change from `base`, as key-path ops settled against `mine`: what finds its value applies, the rest is a conflict.
pub fn merge_k7x(path: &str, base: &str, mine: &str, theirs: &str, author: &str) -> Option<Merged> {
    let op = k7_sync::k7x::k7x_op_for(path, base, theirs)?;
    settle_into(path, mine, &op, author)
}

/// One op against mine, as the relay settles it; none when the settle itself fails.
pub fn settle_into(path: &str, mine: &str, op: &Value, author: &str) -> Option<Merged> {
    let settled = k7_sync::k7x::settle_k7x(Some(mine), op).ok()?;
    let Ok(Some(start)) = settled.next else {
        return None;
    };
    if settled.aside.is_empty() {
        return Some(Merged::Clean(start));
    }
    let mut aside = Vec::new();
    let mut conflicts = Vec::new();
    for (n, (one, kept)) in settled.aside.iter().enumerate() {
        let mut sub = one
            .get("ops")
            .and_then(|o| o.get(0))
            .cloned()
            .unwrap_or(Value::Null);
        let both = both_op(&sub, mine, author);
        if let (Some(b), Some(m)) = (&both, sub.as_object_mut()) {
            m.insert("both".into(), b.clone());
        }
        let id = format!("{path}#{n}");
        let theirs = (sub.get("op").and_then(Value::as_str) == Some("set"))
            .then(|| sub.get("k7x").and_then(Value::as_str).map(str::to_string))
            .flatten();
        conflicts.push(Conflict {
            id: id.clone(),
            path: path.to_string(),
            kind: Kind::K7x,
            key_path: Some(dotted(&sub)),
            line: None,
            base: sub.get("was").and_then(Value::as_str).map(str::to_string),
            mine: kept.clone().flatten(),
            theirs,
            both: both.is_some(),
            choice: None,
        });
        aside.push((id, sub));
    }
    Some(Merged::K7x {
        start,
        aside,
        conflicts,
    })
}

/// A JSON string as k7x writes a string value.
fn quoted(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

/// The value text of key field `field` in an item segment `[a="x",b=2]`: a string up to its closing quote, else up to `,` or `]`.
fn field_text<'s>(segment: &'s str, field: &str) -> Option<&'s str> {
    let at = segment.find(&format!("{field}="))? + field.len() + 1;
    let rest = &segment[at..];
    if rest.starts_with('"') {
        let mut escaped = false;
        for (i, c) in rest.char_indices().skip(1) {
            match (c, escaped) {
                ('\\', false) => escaped = true,
                ('"', false) => return Some(&rest[..=i]),
                _ => escaped = false,
            }
        }
        return None;
    }
    let end = rest.find([',', ']']).unwrap_or(rest.len());
    Some(&rest[..end])
}

/// The op that puts their item beside mine under a fresh key, for a whole item both sides added under one key; none for
/// anything else (a value, a field inside an item, a list whose keys are times or notes: another key would change the song).
fn both_op(sub: &Value, mine: &str, author: &str) -> Option<Value> {
    if sub.get("op")?.as_str()? != "set" || !sub.get("was")?.is_null() {
        return None;
    }
    let path: Vec<&str> = sub
        .get("path")?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let (segment, list) = (path.last()?, path.get(path.len().checked_sub(2)?)?);
    if !segment.starts_with('[') {
        return None;
    }
    let keys = k7_sync::k7x::list_keys(list)?;
    let item = sub.get("k7x")?.as_str()?;
    // The first key field that holds a name is renamed; failing that, an `id` that is a number takes the next free one.
    let (field, old, fresh) = keys.iter().find_map(|f| {
        let v = field_text(segment, f)?;
        if v.starts_with('"') {
            let name: String = serde_json::from_str(v).ok()?;
            let mut n = 1;
            loop {
                let tag = if n == 1 {
                    author.to_string()
                } else {
                    format!("{author} {n}")
                };
                let fresh = quoted(&format!("{name} ({tag})"));
                if !mine.contains(&format!("{f}({fresh})")) {
                    return Some((*f, v.to_string(), fresh));
                }
                n += 1;
            }
        }
        if *f == "id" && v.parse::<u64>().is_ok() {
            let top = mine
                .match_indices("id(")
                .filter_map(|(i, _)| {
                    let digits: String = mine[i + 3..]
                        .chars()
                        .take_while(char::is_ascii_digit)
                        .collect();
                    digits.parse::<u64>().ok()
                })
                .max()
                .unwrap_or(0);
            return Some((*f, v.to_string(), (top + 1).to_string()));
        }
        None
    })?;
    let renamed = item.replacen(&format!("{field}({old})"), &format!("{field}({fresh})"), 1);
    if renamed == item {
        return None;
    }
    let seg = segment.replacen(&format!("{field}={old}"), &format!("{field}={fresh}"), 1);
    let mut at: Vec<Value> = path[..path.len() - 1]
        .iter()
        .map(|s| Value::from(*s))
        .collect();
    at.push(Value::from(seg));
    Some(
        serde_json::json!({ "op": "set", "path": at, "k7x": renamed, "was": null, "after": *segment }),
    )
}

/// A text merged by lines, with a conflict for each block the two sides changed differently.
pub fn merge_lines(path: &str, base: &str, mine: &str, theirs: &str, kind: Kind) -> Option<Merged> {
    use k7_sync::diff3::{Block, diff3_merge, lines};
    let blocks = diff3_merge(&lines(mine), &lines(base), &lines(theirs), true).ok()?;
    let mut segs = Vec::new();
    let mut conflicts = Vec::new();
    let mut clean = String::new();
    for b in &blocks {
        match b {
            Block::Ok(l) => {
                let t: String = l.concat();
                clean.push_str(&t);
                segs.push(Seg::Ok { text: t });
            }
            Block::Conflict {
                a, a_index, o, b, ..
            } => {
                let id = format!("{path}#{}", conflicts.len());
                conflicts.push(Conflict {
                    id: id.clone(),
                    path: path.to_string(),
                    kind,
                    key_path: None,
                    line: Some(*a_index + 1),
                    base: Some(o.concat()),
                    mine: Some(a.concat()),
                    theirs: Some(b.concat()),
                    both: true,
                    choice: None,
                });
                segs.push(Seg::Conflict { id });
            }
        }
    }
    if conflicts.is_empty() {
        return Some(Merged::Clean(clean));
    }
    Some(Merged::Lines { segs, conflicts })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The relay's own settle vectors that carry a base (an op with a `was`): the merge answers as the relay does.
    #[test]
    fn every_relay_settle_vector_of_a_k7x_op_with_a_current_text_is_answered_as_the_relay_answers()
    {
        let d: Value =
            serde_json::from_str(include_str!("../../../../fixtures/settle-vectors.json")).unwrap();
        let (texts, ops) = (d["texts"].as_array().unwrap(), d["ops"].as_array().unwrap());
        let mut run = 0;
        for v in d["vectors"].as_array().unwrap() {
            if v["fn"] != "settle" {
                continue;
            }
            let op = &ops[v["args"]["op"].as_u64().unwrap() as usize];
            let Some(cur) = v["args"]["current"].as_u64() else {
                continue;
            };
            if op["kind"] != "k7x"
                || !op["ops"]
                    .as_array()
                    .is_some_and(|o| o.iter().any(|x| x.get("was").is_some()))
            {
                continue;
            }
            let Some(raw) = v["expect"].as_str() else {
                continue;
            };
            let expect: Value = serde_json::from_str(raw).unwrap();
            let Some(ok) = expect.get("ok") else { continue };
            let Some(next) = ok["next"].as_str() else {
                continue;
            };
            let mine = texts[cur as usize].as_str().unwrap();
            let aside = ok["aside"].as_array().unwrap().len();
            match settle_into("f.k7x", mine, op, "x") {
                Some(Merged::Clean(t)) => assert!(aside == 0 && t == next, "{v}"),
                Some(Merged::K7x {
                    start, conflicts, ..
                }) => assert!(aside == conflicts.len() && start == next, "{v}"),
                _ => panic!("no answer for {v}"),
            }
            run += 1;
        }
        assert!(run > 500, "only {run} vectors ran");
    }
}
