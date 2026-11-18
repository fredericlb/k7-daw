//! history.rs — an account's sync history as its browser reads it and as a share names it
//! (`docs/files/history.md`): a room's pages, checkpoints, what changed between two points, a restore.
//! Each answer is `(status, JSON)`; the front's routes only carry them.
//! What a reader gets wrong: a restore is an ordinary batch ordered in the LIVE room, so every replica hears it.

use indexmap::{IndexMap, IndexSet};
use serde_json::{Map, Value, json};

use crate::json::{js_order, utf16_cmp, utf16_len};
use crate::log::{self, LogError};
use crate::path::{is_workspace_name, room_dir};
use crate::relay::{Live, Relay, now_ms};

/// A status and its JSON body.
pub type Answer = (u16, Value);

fn refuse(status: u16, error: impl Into<String>) -> Answer {
    (status, json!({ "error": error.into() }))
}

fn failed(e: LogError) -> Answer {
    match e {
        LogError::Gap(m) => refuse(410, m),
        e => refuse(500, e.message()),
    }
}

/// `intParam`: a string or number that is a whole count, as `Number` reads it.
pub fn int_param(v: Option<&Value>) -> Option<f64> {
    let n = match v? {
        Value::Number(n) => n.as_f64()?,
        Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                0.0
            } else if t
                .bytes()
                .all(|b| b.is_ascii_digit() || b"+-.eE".contains(&b))
            {
                t.parse().ok()?
            } else {
                return None;
            }
        }
        _ => return None,
    };
    (n.fract() == 0.0 && n >= 0.0).then_some(n)
}

/// One entry as the history lists it: who, when, which files, and how many ops were set aside.
fn summary(e: &Value) -> Value {
    let b = &e["batch"];
    let files: IndexSet<String> = b["ops"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|op| crate::protocol::js_string(op.get("file")))
        .collect();
    json!({
        "seq": e["seq"], "ts": b["ts"], "replica": b["replica"], "label": b["label"],
        "files": files.into_iter().collect::<Vec<_>>(),
        "parked": e["parked"].as_array().map_or(0, Vec::len),
    })
}

/// A binary file travels as a ref starting with NUL.
fn is_blob_ref(v: Option<&String>) -> bool {
    v.is_some_and(|t| t.starts_with('\u{0}'))
}

/// Every file whose value differs between two sets of files, by name in `sort()`'s order.
fn changes(from: &IndexMap<String, String>, to: &IndexMap<String, String>) -> Vec<Value> {
    let mut names: Vec<&String> = from
        .keys()
        .chain(to.keys())
        .collect::<IndexSet<_>>()
        .into_iter()
        .collect();
    names.sort_by(|a, b| utf16_cmp(a, b));
    let mut out = Vec::new();
    for f in names {
        let (before, after) = (from.get(f), to.get(f));
        if before == after {
            continue;
        }
        let mut c = Map::new();
        c.insert("file".into(), f.clone().into());
        c.insert(
            "before".into(),
            before.cloned().map_or(Value::Null, Value::String),
        );
        c.insert(
            "after".into(),
            after.cloned().map_or(Value::Null, Value::String),
        );
        if is_blob_ref(before) || is_blob_ref(after) {
            c.insert("binary".into(), true.into());
        }
        out.push(Value::Object(c));
    }
    out
}

/// The files at `seq` as an object.
fn files_value(files: &IndexMap<String, String>) -> IndexMap<String, String> {
    js_order(files.keys())
        .into_iter()
        .map(|k| (k.clone(), files[k.as_str()].clone()))
        .collect()
}

/// The history's view of one room: its live state locked for the whole answer, so no save rolls the log mid-read.
fn with_room(
    relay: &Relay,
    slug: Option<&str>,
    workspace: &str,
    f: impl FnOnce(&Live) -> Answer,
) -> Answer {
    let Some(slug) = slug else {
        return refuse(401, "no user on the session");
    };
    if !is_workspace_name(workspace) {
        return refuse(400, "not a workspace name");
    }
    if !room_dir(&relay.account_dir(slug), workspace).exists() {
        return refuse(404, format!("no history for {workspace}"));
    }
    match relay.room(slug, workspace) {
        Ok(live) => f(&live.lock()),
        Err(e) => refuse(500, e),
    }
}

/// `GET /:workspace?before&limit`: the head, the checkpoints, and a page of entries newest first.
pub fn page(
    relay: &Relay,
    slug: Option<&str>,
    workspace: &str,
    before: Option<&Value>,
    limit: Option<&Value>,
) -> Answer {
    with_room(relay, slug, workspace, |l| {
        let head = l.room().head;
        let before = int_param(before).unwrap_or(head + 1.0);
        let limit = int_param(limit).unwrap_or(50.0).clamp(1.0, 500.0) as usize;
        let read = || -> Result<Value, LogError> {
            let entries: Vec<Value> = log::entries_before(l.dir(), before, limit)?
                .iter()
                .map(summary)
                .collect();
            Ok(
                json!({ "head": head, "checkpoints": log::read_checkpoints(l.dir())?, "entries": entries }),
            )
        };
        read().map_or_else(failed, |v| (200, v))
    })
}

/// `POST /:workspace/checkpoints { name, seq? }`.
pub fn add_checkpoint(
    relay: &Relay,
    slug: Option<&str>,
    workspace: &str,
    body: &Value,
    user: &str,
) -> Answer {
    with_room(relay, slug, workspace, |l| {
        let head = l.room().head;
        let name = body
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        let seq = match body.get("seq") {
            None => Some(head),
            v => int_param(v),
        };
        if name.is_empty() || utf16_len(name) > 120 {
            return refuse(400, "a checkpoint needs a name");
        }
        let Some(seq) = seq.filter(|s| *s <= head) else {
            return refuse(400, "no such point");
        };
        let by = if user.is_empty() { "unknown" } else { user };
        log::add_checkpoint(l.dir(), log::checkpoint(seq, name, now_ms(), by))
            .map_or_else(failed, |cp| (200, cp))
    })
}

