//! engine_abi.rs — `engine::ffi`'s tables held to k7-runtime's headers: every table's fields, by name and in
//! order, and the top table's size. A signature is not compared: CApi_test.cpp and the fake's tests call them.

use k7_runtime_server::engine::ffi::{K7CoreApi, K7EngineApi};
use k7_runtime_server::engine::plugins::K7PluginsApi;
use k7_runtime_server::engine::state::K7StateApi;

const CAPI: &str = "apps/k7-daw-engine/k7-runtime/engine/capi";

/// The field names of `struct <name> { … };` in a header: `(*name)(` for a function, `name;` for the rest.
fn header_fields(header: &str, name: &str) -> Vec<String> {
    let start = header
        .find(&format!("struct {name} {{"))
        .unwrap_or_else(|| panic!("no struct {name}"));
    let body = &header[start..start + header[start..].find("\n}").unwrap()];
    body.lines()
        .skip(1)
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("/*") && !l.starts_with('}'))
        .filter_map(|l| {
            if let Some(at) = l.find("(*") {
                return l[at + 2..].split(')').next().map(str::to_string);
            }
            l.trim_end_matches(';')
                .split_whitespace()
                .last()
                .map(|w| w.trim_start_matches('*').to_string())
        })
        .collect()
}

#[test]
fn the_top_table_is_k7_engine_h() {
    let header = crate::repo_file(&format!("{CAPI}/k7_engine.h"));
    assert_eq!(header_fields(&header, "K7EngineApi"), K7EngineApi::FIELDS);
    assert!(header.contains("#define K7_ENGINE_ABI 1"));
    let areas = K7EngineApi::FIELDS.len() - 2;
    assert_eq!(
        std::mem::size_of::<K7EngineApi>(),
        8 + areas * std::mem::size_of::<usize>()
    );
}

#[test]
fn the_core_table_is_k7_engine_core_h() {
    let header = crate::repo_file(&format!("{CAPI}/k7_engine_core.h"));
    assert_eq!(header_fields(&header, "K7CoreApi"), K7CoreApi::FIELDS);
    assert_eq!(
        std::mem::size_of::<K7CoreApi>(),
        K7CoreApi::FIELDS.len() * std::mem::size_of::<usize>()
    );
}

#[test]
fn the_project_and_transport_tables_are_their_headers() {
    use k7_runtime_server::engine::project::K7ProjectApi;
    use k7_runtime_server::engine::transport::K7TransportApi;
    let header = crate::repo_file(&format!("{CAPI}/k7_engine_project.h"));
    assert_eq!(header_fields(&header, "K7ProjectApi"), K7ProjectApi::FIELDS);
    let header = crate::repo_file(&format!("{CAPI}/k7_engine_transport.h"));
    assert_eq!(
        header_fields(&header, "K7TransportApi"),
        K7TransportApi::FIELDS
    );
}

#[test]
fn every_area_slot_is_declared_by_the_header() {
    let header = crate::repo_file(&format!("{CAPI}/k7_engine.h"));
    for field in &K7EngineApi::FIELDS[2..] {
        let ty: String = field
            .split('_')
            .map(|w| w[..1].to_uppercase() + &w[1..])
            .collect();
        assert!(
            header.contains(&format!("typedef struct K7{ty}Api K7{ty}Api;")),
            "{field}"
        );
    }
}

#[test]
fn the_state_table_is_k7_engine_state_h() {
    let header = crate::repo_file(&format!("{CAPI}/k7_engine_state.h"));
    assert_eq!(header_fields(&header, "K7StateApi"), K7StateApi::FIELDS);
    assert_eq!(
        std::mem::size_of::<K7StateApi>(),
        K7StateApi::FIELDS.len() * std::mem::size_of::<usize>()
    );
}

#[test]
fn the_plugins_table_is_k7_engine_plugins_h() {
    let header = crate::repo_file(&format!("{CAPI}/k7_engine_plugins.h"));
    assert_eq!(header_fields(&header, "K7PluginsApi"), K7PluginsApi::FIELDS);
    assert_eq!(
        std::mem::size_of::<K7PluginsApi>(),
        K7PluginsApi::FIELDS.len() * std::mem::size_of::<usize>()
    );
}
