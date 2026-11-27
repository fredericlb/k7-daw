// state.rs — what `GET /git/state` answers and what the workspace bar offers on it, read off the state by one function.
// A reader would get wrong that Push is never offered over a branch that is behind, and Pull never over changes.

use crate::address::remote_name;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "GitMode"))]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    None,
    Local,
    Connected,
}

/// The person's technical branch: when it was last written here, and whether the remote still lacks it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Autosave {
    pub branch: String,
    /// Milliseconds since the epoch.
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub at: u64,
    pub unsent: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FetchError {
    pub code: String,
    pub error: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LastFetch {
    /// Milliseconds since the epoch, like `Date.now()`.
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub error: Option<FetchError>,
}

/// The state of a workspace's repository; a field a route leaves out and a null read the same.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct GitState {
    pub mode: Mode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub remote: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "number"))]
    pub ahead: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "number"))]
    pub behind: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "number"))]
    pub changed: Option<u32>,
    /// Conflicts still without a choice; absent when no merge waits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional = nullable, type = "number"))]
    pub merge: Option<u32>,
    /// True when connected, a fetch worked, and the remote has no branch yet: nobody has pushed.
    #[serde(default, rename = "remoteEmpty", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub remote_empty: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub autosave: Option<Autosave>,
    #[serde(default, rename = "lastFetch", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub last_fetch: Option<LastFetch>,
}

/// Why a button is dimmed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum Blocked {
    PullFirst,
    CommitFirst,
}

/// The last auto-save here and whether it still waits to be sent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BarAutosave {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub at: u64,
    pub unsent: bool,
}

/// What the bar offers; every flag is one button, so a test reads the rules off it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct BarView {
    /// A workspace with no repository has no bar at all.
    pub show: bool,
    pub remote: Option<String>,
    pub branch: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub to_pull: u32,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub to_push: u32,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub changed: u32,
    /// Conflicts waiting for a choice (the bar then offers only the merge), or null when no merge waits.
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub merge: Option<u32>,
    /// Commit has something to commit.
    pub commit: bool,
    /// Push has a commit to send and the branch is not behind.
    pub push: bool,
    pub push_blocked: Option<Blocked>,
    /// Pull has something to bring and nothing uncommitted stands in its way.
    pub pull: bool,
    pub pull_blocked: Option<Blocked>,
    /// The last fetch failed: its sentence, said once by the host; null when it worked or never ran.
    pub fetch_error: Option<String>,
    /// The remote answered and holds nothing yet: an empty workspace here is not a sync that lost projects.
    pub remote_empty: bool,
    /// The editors refuse every write while a merge waits.
    pub read_only: bool,
    pub autosave: Option<BarAutosave>,
}

impl BarView {
    fn hidden() -> BarView {
        BarView {
            show: false,
            remote: None,
            branch: String::new(),
            to_pull: 0,
            to_push: 0,
            changed: 0,
            merge: None,
            commit: false,
            push: false,
            push_blocked: None,
            pull: false,
            pull_blocked: None,
            fetch_error: None,
            remote_empty: false,
            read_only: false,
            autosave: None,
        }
    }
}

/// The bar on a state; a workspace with no repository, or no state yet, has none.
#[must_use]
pub fn bar_view(state: Option<&GitState>) -> BarView {
    let Some(s) = state.filter(|s| s.mode != Mode::None) else {
        return BarView::hidden();
    };
    let (to_pull, to_push, changed) = (s.behind.unwrap_or(0), s.ahead.unwrap_or(0), s.changed.unwrap_or(0));
    let merging = s.merge.is_some();
    let connected = s.mode == Mode::Connected;
    let base = BarView {
        show: true,
        remote: if connected { s.remote.clone() } else { None },
        branch: s.branch.clone().unwrap_or_default(),
        to_pull,
        to_push,
        changed,
        merge: s.merge,
        fetch_error: s
            .last_fetch
            .as_ref()
            .and_then(|f| f.error.as_ref())
            .map(|e| e.error.clone()),
        remote_empty: connected && s.remote_empty == Some(true),
        read_only: merging,
        autosave: s.autosave.as_ref().filter(|_| connected).map(|a| BarAutosave {
            at: a.at,
            unsent: a.unsent,
        }),
        ..BarView::hidden()
    };
    // While a merge waits only its own screen is offered: a commit would be a second history.
    if merging {
        return BarView { show: true, ..base };
    }
    let blocked = |on: bool, why| (connected && on).then_some(why);
    BarView {
        commit: changed > 0,
        push: connected && to_push > 0 && to_pull == 0,
        push_blocked: blocked(to_push > 0 && to_pull > 0, Blocked::PullFirst),
        pull: connected && to_pull > 0 && changed == 0,
        pull_blocked: blocked(to_pull > 0 && changed > 0, Blocked::CommitFirst),
        ..base
    }
}

/// Whether this state locks the writers of its workspace (a merge waits for choices).
#[must_use]
pub fn locked_by(state: Option<&GitState>) -> bool {
    bar_view(state).read_only
}

/// What the workspace menu offers on the git state: connect when there is no remote, else say it and offer to change it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MenuView {
    pub connected: bool,
    pub remote: Option<String>,
}

