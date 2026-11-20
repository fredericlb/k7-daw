// One test binary for the crate: every former tests/<name>.rs is the module <name>, and the support folders are declared once here.
// Kept apart: files_health.rs, files_hub.rs and files_hub_refusal.rs, since a hub's teardown SIGTERMs every service its process started.
// http_support/ stays beside them in tests/, which they declare too; files_hub_support/ is theirs alone.

#[path = "../http_support/mod.rs"]
mod http_support;
mod support;

mod ai_turn;
mod auth;
mod cameras;
mod engine;
mod export;
mod files_ready;
mod last_routes;
mod leaves;
mod routes;
mod trust;
