// pane_in_use.rs — which of THIS device's panes a remote shows, so this device cannot open it, as pure rules.
// A surface is a pane here by its declared id: timeline and turntables are panes, the picture is a window.
// A reader would get wrong that a mastering surface has no pane on this device, so nothing of it is ever blocked.

use serde::{Deserialize, Serialize};

/// A screen another device shows of this device's project, since the moment it began.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RemoteScreen {
    pub surface: String,
    pub device: String,
    pub since: f64,
}

/// The id of the surface each control of the main device opens on this device, none where the project declares none.
#[derive(Clone, Debug, Default, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MainSurfaceIds {
    pub timeline: Option<String>,
    pub turntables: Option<String>,
    pub viewport: Option<String>,
}

/// The device holding each control the remotes show; a control no remote shows is absent.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct InUse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeline: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turntables: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport: Option<String>,
}

/// The device holding each control the remotes show; where two show one surface, the one that came first.
pub fn in_use_by(screens: &[RemoteScreen], ids: &MainSurfaceIds) -> InUse {
    let mut out = InUse::default();
    let mut since: [Option<f64>; 3] = [None; 3];
    for s in screens {
        let controls: [(&Option<String>, &mut Option<String>); 3] = [
            (&ids.timeline, &mut out.timeline),
            (&ids.turntables, &mut out.turntables),
            (&ids.viewport, &mut out.viewport),
        ];
        for ((id, holder), at) in controls.into_iter().zip(since.iter_mut()) {
            if id.as_deref() != Some(s.surface.as_str()) {
                continue;
            }
            if at.is_none_or(|t| s.since < t) {
                *holder = Some(s.device.clone());
                *at = Some(s.since);
            }
        }
    }
    out
}

/// The next pane after `from` in `order`, wrapping, that is not blocked; none when every other one is.
pub fn next_free(order: &[String], from: &str, blocked: &[String]) -> Option<String> {
    let n = order.len();
    // A `from` outside the order starts one before the first, as `indexOf` answering -1 does.
    let at = order.iter().position(|p| p == from).map_or(-1, |i| i as isize);
    (1..=n as isize)
        .map(|i| &order[(at + i).rem_euclid(n as isize) as usize])
        .find(|id| id.as_str() != from && !blocked.contains(id))
        .cloned()
}

/// Whether two lists name the same screens, so a poll that changed nothing re-renders nothing.
pub fn same_screens(a: &[RemoteScreen], b: &[RemoteScreen]) -> bool {
    a == b
}

#[cfg(test)]
mod tests;
