//! jobs_preview.rs — `resolve_preview` over the TypeScript's recorded clip tables (jobs.json, 2345), and over the song crate's own compile.
//! A reader gets wrong the two kinds of mismatch: a recorded answer is the TypeScript's, and the ones that differ are the link
//! cases of DEPARTS 2345c, found by their rule below, never by a list that could swallow another difference.

use crate::support::jobs::{Tmp, section, vectors};
use k7_file_service::confine::{Disk, project_roots};
use k7_file_service::jobs::preview::{
    ClipTable, DocClip, DocSource, PreviewCtx, PreviewError, PreviewReq, disk_defs, disk_exists,
    resolve_preview,
};
use k7_file_service::libraries::Home;
use serde_json::Value;
use song::PathRule;
use song::clips::audio_def::ClipGrid;
use std::cell::Cell;

/// A clip as recorded: id, whether it is audio, its source, its window.
type Row = (String, bool, String, Option<f64>, Option<f64>);

struct Recorded {
    clips: Vec<Row>,
    grid: ClipGrid,
}

impl ClipTable for Recorded {
    fn find(&self, id: &str) -> Option<DocClip<'_>> {
        let c = self.clips.iter().find(|c| c.0 == id)?;
        Some(DocClip {
            audio: c.1,
            source: &c.2,
            source_from: c.3,
            source_to: c.4,
        })
    }

    fn grid(&self) -> ClipGrid {
        self.grid
    }
}

/// A document recorded from the TypeScript, which counts how often an audition asked for it.
struct Doc {
    table: Result<Recorded, PreviewError>,
    asked: Cell<usize>,
}

impl DocSource for Doc {
    fn table(&self) -> Result<&dyn ClipTable, PreviewError> {
        self.asked.set(self.asked.get() + 1);
        self.table
            .as_ref()
            .map(|t| t as &dyn ClipTable)
            .map_err(Clone::clone)
    }
}

fn recorded(sc: &Value, base: &str) -> Doc {
    let table = if let Some(e) = sc["doc_error"].as_object() {
        Err(PreviewError {
            message: e["message"].as_str().unwrap().replace("{base}", base),
            code: e["code"].as_str().map(str::to_owned),
        })
    } else if let Some(clips) = sc["clips"].as_array() {
        let n = |v: &Value| v.as_f64();
        let g = &sc["grid"];
        Ok(Recorded {
            clips: clips
                .iter()
                .map(|c| {
                    (
                        c["id"].as_str().unwrap().into(),
                        c["type"] == "audio",
                        c["source"].as_str().unwrap().replace("{base}", base),
                        n(&c["source_from"]),
                        n(&c["source_to"]),
                    )
                })
                .collect(),
            grid: ClipGrid {
                tempo: g["tempo"].as_f64().unwrap(),
                num: g["num"].as_f64().unwrap(),
                den: g["den"].as_f64().unwrap(),
            },
        })
    } else {
        Err(PreviewError {
            message: "no document".into(),
            code: None,
        })
    };
    Doc {
        table,
        asked: Cell::new(0),
    }
}

/// A link of the scenario; Windows makes none without Developer Mode, so its entries and the cases through them are skipped there.
#[cfg(unix)]
fn link_entry(tmp: &Tmp, p: &str, target: &str) {
    tmp.mkdir(&p[..p.rfind('/').unwrap_or(0)]);
    std::os::unix::fs::symlink(tmp.at(target), tmp.at(p)).unwrap();
}

#[cfg(not(unix))]
fn link_entry(_: &Tmp, _: &str, _: &str) {}

/// The scenario's files on disk, but for its links on a host that cannot make them.
fn materialize(tmp: &Tmp, sc: &Value) {
    for e in sc["entries"].as_array().unwrap() {
        let p = e["p"].as_str().unwrap();
        if let Some(target) = e["link"].as_str() {
            link_entry(tmp, p, target);
        } else if let Some(size) = e["size"].as_u64() {
            // `RIFF` padded with NUL, as the vectors write it.
            let mut bytes = b"RIFF".to_vec();
            bytes.resize(size as usize, 0);
            tmp.write(p, &String::from_utf8(bytes).unwrap());
        } else {
            tmp.write(p, e["t"].as_str().unwrap_or(""));
        }
    }
}





