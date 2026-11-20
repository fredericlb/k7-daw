//! picture.rs — the picture job's settings and text, and the browser it runs in (2504, 2551, 2554).
//! What a reader gets wrong: the scene is the text the picture's writer made and is spliced in verbatim; a number is spelt as `String(n)`, never by serde.

use std::path::{Path, PathBuf};

use k7_js::number::{math_round, to_string};

use crate::paths::join;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Webm,
    Mp4,
}

impl Format {
    pub fn as_str(self) -> &'static str {
        match self {
            Format::Webm => "webm",
            Format::Mp4 => "mp4",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Size {
    /// A height; the width follows the viewport's aspect.
    Height(f64),
    Both(f64, f64),
}

/// `ExportQuality`; anything else, an empty string included, is no setting and is never written to the job.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quality {
    Low,
    Medium,
    High,
    Max,
}

impl Quality {
    pub fn parse(s: &str) -> Option<Quality> {
        match s {
            "low" => Some(Quality::Low),
            "medium" => Some(Quality::Medium),
            "high" => Some(Quality::High),
            "max" => Some(Quality::Max),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Quality::Low => "low",
            Quality::Medium => "medium",
            Quality::High => "high",
            Quality::Max => "max",
        }
    }
}

/// What a server render is asked for (`PictureRenderOptions`): each setting is optional.
#[derive(Clone, Debug, PartialEq)]
pub struct VideoSettings {
    pub format: Format,
    pub audio: bool,
    pub size: Option<Size>,
    pub quality: Option<Quality>,
    pub from: Option<f64>,
    pub to: Option<f64>,
}

/// The range and file size the settings resolve to on a scene.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resolved {
    pub from: f64,
    pub frame_count: f64,
    pub size: Option<[f64; 2]>,
}

/// `resolveVideoSettings`: the whole song by default, a height alone taking the viewport's aspect.
pub fn resolve_video_settings(
    s: &VideoSettings,
    viewport: [f64; 2],
    fps: f64,
    duration: f64,
) -> Resolved {
    let from = s.from.unwrap_or(0.0);
    let to = s.to.unwrap_or(duration);
    // `Math.max` takes NaN when either is NaN, which `f64::max` does not.
    let n = math_round((to - from) * fps);
    let frame_count = if n.is_nan() || n >= 1.0 { n } else { 1.0 };
    let size = s.size.map(|z| match z {
        Size::Both(w, h) => [w, h],
        Size::Height(h) => [math_round((h * viewport[0]) / viewport[1]), h],
    });
    Resolved {
        from,
        frame_count,
        size,
    }
}

/// A number as `JSON.stringify` writes it: `String(n)`, and `null` for NaN and the infinities.
fn num(x: f64) -> String {
    if x.is_finite() {
        to_string(x)
    } else {
        "null".into()
    }
}

/// The job's text byte for byte as the TypeScript's `JSON.stringify` writes it, the scene spliced as it is.
pub fn job_text(scene: &str, s: &VideoSettings, r: &Resolved, audio: bool) -> String {
    let mut out = format!(
        "{{\"scene\":{scene},\"format\":\"{}\",\"audio\":{audio},\"from\":{},\"frameCount\":{}",
        s.format.as_str(),
        num(r.from),
        num(r.frame_count)
    );
    if let Some([w, h]) = r.size {
        out.push_str(&format!(",\"size\":[{},{}]", num(w), num(h)));
    }
    if let Some(q) = s.quality {
        out.push_str(&format!(",\"quality\":\"{}\"", q.as_str()));
    }
    out.push('}');
    out
}

/// The system's Chromium, looked for after Playwright's: the image's, then a desktop's.
pub const SYSTEM_CHROMIUM: [&str; 4] = [
    "/usr/bin/chromium-browser",
    "/usr/bin/chromium",
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
];

/// A `chromium-<revision>` folder's browser, in each layout Playwright has used.
const PLAYWRIGHT_BINARIES: [&str; 8] = [
    "chrome-linux64/chrome",
    "chrome-linux/chrome",
    "chrome-win64/chrome.exe",
    "chrome-win/chrome.exe",
    "chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing",
    "chrome-mac-x64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing",
    "chrome-mac/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing",
    "chrome-mac/Chromium.app/Contents/MacOS/Chromium",
];

type Env<'a> = &'a dyn Fn(&str) -> Option<String>;

/// Where Playwright keeps its browsers, every platform's spelling.
fn playwright_roots(env: Env) -> Vec<String> {
    let get = |k: &str| env(k).filter(|v| !v.is_empty());
    if let Some(named) = get("PLAYWRIGHT_BROWSERS_PATH").filter(|n| n != "0") {
        return vec![named];
    }
    let home = get("HOME")
        .or_else(|| get("USERPROFILE"))
        .unwrap_or_default();
    let cache = get("XDG_CACHE_HOME").unwrap_or_else(|| {
        if home.is_empty() {
            String::new()
        } else {
            join(&[&home, ".cache"])
        }
    });
    [
        get("LOCALAPPDATA").map_or_else(String::new, |l| join(&[&l, "ms-playwright"])),
        if home.is_empty() {
            String::new()
        } else {
            join(&[&home, "Library", "Caches", "ms-playwright"])
        },
        join(&[&cache, "ms-playwright"]),
    ]
    .into_iter()
    .filter(|r| !r.is_empty() && r != "ms-playwright")
    .collect()
}

/// The browser to render in: `STUDIO_CHROMIUM`, then the newest Playwright's, then the system's; else the sentence saying where it looked.
pub fn find_chromium(
    env: Env,
    exists: &dyn Fn(&Path) -> bool,
    list: &dyn Fn(&Path) -> Option<Vec<String>>,
    system: &[&str],
) -> Result<PathBuf, String> {
    if let Some(named) = env("STUDIO_CHROMIUM").filter(|v| !v.is_empty()) {
        if exists(Path::new(&named)) {
            return Ok(PathBuf::from(named));
        }
        return Err(format!("STUDIO_CHROMIUM names {named}, which is not there"));
    }
    let roots = playwright_roots(env);
    for root in &roots {
        let Some(names) = list(Path::new(root)) else {
            continue;
        };
        let mut revisions: Vec<(f64, &String)> = names
            .iter()
            .filter_map(|n| {
                let digits = n.strip_prefix("chromium-")?;
                (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
                    .then(|| (digits.parse::<f64>().unwrap_or(f64::INFINITY), n))
            })
            .collect();
        revisions.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (_, rev) in revisions {
            for bin in PLAYWRIGHT_BINARIES {
                let at = join(&[root, rev, bin]);
                if exists(Path::new(&at)) {
                    return Ok(PathBuf::from(at));
                }
            }
        }
    }
    if let Some(hit) = system.iter().find(|p| exists(Path::new(p))) {
        return Ok(PathBuf::from(hit));
    }
    let looked = if roots.is_empty() {
        "no folder".to_owned()
    } else {
        roots.join(", ")
    };
    Err(format!(
        "no Chromium to render in: STUDIO_CHROMIUM is unset, Playwright has none under {looked}, and none is at {} — install one, or set STUDIO_CHROMIUM to it",
        system.join(", ")
    ))
}
