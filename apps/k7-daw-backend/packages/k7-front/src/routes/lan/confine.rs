//! confine.rs — what a device on the LAN may ask of the main device's file service: the served project's workspace,
//! and nothing that opens, activates, creates, deletes, syncs or configures. A deny list over the routes, since the
//! surface reads and writes through the same routes the editor does. What a reader gets wrong: `GET /workspace` is
//! refused too, because answering it loads the project into the engine.

use hyper::Method;

/// The project the engine holds, which the door's file mount is confined to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Served {
    pub workspace: String,
    pub path: String,
}

impl Served {
    /// `workspace/path`, the key presence is kept under.
    pub fn key(&self) -> String {
        format!("{}/{}", self.workspace, self.path)
    }
}

/// Route prefixes that move the project, the engine or the account, whoever asks.
const REFUSED: [&str; 14] = [
    "/open",
    "/workspace",
    "/engine",
    "/workspaces",
    "/project",
    "/sync",
    "/git",
    "/preferences",
    "/account",
    "/models",
    "/bench",
    "/auto-freeze",
    "/module-libraries",
    "/libraries",
];

fn under(path: &str, prefix: &str) -> bool {
    path.strip_prefix(prefix)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// Why a request is refused, or None when the door lets it through to the file service.
pub fn refusal(
    method: &Method,
    path: &str,
    workspace: Option<&str>,
    served: Option<&Served>,
) -> Option<&'static str> {
    let Some(served) = served else {
        return Some("no project is open on this device");
    };
    if path.contains("..") || path.contains('\\') || !path.starts_with('/') {
        return Some("not a file route");
    }
    // `/project-doc` and `/project-errors` are reads of the song, `/project` itself is the lifecycle.
    if REFUSED.iter().any(|p| under(path, p)) {
        return Some("a device on the network cannot open, create or configure projects here");
    }
    if workspace != Some(served.workspace.as_str()) {
        return Some("outside the workspace of the project this device serves");
    }
    if !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
        && path.starts_with("/export")
    {
        return Some("not a file route");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn served() -> Served {
        Served {
            workspace: "band".into(),
            path: "mix.k7".into(),
        }
    }

    fn asked(method: Method, path: &str, ws: Option<&str>) -> Option<&'static str> {
        refusal(&method, path, ws, Some(&served()))
    }

    #[test]
    fn the_served_workspace_is_the_only_one_reached() {
        assert!(asked(Method::GET, "/revision", Some("band")).is_none());
        assert!(asked(Method::POST, "/command", Some("band")).is_none());
        assert!(asked(Method::GET, "/revision", Some("other")).is_some());
        assert!(asked(Method::GET, "/revision", None).is_some());
    }

    #[test]
    fn what_opens_creates_or_configures_is_refused_even_in_the_workspace() {
        for (m, p) in [
            (Method::POST, "/open"),
            (Method::GET, "/workspace"),
            (Method::POST, "/engine/project"),
            (Method::POST, "/engine/unload"),
            (Method::POST, "/workspaces"),
            (Method::GET, "/workspaces/surfaces"),
            (Method::POST, "/project/delete"),
            (Method::PUT, "/preferences/vim"),
            (Method::GET, "/git/status"),
            (Method::GET, "/account/live-chains"),
        ] {
            assert!(asked(m.clone(), p, Some("band")).is_some(), "{m} {p}");
        }
        // A prefix is a path segment, not a string prefix: `/project-doc` is a read of the song.
        assert!(asked(Method::GET, "/project-doc", Some("band")).is_none());
        assert!(asked(Method::GET, "/workspace-tree", Some("band")).is_none());
    }

    #[test]
    fn nothing_is_served_nothing_is_reached() {
        assert!(refusal(&Method::GET, "/revision", Some("band"), None).is_some());
        assert!(asked(Method::GET, "/../etc", Some("band")).is_some());
    }
}
