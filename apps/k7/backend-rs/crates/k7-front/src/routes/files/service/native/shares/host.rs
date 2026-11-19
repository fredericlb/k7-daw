//! host.rs — what the picture verb's render needs of the shares: its ports, its settings, its render page (2550 to 2556, 2561).
//! What a reader gets wrong: a picture render makes no share call but the mixdown.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use k7_file_service::shares::picture::{Format, Quality, Size, VideoSettings};
use serde_json::Value;

use super::picture::Browser;
use super::ports::JobsMixdown;
use super::{Ports, Views};
use crate::routes::files::service::native::jobs::Jobs;

/// Where the render page is built: `K7_RENDER_PAGE`, else the build a checkout keeps beside the crate (as `media.picture` finds it).
pub fn script(env: &dyn Fn(&str) -> Option<String>, code_root: &Path) -> PathBuf {
    match env("K7_RENDER_PAGE").filter(|v| !v.is_empty()) {
        Some(p) => PathBuf::from(p),
        // Built by `page.mjs` and ignored by git, so the path is joined in parts rather than named as a file.
        None => code_root
            .join("apps/k7/backend-rs/crates/k7-picture")
            .join("page")
            .join("renderPage.js"),
    }
}

/// The ports a picture render calls: the mixdown, the views and the browser.
pub fn ports_of(jobs: Arc<Jobs>, views: Arc<dyn Views>, browser: Arc<dyn Browser>) -> Ports {
    Ports {
        mixdown: Arc::new(JobsMixdown(jobs)),
        views,
        browser,
    }
}

/// `PictureRenderOptions` as a client sends them: each setting optional, unknown ones ignored.
pub fn settings(opts: &Value) -> VideoSettings {
    let num = |v: &Value| v.as_f64();
    let size = match (num(&opts["size"]["width"]), num(&opts["size"]["height"])) {
        (Some(w), Some(h)) => Some(Size::Both(w, h)),
        (None, Some(h)) => Some(Size::Height(h)),
        _ => None,
    };
    VideoSettings {
        format: if opts["format"] == "mp4" {
            Format::Mp4
        } else {
            Format::Webm
        },
        audio: opts["audio"].as_bool().unwrap_or(false),
        size,
        quality: opts["quality"].as_str().and_then(Quality::parse),
        from: num(&opts["from"]),
        to: num(&opts["to"]),
    }
}
