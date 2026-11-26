//! The crate's tests in one binary: one module per part, one line each.

mod catalogue;
mod engine_abi;
mod engine_fake;
mod pairing_gate;
mod pairing_store;
mod stream;
mod wire_json;

/// A file of the repo, by its path from the root.
pub fn repo_file(path: &str) -> String {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../..");
    std::fs::read_to_string(format!("{root}/{path}")).unwrap_or_else(|e| panic!("{path}: {e}"))
}