#[must_use]
pub fn menu_view(state: Option<&GitState>) -> MenuView {
    match state {
        Some(s) if s.mode == Mode::Connected && s.remote.as_deref().is_some_and(|r| !r.is_empty()) => MenuView {
            connected: true,
            remote: s.remote.as_deref().map(remote_name),
        },
        _ => MenuView {
            connected: false,
            remote: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn(f: impl FnOnce(&mut GitState)) -> GitState {
        let mut s = GitState {
            mode: Mode::Connected,
            branch: Some("main".into()),
            remote: Some("https://h/r.git".into()),
            user: None,
            ahead: Some(0),
            behind: Some(0),
            changed: Some(0),
            merge: None,
            remote_empty: None,
            autosave: None,
            last_fetch: None,
        };
        f(&mut s);
        s
    }
    fn local(changed: u32) -> GitState {
        conn(|s| {
            s.mode = Mode::Local;
            s.remote = None;
            s.changed = Some(changed);
        })
    }
    fn bar(s: &GitState) -> BarView {
        bar_view(Some(s))
    }
    fn none(v: &BarView) -> bool {
        !(v.commit || v.push || v.pull)
    }

    #[test]
    fn draws_nothing_without_a_repository() {
        assert!(!bar_view(None).show);
        assert!(
            !bar(&GitState {
                mode: Mode::None,
                ..conn(|_| {})
            })
            .show
        );
    }

    #[test]
    fn carries_the_last_auto_save_of_a_connected_workspace_only() {
        let autosave = Autosave {
            branch: "k7/autosave/ada".into(),
            at: 1_700_000_000_000,
            unsent: true,
        };
        assert_eq!(bar(&conn(|_| {})).autosave, None);
        let on = bar(&conn(|s| s.autosave = Some(autosave.clone())));
        assert_eq!(
            on.autosave,
            Some(BarAutosave {
                at: 1_700_000_000_000,
                unsent: true
            })
        );
        let off = bar(&GitState {
            autosave: Some(autosave),
            ..local(0)
        });
        assert_eq!(off.autosave, None);
    }

    #[test]
    fn offers_commit_over_changes_only() {
        assert!(!bar(&conn(|_| {})).commit);
        assert!(bar(&conn(|s| s.changed = Some(2))).commit);
    }

    #[test]
    fn refuses_a_push_over_a_branch_that_is_behind() {
        let v = bar(&conn(|s| (s.ahead, s.behind) = (Some(1), Some(2))));
        assert!(!v.push);
        assert_eq!(v.push_blocked, Some(Blocked::PullFirst));
        assert!(bar(&conn(|s| s.ahead = Some(1))).push);
    }

    #[test]
    fn refuses_a_pull_over_uncommitted_changes() {
        let v = bar(&conn(|s| (s.behind, s.changed) = (Some(1), Some(3))));
        assert!(!v.pull);
        assert_eq!(v.pull_blocked, Some(Blocked::CommitFirst));
        assert!(bar(&conn(|s| s.behind = Some(1))).pull);
    }

    #[test]
    fn offers_nothing_to_push_or_pull_on_a_local_repository() {
        let v = bar(&local(1));
        assert_eq!(v.remote, None);
        assert!(v.commit && !v.push && !v.pull);
    }

    #[test]
    fn offers_only_the_merge_while_one_waits_and_locks_the_editors() {
        let s = conn(|s| {
            (s.changed, s.behind, s.ahead, s.merge) = (Some(4), Some(1), Some(1), Some(2));
        });
        let v = bar(&s);
        assert_eq!(v.merge, Some(2));
        assert!(none(&v));
        assert!(locked_by(Some(&s)));
        assert!(!locked_by(Some(&conn(|_| {}))));
        assert!(!locked_by(None));
    }

    #[test]
    fn says_a_failed_fetch() {
        let s = conn(|s| {
            s.last_fetch = Some(LastFetch {
                at: 1_760_000_000_000,
                error: Some(FetchError {
                    code: "unreachable".into(),
                    error: "down".into(),
                }),
            });
        });
        assert_eq!(bar(&s).fetch_error.as_deref(), Some("down"));
    }

    #[test]
    fn says_nothing_was_pushed_and_offers_nothing_on_a_clean_clone() {
        let fetched = Some(LastFetch {
            at: 1_760_000_000_000,
            error: None,
        });
        let v = bar(&conn(|s| {
            (s.remote_empty, s.last_fetch) = (Some(true), fetched.clone())
        }));
        assert!(v.remote_empty && none(&v));
        let v = bar(&conn(|s| (s.remote_empty, s.changed) = (Some(true), Some(3))));
        assert!(v.commit);
        assert!(bar(&conn(|s| (s.remote_empty, s.ahead) = (Some(true), Some(2)))).push);
    }

    #[test]
    fn offers_the_first_push_on_a_remote_with_no_branch_yet() {
        let v = bar(&conn(|s| (s.ahead, s.behind) = (Some(3), Some(0))));
        assert!(v.push && !v.pull);
        assert_eq!(v.to_push, 3);
    }

    #[test]
    fn reads_a_state_with_nulls_and_extra_fields() {
        let s: GitState =
            serde_json::from_str(r#"{"mode":"local","remote":null,"merge":null,"x":1,"lastFetch":null}"#).unwrap();
        assert_eq!(s.merge, None);
        assert!(bar(&s).show);
    }

    #[test]
    fn serialises_a_state_without_the_fields_it_does_not_have() {
        let v = serde_json::to_value(GitState {
            mode: Mode::None,
            ..conn(|s| *s = local(0))
        })
        .unwrap();
        assert!(v.get("remote").is_none() && v["mode"] == "none");
    }

    #[test]
    fn menu_connects_or_names_the_remote() {
        let m = menu_view(Some(&conn(|s| s.remote = Some("git@github.com:o/r.git".into()))));
        assert_eq!(
            m,
            MenuView {
                connected: true,
                remote: Some("github.com/o/r".into())
            }
        );
        assert!(!menu_view(Some(&local(0))).connected);
        assert!(!menu_view(None).connected);
        assert!(!menu_view(Some(&conn(|s| s.remote = Some(String::new())))).connected);
    }
}
