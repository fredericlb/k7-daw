//! owner.rs — `requireRootOwner`: the first account to reach a home owns it.
//! What a reader gets wrong: the claim is a create-new file, so two racing first requests agree on one owner.

use std::path::Path;

use serde_json::{Value, json};

/// `requireRootOwner`: the first account to reach a home is recorded beside it, and no other ever passes.
pub fn claim_root(home: &Path, email: &str) -> bool {
    let at = home.join("owner.json");
    let read = |p: &Path| -> Option<String> {
        let v: Value = serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()?;
        v.get("owner")
            .and_then(Value::as_str)
            .filter(|o| !o.is_empty())
            .map(str::to_string)
    };
    if let Some(owner) = read(&at) {
        return owner == email;
    }
    use std::io::Write;
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&at)
    {
        Ok(mut f) => {
            let text = format!(
                "{}\n",
                serde_json::to_string_pretty(&json!({ "owner": email })).unwrap_or_default()
            );
            f.write_all(text.as_bytes()).is_ok()
        }
        Err(_) => read(&at).as_deref() == Some(email),
    }
}
