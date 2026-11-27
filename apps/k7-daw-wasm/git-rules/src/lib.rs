// lib.rs — git's wire shapes and the page's rules over them, with no git2 in it so a browser's module can link it.
// A reader would get wrong that the routes live elsewhere: k7-git and k7-front build these values, WebAssembly applies the rules.

pub mod address;
pub mod files;
pub mod forge;
pub mod github;
pub mod merge;
pub mod roam;
pub mod save;
pub mod state;

pub use address::{
    PortAnswer, SshHost, clone_name, is_web_address, name_from_url, remote_name, short_version, ssh_host, ssh_port,
};
pub use files::{Change, CommitView, Entry, LogEntry, commit_view};
pub use forge::{DeviceCode, ForgeAccount, ForgeKind, ForgeProbe, ForgeStart, ForgeStatus, PollAnswer, PollState};
pub use github::{Ago, AgoUnit, Arranged, GithubRepo, arrange, pushed_ago};
pub use merge::{Choice, Conflict, Kind, PullState, can_finish, offered, remaining};
pub use roam::{RoamAnswer, RoamEntry, RoamState, offered_roam};
pub use save::save_when;
pub use state::{
    Autosave, BarAutosave, BarView, Blocked, FetchError, GitState, LastFetch, MenuView, Mode, bar_view, locked_by,
    menu_view,
};
