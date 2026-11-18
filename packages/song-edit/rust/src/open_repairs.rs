// open_repairs.rs — the repairs a project gets when it is opened, in the order they run, each on its own: one that fails is said and the next runs.
// A reader would get wrong that the format stamp for seconds is not here: it runs after them, unguarded, in the server.

use crate::confine::Env;
use crate::fs::Fs;
use crate::ids::NewId;
use crate::lane_strip_plan::strip_lanes;
use crate::picture_lanes_migrate::migrate_picture_lanes;
use crate::picture_sound_migrate::fold_picture_sounds;
use crate::recorder_nodes::drop_recorder_nodes;
use crate::video_outputs::ensure_video_run;
use song::Cache;
use song::views::surfaces::{SurfaceType, project_surfaces};

/// One line the repairs have to say: a warning, or an information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub warn: bool,
    pub text: String,
}

fn warn(text: String) -> Note {
    Note { warn: true, text }
}

/// What a repair says on success: warnings listed under its label, or one line logged after the project's name.
#[derive(Default)]
struct Said {
    warnings: Vec<String>,
    log: Option<String>,
}

fn warnings(w: Vec<String>) -> Said {
    Said { warnings: w, log: None }
}

fn has_viewport(fs: &dyn Fs, dir: &str) -> Result<bool, String> {
    let all = project_surfaces(fs, &mut Cache::new(), dir).map_err(|e| e.to_string())?;
    Ok(all.surfaces.iter().any(|s| s.surface.kind == SurfaceType::Viewport))
}

/// One repair of an open, in the order they run: each later one reads what the earlier ones wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repair {
    PictureLanes,
    PictureSound,
    PictureStrips,
    RecorderNodes,
}

impl Repair {
    pub const ALL: [Repair; 4] = [
        Repair::PictureLanes,
        Repair::PictureSound,
        Repair::PictureStrips,
        Repair::RecorderNodes,
    ];

    /// What its messages call it.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Repair::PictureLanes => "picture lanes",
            Repair::PictureSound => "picture sound",
            Repair::PictureStrips => "picture strips",
            Repair::RecorderNodes => "recorder nodes",
        }
    }

    /// The verb a failure is reported with, or None for a repair that fails silently.
    #[must_use]
    pub fn failed(self) -> Option<&'static str> {
        match self {
            Repair::PictureLanes => Some("migrated"),
            Repair::PictureSound => Some("folded"),
            Repair::PictureStrips => None,
            Repair::RecorderNodes => Some("removed"),
        }
    }
}

/// What a repair said, ready for `notes_of`.
pub struct Outcome(Result<Option<Said>, String>);

impl Outcome {
    /// Whether the repair ran to its end, so what it staged may land.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.0.is_ok()
    }
}

fn recorders(fs: &mut dyn Fs, env: &Env, dir: &str) -> Result<Option<Said>, String> {
    let gone = drop_recorder_nodes(fs, env, dir).map_err(|e| e.to_string())?;
    let s = if gone.len() > 1 { "s" } else { "" };
    let log = format!(
        "recorder node{s} {} removed — the bar records the output",
        gone.join(", ")
    );
    Ok((!gone.is_empty()).then(|| Said {
        warnings: vec![],
        log: Some(log),
    }))
}

fn strips(fs: &mut dyn Fs, env: &Env, dir: &str, new_id: NewId) -> Result<Option<Said>, String> {
    if !has_viewport(fs, dir)? {
        return Ok(None);
    }
    ensure_video_run(fs, env, dir);
    let (_, _, w) = strip_lanes(fs, env, dir, &|g| g.video_tracks.clone(), false, new_id).map_err(|e| e.to_string())?;
    Ok(Some(warnings(w)))
}

/// Run one repair on `dir`; what it wrote is in `fs`, whole or, on a failure the caller drops, not at all.
pub fn run_repair(repair: Repair, fs: &mut dyn Fs, env: &Env, dir: &str, new_id: NewId) -> Outcome {
    Outcome(match repair {
        Repair::PictureLanes => migrate_picture_lanes(fs, env, dir, new_id)
            .map(|m| m.map(|m| warnings(m.warnings)))
            .map_err(|e| e.to_string()),
        Repair::PictureSound => fold_picture_sounds(fs, env, dir)
            .map(|m| m.map(|m| warnings(m.warnings)))
            .map_err(|e| e.to_string()),
        Repair::PictureStrips => strips(fs, env, dir, new_id),
        Repair::RecorderNodes => recorders(fs, env, dir),
    })
}

/// A failure to land what a repair staged, said as a failed repair is.
#[must_use]
pub fn landing_failed(message: &str) -> Outcome {
    Outcome(Err(message.to_owned()))
}

/// What a repair said as lines: its warnings, its log line, or its failure when it has a verb to report it with.
pub fn notes_of(repair: Repair, rel: &str, outcome: Outcome) -> Vec<Note> {
    let label = repair.label();
    match outcome.0 {
        Ok(None) => vec![],
        Ok(Some(said)) => {
            let mut out = vec![];
            if !said.warnings.is_empty() {
                out.push(warn(format!("{label} of {rel}:\n  {}", said.warnings.join("\n  "))));
            }
            if let Some(log) = said.log {
                out.push(Note {
                    warn: false,
                    text: format!("{rel}: {log}"),
                });
            }
            out
        }
        Err(e) => repair
            .failed()
            .map(|verb| warn(format!("{label} of {rel} not {verb}: {e}")))
            .into_iter()
            .collect(),
    }
}

/// Run every repair on one `fs`, in order: a failing one is said and the next still runs.
pub fn run_open_repairs(fs: &mut dyn Fs, env: &Env, dir: &str, rel: &str, new_id: NewId) -> Vec<Note> {
    let mut notes = vec![];
    for repair in Repair::ALL {
        let outcome = run_repair(repair, fs, env, dir, &mut *new_id);
        notes.extend(notes_of(repair, rel, outcome));
    }
    notes
}
