//! callbacks.rs — the callback shapes the pieces hand each other: an ask's answer, an event's listener, a boxed future.
//! What a reader gets wrong: none of them crosses a thread boundary unsent; each is `Send` by its type.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde_json::Value;

/// Where an ask's answer goes once it is given.
pub type Reply = Box<dyn FnOnce(Result<Value, String>) + Send>;

/// One listener of a piece's events, such as a freezer's progress.
pub type Sink = Arc<dyn Fn(Value) + Send + Sync>;

/// A future boxed for a trait method.
pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;
