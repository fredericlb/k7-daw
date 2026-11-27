// ops/address.rs — the address.* operations: a project's address read and written, and the names it reserves.
// A reader would get wrong that no origin is here: the page joins its own to a path this family answers.

use crate::error::WasmError;
use crate::ops::write;
use serde::{Deserialize, Serialize};
use song_schema::address as a;

/// An address and the mount it is read under.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AddressAt {
    pub pathname: String,
    pub base: String,
}

/// A project: the repository's short name and the folder inside it.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AddressProject {
    pub repo: String,
    pub project: String,
}

/// A project URL already built, and the surface hung off it.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", ts(rename_all = "camelCase"))]
pub struct AddressExternal {
    pub project_url: String,
    pub surface: String,
}

/// A surface of a project on another device, by its engine id.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", ts(rename_all = "camelCase"))]
pub struct AddressLan {
    pub engine_id: String,
    pub workspace: String,
    pub project: String,
    pub surface: String,
}

/// A workspace and the project inside it.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AddressLocal {
    pub workspace: String,
    pub project: String,
}

/// One name: a folder, a project half or a claim.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AddressName {
    pub name: String,
}

/// The chain a bench address seeds its rig with, when one is given.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AddressChain {
    pub chain: Option<String>,
}

/// What a bench address names: a rig, bare (`chain` null) or seeded with a library chain.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BenchRef {
    pub chain: Option<String>,
}

/// The address family's requests: `{"op": "address.parse_launch", "args": {…}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum AddressRequest {
    #[serde(rename = "address.parse_launch")]
    ParseLaunch(AddressAt),
    #[serde(rename = "address.basename")]
    Basename(AddressAt),
    #[serde(rename = "address.parse_external")]
    ParseExternal(AddressAt),
    #[serde(rename = "address.parse_lan_surface")]
    ParseLanSurface(AddressAt),
    #[serde(rename = "address.is_home_page")]
    IsHomePage(AddressAt),
    #[serde(rename = "address.launch_path")]
    LaunchPath(AddressProject),
    #[serde(rename = "address.external_of")]
    ExternalOf(AddressExternal),
    #[serde(rename = "address.lan_surface_path")]
    LanSurfacePath(AddressLan),
    #[serde(rename = "address.local_project_path")]
    LocalProjectPath(AddressLocal),
    #[serde(rename = "address.local_target")]
    LocalTarget(AddressName),
    #[serde(rename = "address.bench_chain")]
    BenchChain(AddressName),
    #[serde(rename = "address.bench_project")]
    BenchProject(AddressChain),
    #[serde(rename = "address.claim_key")]
    ClaimKey(AddressProject),
    #[serde(rename = "address.claim_is_bench")]
    ClaimIsBench(AddressName),
    #[serde(rename = "address.is_project")]
    IsProject(AddressName),
    #[serde(rename = "address.project_label")]
    ProjectLabel(AddressName),
    #[serde(rename = "address.is_droppable")]
    IsDroppable(AddressName),
}

pub fn dispatch(request: AddressRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    use AddressRequest as R;
    match request {
        R::ParseLaunch(r) => write(out, &a::parse_launch(&r.pathname, &r.base)),
        R::Basename(r) => write(out, &a::project_basename(&r.pathname, &r.base)),
        R::ParseExternal(r) => write(out, &a::parse_external(&r.pathname, &r.base)),
        R::ParseLanSurface(r) => write(out, &a::parse_lan_surface(&r.pathname, &r.base)),
        R::IsHomePage(r) => write(out, &a::is_home_page(&r.pathname, &r.base)),
        R::LaunchPath(r) => write(out, &a::launch_path(&r.repo, &r.project)),
        R::ExternalOf(r) => write(out, &a::external_of(&r.project_url, &r.surface)),
        R::LanSurfacePath(r) => write(
            out,
            &a::lan_surface_path(&r.engine_id, &r.workspace, &r.project, &r.surface),
        ),
        R::LocalProjectPath(r) => write(out, &a::local_project_path(&r.workspace, &r.project)),
        R::LocalTarget(r) => write(out, &a::local_target(&r.name)),
        R::BenchChain(r) => write(out, &a::bench_chain_of(&r.name).map(|chain| BenchRef { chain })),
        R::BenchProject(r) => write(out, &a::bench_project(r.chain.as_deref())),
        R::ClaimKey(r) => write(out, &a::claim_key(&r.repo, &r.project)),
        R::ClaimIsBench(r) => write(out, &a::claim_is_bench(&r.name)),
        R::IsProject(r) => write(out, &a::is_project(&r.name)),
        R::ProjectLabel(r) => write(out, &a::project_label(&r.name)),
        R::IsDroppable(r) => write(out, &a::is_droppable_project(&r.name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::Session;

    #[test]
    fn the_family_answers_through_the_dispatch() {
        let mut out = Vec::new();
        let input = br#"{"op":"address.parse_launch","args":{"pathname":"/@local/W/a.k7/tl","base":"/"}}"#;
        assert_eq!(Session::new().handle(input, &mut out), crate::ops::OK);
        assert_eq!(String::from_utf8_lossy(&out), r#"{"repo":"@local","project":"W/a.k7"}"#);
        let input = br#"{"op":"address.bench_chain","args":{"name":"live.k7"}}"#;
        Session::new().handle(input, &mut out);
        assert_eq!(String::from_utf8_lossy(&out), r#"{"chain":null}"#);
    }
}
