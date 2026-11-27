// cameras.rs — which lens on this machine a layer's label names: the rule a live lane follows before it opens a camera.
// A reader would get wrong that a missing lens is a failure: it falls back to the default and says why, so a project that travels draws.

use serde::{Deserialize, Serialize};

/// The little of a `MediaDeviceInfo` the match needs.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "CameraDevice"))]
pub struct Device {
    pub device_id: String,
    pub kind: String,
    pub label: String,
}

/// What matching a label answered, and WHY: a fallback that says nothing reads as a broken app.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "CameraMatch"))]
pub struct Match {
    /// The device to open, or none to let the browser pick, which is also what "" asks for.
    pub device_id: Option<String>,
    /// Absent when the label was found, or when none was asked for.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub problem: Option<String>,
}

/// Which camera a layer's label names, among the devices this machine has.
///
/// Exact first, then case-insensitively, then a containment either way: a browser dresses a label up
/// (`"Logitech BRIO (046d:085e)"`), and a person writing one down does not copy the ids. Nothing is REFUSED.
pub fn match_camera(label: &str, devices: &[Device]) -> Match {
    let cams: Vec<&Device> = devices.iter().filter(|d| d.kind == "videoinput").collect();
    // "" is not "no camera": it is "whichever this machine calls default", the ordinary state.
    if label.is_empty() {
        return Match {
            device_id: None,
            problem: None,
        };
    }
    if cams.is_empty() {
        return Match {
            device_id: None,
            problem: Some("this machine reports no camera at all".to_owned()),
        };
    }
    let want = label.trim().to_lowercase();
    let hit = cams
        .iter()
        .find(|d| d.label == label)
        .or_else(|| cams.iter().find(|d| d.label.trim().to_lowercase() == want))
        .or_else(|| cams.iter().find(|d| d.label.to_lowercase().contains(&want)))
        .or_else(|| {
            cams.iter()
                .find(|d| want.contains(&d.label.trim().to_lowercase()) && !d.label.is_empty())
        });
    if let Some(d) = hit {
        return Match {
            device_id: Some(d.device_id.clone()),
            problem: None,
        };
    }
    // The names it DOES have: without them the sentence cannot be acted on.
    let has: Vec<&str> = cams
        .iter()
        .map(|d| {
            if d.label.is_empty() {
                "an unnamed camera"
            } else {
                d.label.as_str()
            }
        })
        .collect();
    Match {
        device_id: None,
        problem: Some(format!(
            "no camera called \"{label}\" on this machine — using the default one (it has: {})",
            has.join(", ")
        )),
    }
}
