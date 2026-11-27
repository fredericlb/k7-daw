// forge.rs — what the forge routes answer: an account and its status, a Forgejo server's probe, a sign-in's start and its polls.
// A reader would get wrong that nothing here holds a token or a secret: the client id is public, the grant stays in the front.

use serde::{Deserialize, Serialize};

/// The forges k7 knows how to draw; a new one is a new member here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum ForgeKind {
    Github,
    Forgejo,
}

/// What `GET /git/forges/<id>` says: the sign-in exists in this build, and who is signed in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ForgeStatus {
    pub available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub login: Option<String>,
    pub scopes: Vec<String>,
    /// Whether signing out (and, for a Forgejo server, removing it) reaches the person's other Apple devices too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub synced: Option<bool>,
}

/// One entry of `GET /git/forges`: the account's id (its forge's host), its kind and its name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ForgeAccount {
    #[serde(flatten)]
    pub status: ForgeStatus,
    pub id: String,
    pub kind: ForgeKind,
    pub name: String,
    /// A Forgejo server's address, as the person typed it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub url: Option<String>,
}

/// What `POST /git/forges/probe` read from a server without a token.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ForgeProbe {
    pub id: String,
    pub url: String,
    pub version: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub page_size: u64,
    pub lfs: bool,
    pub root_url: String,
    pub root_answers: bool,
    pub redirects: Vec<String>,
    pub scopes: Vec<String>,
}

/// What `POST /git/forges/<id>/start` answers for a Forgejo server: the page to open in a browser.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ForgeStart {
    pub authorize_url: String,
    pub redirect: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub expires_in: u64,
}

/// What `POST /git/forges/github.com/start` answers: the code the person types at GitHub.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DeviceCode {
    pub user_code: String,
    pub verification_uri: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub interval: u64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub expires_in: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PollState {
    Pending,
    SlowDown,
    Ok,
    Denied,
    Error,
    Expired,
}

/// How a sign-in stands, said by `POST …/poll`; a Forgejo sign-in's end is said once.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PollAnswer {
    pub state: PollState,
    /// GitHub's polling interval, in seconds, which `slow_down` lengthens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "number"))]
    pub interval: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub login: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub scopes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub key_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub error: Option<String>,
    /// The front's code for a failure, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub code: Option<String>,
}

impl PollAnswer {
    pub fn state(state: PollState) -> PollAnswer {
        PollAnswer {
            state,
            interval: None,
            login: None,
            scopes: None,
            key_note: None,
            error: None,
            code: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_account_reads_as_its_status_and_its_names() {
        let a = ForgeAccount {
            status: ForgeStatus {
                available: true,
                login: Some("o".into()),
                scopes: vec!["repo".into()],
                synced: Some(false),
            },
            id: "github.com".into(),
            kind: ForgeKind::Github,
            name: "GitHub".into(),
            url: None,
        };
        let v = serde_json::to_value(&a).unwrap();
        assert_eq!(
            v,
            serde_json::json!({"available": true, "login": "o", "scopes": ["repo"], "synced": false,
                "id": "github.com", "kind": "github", "name": "GitHub"})
        );
        assert_eq!(serde_json::from_value::<ForgeAccount>(v).unwrap(), a);
    }

    #[test]
    fn a_poll_leaves_out_what_it_does_not_say() {
        let v = serde_json::to_value(PollAnswer {
            interval: Some(10),
            ..PollAnswer::state(PollState::SlowDown)
        })
        .unwrap();
        assert_eq!(v, serde_json::json!({"state": "slow_down", "interval": 10}));
        let done = PollAnswer {
            login: Some("octo".into()),
            key_note: Some("n".into()),
            ..PollAnswer::state(PollState::Ok)
        };
        assert_eq!(
            serde_json::to_value(done).unwrap(),
            serde_json::json!({"state": "ok", "login": "octo", "keyNote": "n"})
        );
    }
}
