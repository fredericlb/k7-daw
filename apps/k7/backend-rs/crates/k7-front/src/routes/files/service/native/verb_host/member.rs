//! member.rs — the host routes under `/verb-host`, one row each: its path, what it takes and what it answers with.
//! What a reader gets wrong: the body limit is the row's, applied before the body is read, never after it is parsed.

use hyper::Method;

/// One host route (3f); `Job` is the poll every long member shares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Member {
    Job,
    Render,
    Peak,
    Separate,
    PlaylistStems,
    TakeSave,
    Probe,
    Prepare,
    Picture,
    Extract,
    Transcribe,
    Plugins,
    PluginsRescan,
    Devices,
    DevicesStereo,
    Onnx,
    Engine,
}

/// A body is 64 KiB, but a doc or a scene.
const SMALL: usize = 64 << 10;
const LARGE: usize = 8 << 20;

pub const ALL: [Member; 17] = [
    Member::Job,
    Member::Render,
    Member::Peak,
    Member::Separate,
    Member::PlaylistStems,
    Member::TakeSave,
    Member::Probe,
    Member::Prepare,
    Member::Picture,
    Member::Extract,
    Member::Transcribe,
    Member::Plugins,
    Member::PluginsRescan,
    Member::Devices,
    Member::DevicesStereo,
    Member::Onnx,
    Member::Engine,
];

impl Member {
    /// Whether the member files a result inside the workspace its header names: a reader's workspace refuses it.
    pub fn writes(self) -> bool {
        matches!(
            self,
            Member::PlaylistStems
                | Member::TakeSave
                | Member::Picture
                | Member::Extract
                | Member::Render
                | Member::Prepare
        )
    }

    /// The member a request names; `None` for a path or a method the table lacks.
    pub fn of(method: &Method, path: &str) -> Option<Member> {
        ALL.into_iter()
            .find(|m| m.path() == path && *method == m.method())
    }

    pub fn method(self) -> Method {
        match self {
            Member::Job | Member::Plugins | Member::Devices => Method::GET,
            _ => Method::POST,
        }
    }

    pub fn path(self) -> &'static str {
        match self {
            Member::Job => "/verb-host/job",
            Member::Render => "/verb-host/render",
            Member::Peak => "/verb-host/peak",
            Member::Separate => "/verb-host/separate",
            Member::PlaylistStems => "/verb-host/playlist-stems",
            Member::TakeSave => "/verb-host/take/save",
            Member::Probe => "/verb-host/probe",
            Member::Prepare => "/verb-host/prepare",
            Member::Picture => "/verb-host/picture",
            Member::Extract => "/verb-host/extract",
            Member::Transcribe => "/verb-host/transcribe",
            Member::Plugins => "/verb-host/plugins",
            Member::PluginsRescan => "/verb-host/plugins/rescan",
            Member::Devices => "/verb-host/devices",
            Member::DevicesStereo => "/verb-host/devices/stereo",
            Member::Onnx => "/verb-host/onnx",
            Member::Engine => "/verb-host/engine",
        }
    }

    /// The route as a line of the log names it.
    pub fn route(self) -> &'static str {
        match self {
            Member::Job => "GET /verb-host/job",
            Member::Render => "POST /verb-host/render",
            Member::Peak => "POST /verb-host/peak",
            Member::Separate => "POST /verb-host/separate",
            Member::PlaylistStems => "POST /verb-host/playlist-stems",
            Member::TakeSave => "POST /verb-host/take/save",
            Member::Probe => "POST /verb-host/probe",
            Member::Prepare => "POST /verb-host/prepare",
            Member::Picture => "POST /verb-host/picture",
            Member::Extract => "POST /verb-host/extract",
            Member::Transcribe => "POST /verb-host/transcribe",
            Member::Plugins => "GET /verb-host/plugins",
            Member::PluginsRescan => "POST /verb-host/plugins/rescan",
            Member::Devices => "GET /verb-host/devices",
            Member::DevicesStereo => "POST /verb-host/devices/stereo",
            Member::Onnx => "POST /verb-host/onnx",
            Member::Engine => "POST /verb-host/engine",
        }
    }

    /// The most bytes of body the route reads.
    pub fn limit(self) -> usize {
        match self {
            Member::Render | Member::Picture => LARGE,
            _ => SMALL,
        }
    }

    /// Whether the body names the open project in `at`, and a wrong one is a 409.
    pub fn names_project(self) -> bool {
        matches!(
            self,
            Member::Render
                | Member::Separate
                | Member::TakeSave
                | Member::Prepare
                | Member::Picture
        )
    }

    /// Whether the member reads a path of the open project; the account's own members do not.
    pub fn reads_paths(self) -> bool {
        !matches!(
            self,
            Member::Job
                | Member::Plugins
                | Member::PluginsRescan
                | Member::Devices
                | Member::DevicesStereo
                | Member::Onnx
                | Member::Engine
        )
    }

    /// Whether a job not polled for a minute is stopped; `prepare` and `picture` outlive the page.
    pub fn heartbeat(self) -> bool {
        !matches!(self, Member::Prepare | Member::Picture)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A member either files a result in the workspace (and a reader's refuses it) or is named here as one that does not.
    const READS: [Member; 11] = [
        Member::Job,
        Member::Peak,
        Member::Probe,
        Member::Transcribe,
        Member::Plugins,
        Member::PluginsRescan,
        Member::Devices,
        Member::DevicesStereo,
        Member::Onnx,
        Member::Engine,
        Member::Separate,
    ];

    #[test]
    fn every_member_is_a_writer_or_named_read_only() {
        for m in ALL {
            assert!(
                m.writes() != READS.contains(&m),
                "{m:?} is in both lists or neither"
            );
        }
    }
}
