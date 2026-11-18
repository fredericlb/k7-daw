//! tests_device.rs — a device's account for the history tests: its capture bound to a temp home, and the request builders.
//! What a reader gets wrong: the account is bound but never attached, so each test attaches the workspaces it uses.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::Request;
use k7_capture::account::Account;
use k7_file_service::libraries::Home;
use serde_json::Value;

use super::tests_capture::host;

pub(super) struct Dev {
    pub(super) acct: Arc<Account>,
    pub(super) home: Home,
    _dir: std::path::PathBuf,
}

pub(super) async fn device(name: &str) -> Dev {
    let dir = crate::routes::files::service::native::tests::tmp_root()
        .join(format!("k7-front-dev-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = crate::routes::files::service::native::tests::slashed(&dir.join("home"));
    std::fs::create_dir_all(format!("{path}/workspace")).unwrap();
    let acct = Arc::new(Account::new(Arc::new(|_| {})));
    acct.bind(host(&path));
    let mut env = std::collections::BTreeMap::new();
    env.insert("STUDIO_HOME".to_string(), path);
    Dev {
        acct,
        home: Home::of(&env, false),
        _dir: dir,
    }
}

pub(super) fn req(method: &str, uri: &str, body: Option<Value>) -> Request {
    let mut b = Request::builder().method(method).uri(uri);
    if body.is_some() {
        b = b.header("content-type", "application/json");
    }
    b.body(Body::from(body.map(|v| v.to_string()).unwrap_or_default()))
        .unwrap()
}

pub(super) fn parse(s: &str) -> Value {
    serde_json::from_str(s).unwrap_or(Value::Null)
}
