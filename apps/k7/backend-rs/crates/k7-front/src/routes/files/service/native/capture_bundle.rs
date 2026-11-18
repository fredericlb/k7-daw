//! capture_bundle.rs — the `Bundle` the account's native captures call back through: the front's own memos, watches, engine and lock.
//! What a reader gets wrong: nothing here asks a runtime; a legacy namespace is never migrated (5-4), the attach just goes on.

use std::any::Any;
use std::sync::Arc;

use k7_capture::account::{BoxFuture, Bundle};
use k7_file_service::modwatch::Watch;

use super::engine::routes::Engine;
use super::{locks, workspaces_memo as memo};

/// What the captures of one account tell and ask the front.
pub struct FrontBundle {
    studio: String,
    account: String,
    /// The module watch that hears the capture's feed, where `runtime` answers.
    watch: Option<Arc<Watch>>,
    engine: Option<Arc<Engine>>,
}

impl FrontBundle {
    pub fn new(
        studio: &str,
        account: &str,
        watch: Option<Arc<Watch>>,
        engine: Option<Arc<Engine>>,
    ) -> FrontBundle {
        FrontBundle {
            studio: studio.to_string(),
            account: account.to_string(),
            watch,
            engine,
        }
    }
}

impl Bundle for FrontBundle {
    fn feed(&self, root: &str, paths: Vec<String>, _generation: u64) {
        if let Some(w) = &self.watch {
            w.feed(root, &paths);
        }
    }

    fn migrate_namespaces(&self) -> BoxFuture<Result<(), String>> {
        Box::pin(async { Ok(()) })
    }

    fn let_go_under(&self, root: &str) -> BoxFuture<Result<(), String>> {
        let (watch, engine, root) = (self.watch.clone(), self.engine.clone(), root.to_string());
        Box::pin(async move {
            if let Some(w) = watch {
                let under = root.clone();
                let _ = tokio::task::spawn_blocking(move || w.release(Some(&under))).await;
            }
            if let Some(e) = engine {
                e.link.unload_under(&root).await;
            }
            Ok(())
        })
    }

    fn lock(&self) -> BoxFuture<Box<dyn Any + Send>> {
        let studio = self.studio.clone();
        Box::pin(async move { Box::new(locks::write(&studio).await) as Box<dyn Any + Send> })
    }

    fn folders_changed(&self) {
        memo::dropped(&self.account);
    }
}
