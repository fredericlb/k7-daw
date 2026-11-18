// request.rs — a call and an environment as the host sends them: JSON with the TypeScript's own field names.
// A reader would get wrong that the environment is data: the crate reads no process state, so each call or batch carries it.

use crate::confine::Env;
use serde_json::Value as Json;

/// One call of a batch: the name `module.fn` and its positional arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    pub name: String,
    pub args: Vec<Json>,
}

impl Call {
    /// `{fn, args}`, None when `fn` is no string.
    #[must_use]
    pub fn from_json(j: &Json) -> Option<Call> {
        Some(Call {
            name: j.get("fn")?.as_str()?.to_owned(),
            args: j.get("args").and_then(Json::as_array).cloned().unwrap_or_default(),
        })
    }
}

fn pairs(j: &Json) -> Vec<(String, String)> {
    j.as_array()
        .map(|l| {
            l.iter()
                .filter_map(|p| Some((p.get(0)?.as_str()?.to_owned(), p.get(1)?.as_str()?.to_owned())))
                .collect()
        })
        .unwrap_or_default()
}

/// `{studioHome, metaRoot, librariesHome?, modelsRoot?, overlays?, libraryRoots?}` as an environment.
#[must_use]
pub fn env_from_json(j: &Json) -> Env {
    let text = |k: &str| j.get(k).and_then(Json::as_str);
    let mut env = Env::at(text("studioHome").unwrap_or(""), text("metaRoot").unwrap_or(""));
    if let Some(x) = text("librariesHome") {
        x.clone_into(&mut env.libraries_home);
    }
    if let Some(x) = text("modelsRoot") {
        x.clone_into(&mut env.models_root);
    }
    env.overlays = j.get("overlays").map(pairs).unwrap_or_default();
    env.library_roots = j.get("libraryRoots").map(pairs);
    env
}
