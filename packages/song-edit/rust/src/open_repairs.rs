// open_repairs.rs — what a project gets when it is opened: the picture lanes' strips and the video run a viewport needs.
// A reader would get wrong that the format stamp for seconds is not here: it runs after, unguarded, in the server.

use crate::confine::Env;
use crate::fs::Fs;
use crate::ids::NewId;
use crate::lane_strip_plan::strip_lanes;
use crate::video_outputs::ensure_video_run;
use song::Cache;
use song::views::surfaces::{SurfaceType, project_surfaces};

/// One line the repair has to say: a warning, or an information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub warn: bool,
    pub text: String,
}

fn has_viewport(fs: &dyn Fs, dir: &str) -> Result<bool, String> {
    let all = project_surfaces(fs, &mut Cache::new(), dir).map_err(|e| e.to_string())?;
    Ok(all.surfaces.iter().any(|s| s.surface.kind == SurfaceType::Viewport))
}

/// The strips of the picture lanes of a project with a viewport, after its video run; what is written is in `fs`, whole or, on an Err the caller drops, not at all.
pub fn repair_picture_strips(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    rel: &str,
    new_id: NewId,
) -> Result<Vec<Note>, String> {
    if !has_viewport(fs, dir)? {
        return Ok(vec![]);
    }
    ensure_video_run(fs, env, dir);
    let (_, _, w) = strip_lanes(fs, env, dir, &|g| g.video_tracks.clone(), false, new_id).map_err(|e| e.to_string())?;
    Ok(if w.is_empty() {
        vec![]
    } else {
        vec![Note {
            warn: true,
            text: format!("picture strips of {rel}:\n  {}", w.join("\n  ")),
        }]
    })
}