struct World {
    home: Home,
    roots: Vec<String>,
    project: Option<String>,
    workspace: String,
}

fn world(tmp: &Tmp, sc: &Value) -> World {
    let home = Home {
        studio: tmp.at("studio"),
        libraries: tmp.at("libraries"),
    };
    let project = tmp.at(sc["active"].as_str().unwrap());
    let roots = project_roots(&home, &project, &tmp.at("no-meta"), &tmp.at("no-models"));
    World {
        home,
        roots,
        project: sc["selected"].as_bool().unwrap().then_some(project),
        workspace: tmp.at("ws"),
    }
}

#[test]
fn an_audition_asks_for_the_compiled_document_only_when_it_needs_it() {
    let scenarios = section(&vectors(), "preview");
    let sc = &scenarios[0];
    let tmp = Tmp::new("jobspreviewlazy");
    materialize(&tmp, sc);
    let base = tmp.at("").trim_end_matches('/').to_owned();
    let w = world(&tmp, sc);
    let doc = recorded(sc, &base);
    let defs = disk_defs(&w.roots, PathRule::HOST);
    let ctx = PreviewCtx {
        active: w.project.as_deref(),
        workspace_root: &w.workspace,
        home: &w.home,
        roots: &w.roots,
        probe: &Disk,
        doc: &doc,
        defs: &defs,
        exists: &disk_exists,
    };
    // An absolute audio path, an empty source and a path that is no clip never ask for the document.
    for source in [format!("{base}/ws/p.k7/clips/kick.wav"), String::new()] {
        let _ = resolve_preview(&ctx, &PreviewReq::Source { source });
        assert_eq!(
            doc.asked.get(),
            0,
            "no compile for an absolute path or an empty source"
        );
    }
    // A clip id asks once, and a plain path asks once to learn it is no id.
    let _ = resolve_preview(
        &ctx,
        &PreviewReq::Clip {
            clip: "kick".into(),
        },
    );
    assert_eq!(doc.asked.get(), 1);
    let _ = resolve_preview(
        &ctx,
        &PreviewReq::Source {
            source: "clips/kick.wav".into(),
        },
    );
    assert_eq!(doc.asked.get(), 2);
}

#[test]
fn a_document_that_threw_enoent_is_a_404_and_any_other_refusal_a_400() {
    let scenarios = section(&vectors(), "preview");
    let sc = scenarios
        .iter()
        .find(|s| s["doc_error"]["code"] == "ENOENT")
        .expect("the vanishing scenario");
    let tmp = Tmp::new("jobspreviewenoent");
    materialize(&tmp, sc);
    let base = tmp.at("").trim_end_matches('/').to_owned();
    let w = world(&tmp, sc);
    let doc = recorded(sc, &base);
    let defs = disk_defs(&w.roots, PathRule::HOST);
    let ctx = PreviewCtx {
        active: w.project.as_deref(),
        workspace_root: &w.workspace,
        home: &w.home,
        roots: &w.roots,
        probe: &Disk,
        doc: &doc,
        defs: &defs,
        exists: &disk_exists,
    };
    let gone = resolve_preview(
        &ctx,
        &PreviewReq::Clip {
            clip: "kick".into(),
        },
    )
    .unwrap_err();
    assert_eq!(gone.status(), 404);
    let refused = resolve_preview(
        &ctx,
        &PreviewReq::Source {
            source: "clips/none.wav".into(),
        },
    )
    .unwrap_err();
    assert_eq!(
        (refused.status(), refused.message.as_str()),
        (400, "no audio for 'clips/none.wav'")
    );
}

