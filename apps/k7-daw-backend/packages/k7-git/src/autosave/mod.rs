//! autosave — the person's technical branch (`k7/autosave/<who>`): snapshots of the work tree taken while `HEAD`
//! stays on the main branch, shared by every device of that person, and reset onto the main branch by a Save.
//! What a reader gets wrong: the work tree is always judged against `HEAD` (main), never against this branch; a
//! snapshot's `K7-Base:` trailer names the main commit it sits on (`docs/git.md`, "Auto-save").

mod integrate;
mod push;
mod snapshot;

use git2::Oid;

use crate::Repo;

pub use integrate::{FetchObjects, Picked};
pub use push::Sent;

/// The prefix every technical branch carries.
pub const PREFIX: &str = "k7/autosave/";

/// Who a technical branch belongs to: a forge account's login, or the commit email on any other remote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner<'a> {
    Forge(&'a str),
    Email(&'a str),
}

/// `text` lowercased, `@` spelled `-at-`, anything but `[a-z0-9._-]` a dash, runs of dashes and dots folded.
pub fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.trim().to_lowercase().replace('@', "-at-").chars() {
        let c = if c.is_ascii_alphanumeric() || c == '.' || c == '_' {
            c
        } else {
            '-'
        };
        let last = out.chars().last();
        if (c == '-' || c == '.') && (last == Some('-') || last == Some('.')) {
            continue;
        }
        out.push(c);
    }
    let out = out.trim_matches(|c| c == '-' || c == '.').to_string();
    let out = out
        .strip_suffix(".lock")
        .map_or(out.clone(), str::to_string);
    if out.is_empty() { "user".into() } else { out }
}

/// The branch an owner's devices share.
pub fn branch_name(owner: Owner<'_>) -> String {
    match owner {
        Owner::Forge(login) => format!("{PREFIX}{}", slug(login)),
        Owner::Email(email) => format!("{PREFIX}{}", slug(email)),
    }
}

/// The trailer naming the main commit a snapshot sits on; `none` before the first commit.
const BASE: &str = "K7-Base:";

pub(crate) fn base_trailer(head: Option<Oid>) -> String {
    format!("{BASE} {}", head.map_or("none".into(), |h| h.to_string()))
}

impl Repo {
    pub(crate) fn tip_of(&self, refname: &str) -> Option<Oid> {
        self.git.refname_to_id(refname).ok()
    }

    /// The technical branch here, and as the last fetch saw it on the remote.
    pub(crate) fn autosave_tips(&self, branch: &str) -> (Option<Oid>, Option<Oid>) {
        (
            self.tip_of(&format!("refs/heads/{branch}")),
            self.tip_of(&format!("refs/remotes/origin/{branch}")),
        )
    }

    pub(crate) fn head_id(&self) -> Option<Oid> {
        self.git.head().ok().and_then(|h| h.target())
    }

    /// The main commit `id` sits on: its trailer, or the commit itself when it is a main commit (a reset tip).
    pub(crate) fn base_of(&self, id: Oid) -> Option<Oid> {
        let c = self.git.find_commit(id).ok()?;
        let msg = c.message().unwrap_or("");
        match msg
            .lines()
            .find_map(|l| l.strip_prefix(BASE))
            .map(str::trim)
        {
            Some("none") => None,
            Some(h) => Oid::from_str(h).ok(),
            None => Some(id),
        }
    }

    /// Whether `a` is `b` or has it in its history.
    pub(crate) fn has_in_history(&self, a: Oid, b: Oid) -> bool {
        a == b || self.git.graph_descendant_of(a, b).unwrap_or(false)
    }

    /// When the technical branch here was last written, in seconds, and whether the remote lacks it.
    pub fn autosave_info(&self, branch: &str) -> Option<(i64, bool)> {
        let (local, remote) = self.autosave_tips(branch);
        let local = local?;
        let at = self.git.find_commit(local).ok()?.time().seconds();
        Some((at, remote != Some(local)))
    }

    fn lease_file(&self) -> std::path::PathBuf {
        self.gitdir.join("k7-autosave-lease")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_login_or_an_email_becomes_one_safe_branch() {
        assert_eq!(branch_name(Owner::Forge("Ada")), "k7/autosave/ada");
        assert_eq!(
            branch_name(Owner::Email("Ada.L+k7@Example.com")),
            "k7/autosave/ada.l-k7-at-example.com"
        );
        assert_eq!(slug("..a..b//c.lock"), "a.b-c");
        assert_eq!(slug("  @ "), "at");
        assert_eq!(slug("é"), "user");
        for s in ["a b", "x..y", "-z-", "q.lock", "w@{x}"] {
            let b = branch_name(Owner::Email(s));
            assert!(
                git2::Reference::is_valid_name(&format!("refs/heads/{b}")),
                "{b}"
            );
        }
    }
}
