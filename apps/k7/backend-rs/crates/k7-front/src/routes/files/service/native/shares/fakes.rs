//! fakes.rs — what the export tests stand on: scratch folders, an account and fake ports.
//! What a reader gets wrong: a fake ffmpeg is a shell script, so the tests that use it are Unix-only.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::disk::random;
use super::{Account, BoxFut, Manifest, MixDoc, Mixdown, Ports, Stem, Views, picture};

pub fn scratch(name: &str) -> PathBuf {
    let d = crate::routes::files::service::native::tests::tmp_root()
        .join(format!("k7-shares-{name}-{}", random()));
    std::fs::create_dir_all(&d).unwrap();
    let real = std::fs::canonicalize(d).unwrap();
    // The model's spelling: no `\\?\` prefix, slashes only.
    PathBuf::from(k7_file_service::confine::spelled(&real).expect("a drive or posix temp dir"))
}

/// An account whose workspace is `base/ws`, its home `base/home`, and whose environment holds only `env`.
pub fn account(base: &Path, env: &[(&str, &str)]) -> Account {
    for d in ["ws", "home/library", "tmp"] {
        std::fs::create_dir_all(base.join(d)).unwrap();
    }
    Account {
        root: base.join("ws"),
        tmp: base.join("tmp"),
        env: env
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<BTreeMap<_, _>>(),
    }
}

/// A fake `ffmpeg` in `dir/bin` that records its environment and writes `mp3` as its output.
#[cfg(unix)]
pub fn fake_ffmpeg(dir: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let f = bin.join("ffmpeg");
    let script = format!(
        "#!/bin/sh\n/usr/bin/env > '{}'\nfor last; do :; done\nprintf mp3bytes > \"$last\"\n",
        dir.join("ffmpeg-env.txt").display()
    );
    std::fs::write(&f, script).unwrap();
    std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755)).unwrap();
    bin
}

#[derive(Default)]
pub struct Calls {
    pub mixdown: Mutex<Vec<(Value, bool)>>,
}

pub struct FakeMixdown {
    pub calls: Arc<Calls>,
    pub fail: Option<String>,
}

impl Mixdown for FakeMixdown {
    fn render<'a>(
        &'a self,
        doc: &'a Value,
        out: &'a Path,
        stems: bool,
    ) -> BoxFut<'a, Result<Manifest, String>> {
        Box::pin(async move {
            self.calls
                .mixdown
                .lock()
                .unwrap()
                .push((doc.clone(), stems));
            if let Some(e) = &self.fail {
                return Err(e.clone());
            }
            std::fs::write(out.join("mix.wav"), b"RIFFwav").unwrap();
            let mut list = Vec::new();
            if stems {
                std::fs::write(out.join("kick.wav"), b"RIFFkick").unwrap();
                list.push(Stem {
                    name: "Kick".into(),
                    file: "kick.wav".into(),
                });
            }
            Ok(Manifest {
                mix: "mix.wav".into(),
                stems: list,
            })
        })
    }
}

pub struct FakeViews;

impl Views for FakeViews {
    fn mix_doc<'a>(
        &'a self,
        _dir: &'a Path,
        timeline: Option<&'a str>,
    ) -> BoxFut<'a, Result<MixDoc, String>> {
        Box::pin(async move {
            if timeline == Some("nope") {
                return Err(k7_file_service::shares::names::no_such_timeline("nope"));
            }
            Ok(MixDoc {
                doc: json!({ "name": "Song", "tl": timeline }),
                name: "Song".into(),
            })
        })
    }
}

/// A job as the fake browser saw it: its text, its mixdown and its media keys.
pub type SeenJob = (String, Option<PathBuf>, Vec<String>);

/// A browser that records its job and writes a fixed film where the real one would.
pub struct FakeBrowser {
    pub jobs: Mutex<Vec<SeenJob>>,
    pub fail: Option<String>,
}

impl picture::Browser for FakeBrowser {
    fn render(&self, job: k7_picture::Job) -> BoxFut<'_, Result<k7_picture::Rendered, String>> {
        Box::pin(async move {
            let mut media: Vec<String> = job.media.keys().cloned().collect();
            media.sort();
            self.jobs
                .lock()
                .unwrap()
                .push((job.job.clone(), job.mix.clone(), media));
            if let Some(e) = &self.fail {
                return Err(e.clone());
            }
            let part = k7_picture::part_for(&job.out);
            std::fs::write(&part, b"WEBMfilm").unwrap();
            let mut r = k7_picture::Rendered::new(part, "video/webm".into());
            r.len = 8;
            Ok(r)
        })
    }
}

pub struct World {
    pub ports: Arc<Ports>,
    pub calls: Arc<Calls>,
    pub browser: Arc<FakeBrowser>,
}

/// Every port faked.
pub fn world() -> World {
    let calls = Arc::new(Calls::default());
    let browser = Arc::new(FakeBrowser {
        jobs: Mutex::default(),
        fail: None,
    });
    let ports = Arc::new(Ports {
        mixdown: Arc::new(FakeMixdown {
            calls: calls.clone(),
            fail: None,
        }),
        views: Arc::new(FakeViews),
        browser: browser.clone(),
    });
    World {
        ports,
        calls,
        browser,
    }
}
