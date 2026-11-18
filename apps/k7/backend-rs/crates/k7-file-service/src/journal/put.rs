//! put.rs — `PUT /file` judged before it writes, sentence for sentence in the TypeScript's order (1760): the base read
//! and compared first, then the write's own guards. What a reader gets wrong: an fs failure of the write itself is not
//! predicted here; the plan answers `Write` and the write finds it.

use serde_json::{Value, json};

use crate::confine::Disk;
use crate::entries::guards::{assert_library_writable, refuse_yaml};
use crate::filter::is_binary;
use crate::paths::basename;
use crate::read::{ReadError, read_text};
use crate::space::{Declined, PathError, Space};
use crate::text_hash::text_hash;
use crate::tree::Limits;

/// What a save comes to before anything is written.
#[derive(Clone, Debug, PartialEq)]
pub enum PutPlan {
    Write {
        abs: String,
    },
    /// The route's answer: `400`, `404` or `409` and its body.
    Refused {
        status: u16,
        body: Value,
    },
    /// Past a limit the native side holds: declined.
    Declined(Declined),
}

fn refuse(status: u16, error: String) -> PutPlan {
    PutPlan::Refused {
        status,
        body: json!({ "error": error }),
    }
}

/// The words `entryFailure` gives an fs failure: no folder of the server (1791).
pub fn io_words(code: &str) -> (u16, String) {
    if code == "ENOENT" {
        (404, "no such file or folder".into())
    } else {
        (400, format!("the file could not be changed ({code})"))
    }
}

/// `recordChange`'s mutation for `PUT /file`: `assertBase`, then `write`'s guards.
pub fn plan_put(
    space: &Space<'_>,
    path: &str,
    content: &str,
    base: Option<&str>,
    limits: &Limits,
) -> PutPlan {
    if let Some(base) = base {
        let now = match read_text(space, path, limits) {
            Ok(t) => Some(t.content),
            Err(ReadError::NotFound { .. }) => None,
            Err(ReadError::IsFolder { rel }) => {
                return refuse(400, format!("'{rel}' is a folder, not a file"));
            }
            Err(ReadError::Path(e)) => return refuse(400, e.to_string()),
            Err(ReadError::Io { code }) => {
                let (status, words) = io_words(code);
                return refuse(status, words);
            }
            Err(ReadError::Declined(d)) => return PutPlan::Declined(d),
        };
        if let Some(now) = now
            && now != content
            && text_hash(&now) != base
        {
            return PutPlan::Refused {
                status: 409,
                body: json!({ "error": format!("{path} changed on disk since it was opened \u{2014} not overwritten"), "stale": true }),
            };
        }
    }
    let abs = match space.safe_path(path, &Disk) {
        Ok(a) => a,
        Err(PathError::Declined(d)) => return PutPlan::Declined(d),
        Err(e) => return refuse(400, e.to_string()),
    };
    if let Err(e) = assert_library_writable(space, &abs, "write") {
        return refuse(400, e.to_string());
    }
    if is_binary(&basename(&abs)) {
        return refuse(400, "cannot edit a binary file".into());
    }
    if let Err(e) = refuse_yaml(path) {
        return refuse(400, e.to_string());
    }
    PutPlan::Write { abs }
}
