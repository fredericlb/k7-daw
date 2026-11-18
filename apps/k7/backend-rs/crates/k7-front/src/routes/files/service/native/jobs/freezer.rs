//! freezer.rs — the automatic freezer's timer over the link's served projects (sub-step 1k, B3; 2312).
//! What a reader gets wrong: dropping the task stops the timer only; a pass already running finishes, and `renderAll` works with no timer.

use std::sync::Arc;

use k7_file_service::jobs::FREEZE_EVERY;
use tokio::runtime::Handle;
use tokio::task::JoinHandle;

use super::Jobs;
use super::freeze_run::Freezing;
use crate::routes::files::service::native::docs::Docs;
use crate::routes::files::service::native::engine::EngineLink;

/// The running timer; dropping it stops the timer.
#[must_use = "dropping the task stops the timer"]
pub struct FreezerTask {
    timer: Option<JoinHandle<()>>,
}

impl FreezerTask {
    #[cfg(test)]
    pub(super) fn armed(&self) -> bool {
        self.timer.is_some()
    }
}

impl Drop for FreezerTask {
    fn drop(&mut self) {
        if let Some(t) = self.timer.take() {
            t.abort();
        }
    }
}

impl Jobs {
    /// Starts the freezer of the account's served projects; `docs` is the handle its compile reads the project through.
    /// `STUDIO_AUTO_FREEZE=off` arms no timer, as the TypeScript's, but a `renderAll` still has its executor.
    pub fn start_freezer(
        self: &Arc<Self>,
        link: EngineLink,
        docs: Docs,
        rt: &Handle,
    ) -> FreezerTask {
        let docs = docs.with_thawed(self.thawed.clone());
        let off = docs
            .env
            .get("STUDIO_AUTO_FREEZE")
            .is_some_and(|v| v == "off");
        let freezing = Freezing::new(self, link.clone(), docs, rt.clone());
        let freezing = self.freezing.get_or_init(|| freezing).clone();
        if off {
            return FreezerTask { timer: None };
        }
        let timer = rt.spawn(async move {
            let mut every = tokio::time::interval(FREEZE_EVERY);
            every.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                every.tick().await;
                for session in link.sessions() {
                    freezing.tick(&session);
                }
            }
        });
        FreezerTask { timer: Some(timer) }
    }
}
