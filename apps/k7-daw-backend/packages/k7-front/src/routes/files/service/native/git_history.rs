//! git_history.rs — what git leaves in a workspace's local history: a commit names a checkpoint with its subject, and
//! a pull's landing is one entry `pull <short id>`. What a reader gets wrong: it is best effort, a workspace this account
//! does not capture has no history and a failure here never fails the git operation that came first.

use k7_capture::account::Ask;
use serde_json::json;

use super::git::Ctx;

/// A checkpoint at the room's head named by the commit's subject (the history keeps names to 120 characters).
pub async fn committed(ctx: &Ctx, root: &str, message: &str, by: &str) {
    let Some(account) = &ctx.account else { return };
    let subject: String = message
        .lines()
        .next()
        .unwrap_or("")
        .chars()
        .take(100)
        .collect();
    if subject.trim().is_empty() {
        return;
    }
    let ask = Ask::Checkpoint {
        body: json!({ "name": subject.trim() }),
        by: by.to_string(),
    };
    if let Err(e) = account.history_ask(root, ask).await {
        eprintln!(
            "k7-front: the commit's checkpoint was not made: {}",
            e.message
        );
    }
}

/// One history entry for the files a pull or a merge landed, named by the commit that was pulled.
pub async fn landed(ctx: &Ctx, root: &str, tip: &str) {
    if let Some(account) = &ctx.account {
        let short = &tip[..tip.len().min(7)];
        account.record_landing(root, &format!("pull {short}")).await;
    }
}
