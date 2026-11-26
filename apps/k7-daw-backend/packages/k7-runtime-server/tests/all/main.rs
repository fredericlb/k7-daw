//! The crate's tests in one binary: one module per part, one line each.

mod catalogue;
mod doc_bounds;
mod engine_abi;
mod engine_fake;
mod harness;
mod pairing_gate;
mod pairing_store;
mod project_socket;
mod server_socket;
mod server_state;
mod server_timers;
mod server_units;
mod stream;
mod supersede;
mod wire_json;

/// A file of the repo, by its path from the root.
pub fn repo_file(path: &str) -> String {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../..");
    std::fs::read_to_string(format!("{root}/{path}")).unwrap_or_else(|e| panic!("{path}: {e}"))
}
