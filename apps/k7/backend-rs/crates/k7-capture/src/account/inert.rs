//! inert.rs — the host of an account that only reads: it is told nothing and asks nothing (2171).
//! What a reader gets wrong: such an account is never attached, so nothing here is ever called but by a mistake.

use std::any::Any;

use super::{BoxFuture, Bundle};

pub struct Inert;

impl Bundle for Inert {
    fn feed(&self, _: &str, _: Vec<String>, _: u64) {}

    fn migrate_namespaces(&self) -> BoxFuture<Result<(), String>> {
        Box::pin(async { Ok(()) })
    }

    fn let_go_under(&self, _: &str) -> BoxFuture<Result<(), String>> {
        Box::pin(async { Ok(()) })
    }

    fn lock(&self) -> BoxFuture<Box<dyn Any + Send>> {
        Box::pin(async { Box::new(()) as Box<dyn Any + Send> })
    }

    fn folders_changed(&self) {}
}
