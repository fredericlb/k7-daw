// ops/git.rs — the git.* operations: the workspace bar's offers, a save's files, a merge's choices, an address's host and name.
// A reader would get wrong that none is hot: each runs when a state arrives or a field changes, so JSON is the right ABI.

use crate::error::WasmError;
use crate::ops::write;
use git_rules::{
    Arranged, Conflict, Entry, GitState, GithubRepo, RoamEntry, RoamState, address, bar_view, can_finish, commit_view,
    menu_view, offered, offered_roam, pushed_ago, remaining, save_when,
};
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StateArgs {
    pub state: Option<GitState>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UrlArgs {
    pub url: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct EntriesArgs {
    pub entries: Vec<Entry>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CloneNameArgs {
    pub name: String,
    pub taken: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct VersionArgs {
    pub version: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PortArgs {
    pub typed: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct OfferedArgs {
    pub conflict: Conflict,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProgressArgs {
    pub conflicts: Vec<Conflict>,
    /// The ids of the conflicts that have a choice.
    pub chosen: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ArrangeArgs {
    pub repos: Vec<GithubRepo>,
    pub filter: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AgoArgs {
    pub iso: Option<String>,
    pub now: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RoamArgs {
    pub state: RoamState,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct WhenArgs {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub epoch_ms: i64,
    /// Minutes the caller's clock stands ahead of UTC.
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub offset_min: i32,
}

/// The git family's requests: `{"op": "git.bar", "args": {…}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum GitRequest {
    #[serde(rename = "git.bar")]
    Bar(StateArgs),
    #[serde(rename = "git.menu")]
    Menu(StateArgs),
    #[serde(rename = "git.remoteName")]
    RemoteName(UrlArgs),
    #[serde(rename = "git.commitView")]
    CommitView(EntriesArgs),
    #[serde(rename = "git.sshHost")]
    SshHost(UrlArgs),
    #[serde(rename = "git.isWebAddress")]
    IsWebAddress(UrlArgs),
    #[serde(rename = "git.nameFromUrl")]
    NameFromUrl(UrlArgs),
    #[serde(rename = "git.cloneName")]
    CloneName(CloneNameArgs),
    #[serde(rename = "git.shortVersion")]
    ShortVersion(VersionArgs),
    #[serde(rename = "git.sshPort")]
    SshPort(PortArgs),
    #[serde(rename = "git.offered")]
    Offered(OfferedArgs),
    #[serde(rename = "git.remaining")]
    Remaining(ProgressArgs),
    #[serde(rename = "git.canFinish")]
    CanFinish(ProgressArgs),
    #[serde(rename = "git.arrange")]
    Arrange(ArrangeArgs),
    #[serde(rename = "git.pushedAgo")]
    PushedAgo(AgoArgs),
    #[serde(rename = "git.roamOffered")]
    RoamOffered(RoamArgs),
    #[serde(rename = "git.saveWhen")]
    SaveWhen(WhenArgs),
}

pub fn dispatch(request: GitRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    match request {
        GitRequest::Bar(a) => write(out, &bar_view(a.state.as_ref())),
        GitRequest::Menu(a) => write(out, &menu_view(a.state.as_ref())),
        GitRequest::RemoteName(a) => write(out, &address::remote_name(&a.url)),
        GitRequest::CommitView(a) => write(out, &commit_view(&a.entries)),
        GitRequest::SshHost(a) => write(out, &address::ssh_host(&a.url)),
        GitRequest::IsWebAddress(a) => write(out, &address::is_web_address(&a.url)),
        GitRequest::NameFromUrl(a) => write(out, &address::name_from_url(&a.url)),
        GitRequest::CloneName(a) => write(out, &address::clone_name(&a.name, &a.taken)),
        GitRequest::ShortVersion(a) => write(out, &address::short_version(&a.version)),
        GitRequest::SshPort(a) => write(out, &address::ssh_port(&a.typed)),
        GitRequest::Offered(a) => write(out, &offered(&a.conflict)),
        GitRequest::Remaining(a) => {
            let chosen: HashSet<String> = a.chosen.into_iter().collect();
            write(out, &remaining(&a.conflicts, &chosen))
        }
        GitRequest::CanFinish(a) => {
            let chosen: HashSet<String> = a.chosen.into_iter().collect();
            write(out, &can_finish(&a.conflicts, &chosen))
        }
        GitRequest::Arrange(a) => write::<Arranged>(out, &git_rules::arrange(&a.repos, &a.filter)),
        GitRequest::PushedAgo(a) => write(out, &pushed_ago(a.iso.as_deref(), a.now)),
        GitRequest::RoamOffered(a) => write::<Vec<RoamEntry>>(out, &offered_roam(&a.state)),
        GitRequest::SaveWhen(a) => write(out, &save_when(a.epoch_ms, a.offset_min)),
    }
}
