//! settle.rs — one op against the room's text: what applies and what is set aside (`settleFileOp`), and the text
//! after it (`applyFileOp`). A whole text, its diff3 merge (`diff3.rs`) and a removal are decided here; a k7x op
//! goes to `crate::k7x` through `Js`, which `Native` implements and a test's stand-in replaces.
//! What a reader gets wrong: `Js` is a seam for tests and the capture's memo; nothing outside the process answers it.

use serde_json::{Map, Value};

pub use k7_js::JsError;

use crate::bases::{TextBases, mergeable};
use crate::diff3;

/// One op of `Room.submit`, settled against the room's text of its file.
#[derive(Debug, Clone, PartialEq)]
pub struct Settled {
    /// What still finds the value it replaced (possibly merged or trimmed), or `None`: nothing applies.
    pub apply: Option<Value>,
    /// Each op set aside, with `kept`: `None` when `settleFileOp` leaves it out, `Some(None)` for its `null`.
    pub aside: Vec<(Value, Option<Option<String>>)>,
    /// `applyFileOp(current, apply)`, `Err` when it throws (the room parks `apply`); `Ok(current)` when `apply` is `None`.
    pub next: Result<Option<String>, ()>,
}

/// The k7x half of the relay's merge: `settleFileOp` then `applyFileOp`, and `applyFileOp` alone.
pub trait Js: Send + Sync {
    /// `settleFileOp(current, op, baseOf)` for a k7x op; `bases` is the room's versions of a text, unused by k7x.
    fn settle(
        &self,
        current: Option<String>,
        op: Value,
        bases: Vec<String>,
    ) -> Result<Settled, JsError>;
    /// `applyFileOp(current, op)`; a throw is Err, `NeedsUpdate` named as such.
    fn apply(&self, current: Option<String>, op: Value) -> Result<Option<String>, JsError>;
}

/// The native k7x half: the editor of `apps/k7-daw-frontend/packages/k7x`, no thread and no state, so one is shared by every room.
#[derive(Debug, Clone, Copy, Default)]
pub struct Native;

impl Js for Native {
    fn settle(
        &self,
        current: Option<String>,
        op: Value,
        _bases: Vec<String>,
    ) -> Result<Settled, JsError> {
        crate::k7x::settle_k7x(current.as_deref(), &op)
    }

    fn apply(&self, current: Option<String>, op: Value) -> Result<Option<String>, JsError> {
        let ops = op.get("ops").and_then(Value::as_array).ok_or_else(|| {
            error(
                "TypeError",
                "Cannot read properties of undefined (reading 'find')",
            )
        })?;
        let current = current.ok_or_else(|| error("Error", "k7x ops on a missing file"))?;
        crate::k7x::apply_k7x(&current, ops).map(Some)
    }
}

fn error(name: &str, message: impl Into<String>) -> JsError {
    JsError {
        name: name.into(),
        message: message.into(),
    }
}

/// `applyFileOp`: a text or a removal here, a k7x op through `js`.
pub fn apply(js: &dyn Js, current: Option<&str>, op: &Value) -> Result<Option<String>, JsError> {
    let file = crate::protocol::js_string(op.get("file"));
    match op.get("kind").and_then(Value::as_str) {
        Some("remove") => Ok(None),
        Some("text") => Ok(op.get("text").and_then(Value::as_str).map(str::to_string)),
        Some("k7x") if current.is_none() => {
            Err(error("Error", format!("{file}: k7x ops on a missing file")))
        }
        Some("k7x") => js.apply(current.map(str::to_string), op.clone()),
        _ => {
            let kind = crate::protocol::js_string(op.get("kind"));
            if crate::protocol::is_retired(op.get("kind")) {
                Err(error(
                    "Error",
                    format!("{file}: op kind \"{kind}\" retired, never replayed"),
                ))
            } else {
                Err(error(
                    "NeedsUpdate",
                    format!("{file}: op kind \"{kind}\" unknown here: update k7"),
                ))
            }
        }
    }
}

fn applies(op: &Value) -> Settled {
    // A kind `apply` cannot replay is a throw there, so the room parks the op.
    let next = match op.get("kind").and_then(Value::as_str) {
        Some("text") => Ok(op.get("text").and_then(Value::as_str).map(str::to_string)),
        Some("remove") => Ok(None),
        _ => Err(()),
    };
    Settled {
        apply: Some(op.clone()),
        aside: Vec::new(),
        next,
    }
}

fn aside(op: &Value) -> Settled {
    Settled {
        apply: None,
        aside: vec![(op.clone(), None)],
        next: Ok(None),
    }
}

