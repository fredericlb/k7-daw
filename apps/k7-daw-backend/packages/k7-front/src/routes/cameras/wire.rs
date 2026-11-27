// cameras/wire.rs — what the camera hub and its list say to the page: the account's cameras, a ticket, and the socket's frames.
// A reader would get wrong that the hub does not build these: it passes a peer's SDP and candidates through, adding `from`, `to` and `problem`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One camera of the account, as `GET /api/cameras` lists it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct RemoteCamera {
    pub name: String,
    /// Epoch ms it came on.
    pub since: f64,
}

/// `RTCIceServer`, declared structurally so the backend needs no DOM types.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(rename = "RTCIceServerLike"))]
pub struct IceServer {
    #[cfg_attr(feature = "ts", ts(type = "string | string[]"))]
    pub urls: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub credential: Option<String>,
}

/// The ticket route's answer: the socket's key, and the ICE servers this server was told to hand out.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CameraTicket {
    pub ticket: String,
    pub ice_servers: Vec<IceServer>,
}

/// A session description a peer sends.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SessionDescription {
    #[serde(rename = "type")]
    #[cfg_attr(feature = "ts", ts(rename = "type"))]
    pub type_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub sdp: Option<String>,
}

/// An ICE candidate a peer sends.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct IceCandidate {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub candidate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub sdp_mid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional = nullable))]
    pub sdp_m_line_index: Option<u32>,
}

/// One frame of the signalling socket.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum CameraFrame {
    /// The hub to a camera, once: the name it is listed under, made unique within the account.
    Hello { name: String },
    /// A viewer asking for a camera by label; the hub adds `from` on the way to the camera.
    Want {
        label: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        from: Option<String>,
    },
    Offer {
        sdp: SessionDescription,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        to: Option<String>,
        /// Why the camera a viewer gets is not the one it named: set by the hub, read by the viewer.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional = nullable))]
        problem: Option<String>,
    },
    Answer {
        sdp: SessionDescription,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        from: Option<String>,
    },
    Ice {
        candidate: IceCandidate,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        to: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        from: Option<String>,
    },
    Problem {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        to: Option<String>,
    },
    /// A viewer left: the camera drops its peer connection.
    Gone { from: String },
    /// How many viewers a camera is serving, sent to it whenever that changes.
    Viewers { count: u32 },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_hubs_own_frames_read_back() {
        let frames = [
            json!({ "type": "hello", "name": "cam" }),
            json!({ "type": "want", "label": "remote:cam", "from": "v1" }),
            json!({ "type": "gone", "from": "v1" }),
            json!({ "type": "problem", "text": "no camera" }),
            json!({ "type": "viewers", "count": 2 }),
        ];
        for f in frames {
            let read: CameraFrame = serde_json::from_value(f.clone()).unwrap();
            assert_eq!(serde_json::to_value(&read).unwrap(), f);
        }
    }

    #[test]
    fn a_listing_and_a_ticket_read_back() {
        let list: Vec<RemoteCamera> = serde_json::from_value(json!([{ "name": "a", "since": 1700000000000.0 }])).unwrap();
        assert_eq!(list[0].name, "a");
        let ticket: CameraTicket =
            serde_json::from_value(json!({ "ticket": "t", "iceServers": [{ "urls": "stun:x" }] })).unwrap();
        assert_eq!(ticket.ice_servers.len(), 1);
    }
}