/// `GET /:workspace/diff?from&to`.
pub fn diff(
    relay: &Relay,
    slug: Option<&str>,
    workspace: &str,
    from: Option<&Value>,
    to: Option<&Value>,
) -> Answer {
    with_room(relay, slug, workspace, |l| {
        let head = l.room().head;
        let (Some(from), Some(to)) = (int_param(from), int_param(to)) else {
            return refuse(400, "from and to must be points of the log");
        };
        if from > head || to > head {
            return refuse(400, "from and to must be points of the log");
        }
        let js = relay.js();
        let read = || -> Result<Value, LogError> {
            let a = files_value(&log::files_at(js, l.dir(), from)?);
            let b = files_value(&log::files_at(js, l.dir(), to)?);
            Ok(json!({ "from": from, "to": to, "files": changes(&a, &b) }))
        };
        read().map_or_else(failed, |v| (200, v))
    })
}

/// A random version-4 UUID, as `randomUUID` spells one.
fn uuid() -> String {
    let mut b = [0u8; 16];
    let _ = getrandom::fill(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    )
}

/// `POST /:workspace/restore { seq }`: the files at `seq` written back as one batch, ordered like a replica's.
pub fn restore(relay: &Relay, slug: Option<&str>, workspace: &str, body: &Value) -> Answer {
    restore_except(relay, slug, workspace, body, &[])
}

/// `restore`, the files named in `keep` left as they are: a device that does not hold a binary's bytes keeps it.
pub fn restore_except(
    relay: &Relay,
    slug: Option<&str>,
    workspace: &str,
    body: &Value,
    keep: &[String],
) -> Answer {
    let Some(s) = slug else {
        return refuse(401, "no user on the session");
    };
    let checked = with_room(relay, slug, workspace, |_| (200, Value::Null));
    if checked.0 != 200 {
        return checked;
    }
    let live = match relay.room(s, workspace) {
        Ok(l) => l,
        Err(e) => return refuse(500, e),
    };
    let built = live.submit_with(|l| {
        let Some(seq) = int_param(body.get("seq")).filter(|q| *q <= l.room().head) else {
            return Err(refuse(400, "no such point"));
        };
        let target = log::files_at(relay.js(), l.dir(), seq).map_err(failed)?;
        let named = log::read_checkpoints(l.dir())
            .map_err(failed)?
            .into_iter()
            .rfind(|c| c.get("seq").and_then(Value::as_f64) == Some(seq));
        let ops: Vec<Value> = changes(&files_value(l.room().files()), &files_value(&target))
            .into_iter()
            .filter(|c| {
                !c["file"]
                    .as_str()
                    .is_some_and(|f| keep.iter().any(|k| k == f))
            })
            .map(|c| {
                let was = c["before"]
                    .as_str()
                    .map_or(Value::Null, |t| k7_js::number::text_hash(t).into());
                match c["after"].as_str() {
                    None => json!({ "kind": "remove", "file": c["file"], "was": was }),
                    Some(t) => json!({ "kind": "text", "file": c["file"], "text": t, "was": was }),
                }
            })
            .collect();
        if ops.is_empty() {
            return Ok(None);
        }
        let restored: IndexSet<&str> = ops.iter().filter_map(|o| o["file"].as_str()).collect();
        // An alternative to a value the restore replaces is stale: the restore settles it too.
        let resolves: Vec<Value> = l
            .room()
            .conflicts
            .list()
            .filter(|c| {
                c.pointer("/op/file")
                    .and_then(Value::as_str)
                    .is_some_and(|f| restored.contains(f))
            })
            .map(|c| c["id"].clone())
            .collect();
        let label = match named
            .as_ref()
            .and_then(|c| c["name"].as_str())
            .filter(|n| *n != "auto")
        {
            Some(n) => n.to_string(),
            None => k7_js::number::to_string(seq),
        };
        let mut b = json!({
            "id": format!("history-{}", uuid()), "replica": "history", "local": 0, "ts": now_ms(),
            "label": format!("restore to {label}"), "ops": ops,
        });
        if !resolves.is_empty() {
            b["resolves"] = Value::Array(resolves);
        }
        Ok(b.as_object().cloned())
    });
    match built {
        Ok(None) => (200, json!({ "entry": null })),
        Ok(Some(entry)) => (200, json!({ "entry": summary(&entry) })),
        Err(a) => a,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_params() {
        assert_eq!(int_param(Some(&json!("12"))), Some(12.0));
        assert_eq!(int_param(Some(&json!(""))), Some(0.0));
        assert_eq!(int_param(Some(&json!(3))), Some(3.0));
        assert_eq!(int_param(Some(&json!(1.5))), None);
        assert_eq!(int_param(Some(&json!("-1"))), None);
        assert_eq!(int_param(Some(&json!(true))), None);
        assert_eq!(int_param(None), None);
    }

    #[test]
    fn changes_are_sorted_and_name_binaries() {
        let a: IndexMap<String, String> =
            [("b".into(), "1".into()), ("a".into(), "x".into())].into();
        let b: IndexMap<String, String> = [
            ("a".into(), "x".into()),
            ("c".into(), "\u{0}k7fs-blob".into()),
        ]
        .into();
        let got = changes(&a, &b);
        assert_eq!(
            crate::json::stringify(&Value::Array(got)),
            r#"[{"file":"b","before":"1","after":null},{"file":"c","before":null,"after":"\u0000k7fs-blob","binary":true}]"#
        );
    }
}