/// `settleFileOp(current, op, baseOf)` with the room's bases.
pub fn settle(
    js: &dyn Js,
    current: Option<&str>,
    op: &Value,
    bases: &TextBases,
) -> Result<Settled, JsError> {
    let kind = op.get("kind").and_then(Value::as_str);
    if kind == Some("k7x") {
        return js.settle(current.map(str::to_string), op.clone(), Vec::new());
    }
    let text = if kind == Some("text") {
        op.get("text").and_then(Value::as_str)
    } else {
        None
    };
    let Some(was) = op.get("was") else {
        return Ok(applies(op));
    };
    if current == text {
        return Ok(applies(op));
    }
    let now = current.map(k7_js::number::text_hash);
    let matches = match (was, &now) {
        (Value::Null, None) => true,
        (Value::String(w), Some(n)) => w == n,
        _ => false,
    };
    if matches {
        return Ok(applies(op));
    }
    if kind != Some("text") {
        return Ok(aside(op));
    }
    // `settleText`: only a whole text over a mergeable one, whose base the room still keeps, goes to diff3.
    let file = op.get("file").and_then(Value::as_str).unwrap_or("");
    let hash = was.as_str().filter(|w| !w.is_empty());
    let Some(hash) = hash.filter(|_| mergeable(file, current) && mergeable(file, text)) else {
        return Ok(aside(op));
    };
    let (Some(base), Some(mine), Some(theirs)) = (bases.find(file, hash), current, text) else {
        return Ok(aside(op));
    };
    // A merge past the work budget is parked, as one with a conflict is.
    let Ok(Some(merged)) = diff3::merge_texts(base, mine, theirs) else {
        return Ok(aside(op));
    };
    let mut out = Map::new();
    out.insert("kind".into(), "text".into());
    if let Some(f) = op.get("file") {
        out.insert("file".into(), f.clone());
    }
    out.insert("text".into(), merged.clone().into());
    out.insert("was".into(), k7_js::number::text_hash(mine).into());
    Ok(Settled {
        apply: Some(Value::Object(out)),
        aside: Vec::new(),
        next: Ok(Some(merged)),
    })
}

#[cfg(test)]
pub(crate) mod fake {
    //! A stand-in for the k7x half: k7x ops are whole-file `set`s of `[]`, and a merge never succeeds.
    use super::*;

    pub struct FakeJs;

    impl Js for FakeJs {
        fn settle(
            &self,
            current: Option<String>,
            op: Value,
            _bases: Vec<String>,
        ) -> Result<Settled, JsError> {
            if op["kind"] != "k7x" {
                return Ok(aside(&op));
            }
            let next = self.apply(current, op.clone()).map_err(|_| ());
            Ok(Settled {
                apply: Some(op),
                aside: Vec::new(),
                next,
            })
        }

        fn apply(&self, _current: Option<String>, op: Value) -> Result<Option<String>, JsError> {
            let last = op["ops"]
                .as_array()
                .and_then(|o| o.last())
                .cloned()
                .unwrap_or(Value::Null);
            match last["k7x"].as_str() {
                Some(t) => Ok(Some(t.to_string())),
                None => Err(error("Error", "fake: no k7x")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::FakeJs;
    use super::*;
    use k7_js::number::text_hash;
    use serde_json::json;

    #[test]
    fn whole_texts_and_removals_settle_natively() {
        let b = TextBases::default();
        let text =
            |was: Value| json!({ "kind": "text", "file": "a.md", "text": "new\n", "was": was });
        let s = settle(
            &FakeJs,
            Some("old\n"),
            &json!({ "kind": "text", "file": "a.md", "text": "n" }),
            &b,
        )
        .unwrap();
        assert_eq!(s.next, Ok(Some("n".into())));
        assert!(
            settle(&FakeJs, Some("old\n"), &text(text_hash("old\n").into()), &b)
                .unwrap()
                .apply
                .is_some()
        );
        assert!(
            settle(&FakeJs, None, &text(Value::Null), &b)
                .unwrap()
                .apply
                .is_some()
        );
        assert!(
            settle(&FakeJs, Some("new\n"), &text("zz".into()), &b)
                .unwrap()
                .apply
                .is_some()
        );
        let moved = settle(
            &FakeJs,
            Some("other\n"),
            &text(text_hash("old\n").into()),
            &b,
        )
        .unwrap();
        assert_eq!(moved.apply, None);
        assert_eq!(moved.aside.len(), 1);
        let rm = json!({ "kind": "remove", "file": "a.md", "was": "zz" });
        assert_eq!(
            settle(&FakeJs, Some("x"), &rm, &b).unwrap().aside[0].1,
            None
        );
        let rm_absent = json!({ "kind": "remove", "file": "a.md", "was": "zz" });
        assert!(
            settle(&FakeJs, None, &rm_absent, &b)
                .unwrap()
                .apply
                .is_some()
        );
    }

    #[test]
    fn apply_names_what_it_cannot_replay() {
        let k = json!({ "kind": "k7x", "file": "a.k7x", "ops": [] });
        assert_eq!(
            apply(&FakeJs, None, &k).unwrap_err().message,
            "a.k7x: k7x ops on a missing file"
        );
        let y = json!({ "kind": "yaml", "file": "a.yaml" });
        assert_eq!(apply(&FakeJs, None, &y).unwrap_err().name, "Error");
        let z = json!({ "kind": "zip", "file": "a" });
        assert_eq!(apply(&FakeJs, None, &z).unwrap_err().name, "NeedsUpdate");
    }
}
