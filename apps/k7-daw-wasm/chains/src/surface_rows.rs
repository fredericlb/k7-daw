// surface_rows.rs — the Surfaces tab's rows, as data: one group per device, a row per surface, each an address or the one reason it is not.
// The filters run in a fixed order and a failed one shows the row disabled, never hidden. See ../../docs/external.md.
// A reader would get wrong that nothing here sorts or spells an address: the page orders devices by locale and builds each `target`'s URL.

use serde::{Deserialize, Serialize};

/// What the page speaks of the door and the engine protocol: the version a peer is held against.
#[derive(Clone, Copy, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct RowsVersions {
    pub api: f64,
    pub min_proto: f64,
    pub proto: f64,
}

/// A device found on the network, as far as the rows read it.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct RowPeer {
    pub engine_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub host: String,
    pub reached: bool,
    pub trusted: bool,
}

/// What a device's door says of itself, as far as the rows read it.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RowHello {
    pub device: RowDevice,
    pub project: Option<RowProject>,
    #[serde(default)]
    pub surfaces: Vec<RowSurface>,
    #[serde(default)]
    pub screens: Vec<RowScreen>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RowDevice {
    pub name: String,
    pub api: f64,
    pub proto: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RowProject {
    pub workspace: String,
    pub path: String,
    pub label: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RowSurface {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RowScreen {
    pub surface: String,
    pub device: String,
}

/// A peer and what its hello came to: `asked` is false while the first is on its way (not drawn yet).
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RowPeerAnswer {
    pub peer: RowPeer,
    pub asked: bool,
    pub hello: Option<RowHello>,
}

/// What the rows are made from: the peers, this device, and the surface types a drawer exists for.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct SurfaceRowsArgs {
    pub peers: Vec<RowPeerAnswer>,
    pub self_id: Option<String>,
    pub self_name: String,
    pub drawable: Vec<String>,
    pub versions: RowsVersions,
}

/// Why a row cannot be opened; the page turns each into a sentence.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "why", rename_all = "camelCase")]
pub enum RowBlocked {
    NotAnswering,
    NotTrusted,
    Update { device: String },
    WindowOnly,
}

/// Where a row opens: the pieces of its address, which the page spells.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct RowAddress {
    pub engine_id: String,
    pub workspace: String,
    pub project: String,
    pub surface: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct SurfaceRow {
    pub key: String,
    pub id: String,
    #[serde(rename = "type")]
    #[cfg_attr(feature = "ts", ts(rename = "type"))]
    pub kind: String,
    /// The address's pieces, when the row opens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<RowAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked: Option<RowBlocked>,
    /// The other devices already showing it.
    pub also_on: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct RowGroup {
    pub engine_id: String,
    pub device: String,
    /// The project's name, none when the device has none open.
    pub project: Option<String>,
    /// A device-level reason, when no row can be listed at all.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked: Option<RowBlocked>,
    pub rows: Vec<SurfaceRow>,
}

/// The device that must update, or none when the two speak the same: the door's version, then the engine protocol's range.
pub fn older_device(hello: &RowDevice, self_name: &str, v: &RowsVersions) -> Option<String> {
    if hello.api != v.api {
        return Some(if hello.api < v.api {
            hello.name.clone()
        } else {
            self_name.to_owned()
        });
    }
    if hello.proto < v.min_proto {
        return Some(hello.name.clone());
    }
    (hello.proto > v.proto).then(|| self_name.to_owned())
}

/// The first of the three that is set and not empty, as `a || b || c` reads names.
fn first_name(names: [&str; 3]) -> &str {
    names.into_iter().find(|n| !n.is_empty()).unwrap_or("")
}

fn row_of(
    peer: &RowPeer,
    project: &RowProject,
    hello: &RowHello,
    s: &RowSurface,
    input: &SurfaceRowsArgs,
) -> SurfaceRow {
    let updates = older_device(&hello.device, &input.self_name, &input.versions);
    let blocked = match updates {
        Some(device) => Some(RowBlocked::Update { device }),
        None if input.drawable.contains(&s.kind) => None,
        None => Some(RowBlocked::WindowOnly),
    };
    SurfaceRow {
        key: format!("{}/{}", peer.engine_id, s.id),
        id: s.id.clone(),
        kind: s.kind.clone(),
        target: blocked.is_none().then(|| RowAddress {
            engine_id: peer.engine_id.clone(),
            workspace: project.workspace.clone(),
            project: project.path.clone(),
            surface: s.id.clone(),
        }),
        blocked,
        also_on: hello
            .screens
            .iter()
            .filter(|c| c.surface == s.id)
            .map(|c| c.device.clone())
            .collect(),
    }
}

/// The devices' groups in the order the peers came; a device not yet answered is left out, so the list does not grow line by line.
pub fn surface_groups(input: &SurfaceRowsArgs) -> Vec<RowGroup> {
    let mut groups = Vec::new();
    for RowPeerAnswer { peer, asked, hello } in &input.peers {
        if input.self_id.as_deref() == Some(peer.engine_id.as_str()) {
            continue;
        }
        let hello_name = hello.as_ref().map_or("", |h| h.device.name.as_str());
        let name = first_name([hello_name, &peer.name, &peer.host]).to_owned();
        let empty = |blocked| RowGroup {
            engine_id: peer.engine_id.clone(),
            device: name.clone(),
            project: None,
            blocked: Some(blocked),
            rows: Vec::new(),
        };
        if !peer.reached {
            groups.push(empty(RowBlocked::NotAnswering));
            continue;
        }
        if !peer.trusted {
            groups.push(empty(RowBlocked::NotTrusted));
            continue;
        }
        // Asked and nothing answered: the row says so; not asked yet: nothing is drawn for it.
        let Some(hello) = hello else {
            if *asked {
                groups.push(empty(RowBlocked::NotAnswering));
            }
            continue;
        };
        let rows = match &hello.project {
            Some(p) => hello
                .surfaces
                .iter()
                .map(|s| row_of(peer, p, hello, s, input))
                .collect(),
            None => Vec::new(),
        };
        groups.push(RowGroup {
            engine_id: peer.engine_id.clone(),
            device: name,
            project: hello.project.as_ref().map(|p| p.label.clone()),
            blocked: None,
            rows,
        });
    }
    groups
}

#[cfg(test)]
mod tests;
