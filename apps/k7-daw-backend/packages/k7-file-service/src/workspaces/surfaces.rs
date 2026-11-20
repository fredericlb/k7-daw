//! surfaces.rs — a project's surfaces, read by the song crate's own decoder over a Source mounted at the project (1636).
//! What a reader gets wrong: a `surfaces/` reached through a link out of the project is declined, never read.

use song::source::SourceError;
use song::views::surfaces::{SurfaceType, project_surfaces as decode};
use song::{Cache, DiskSource};

use super::Surfaced;
use crate::space::Declined;

/// The warning the decoder gives for a path its Source refuses, whole folder or one file.
const OUTSIDE: &str = "outside the project's folders — skipped";

fn kind_name(kind: SurfaceType) -> &'static str {
    match kind {
        SurfaceType::Turntables => "turntables",
        SurfaceType::Timeline => "timeline",
        SurfaceType::Viewport => "viewport",
        SurfaceType::Mastering => "mastering",
    }
}

/// Every surface the project at `project` declares, in the decoder's order; a read that left the project is `Declined`.
pub fn project_surfaces(project: &str) -> Result<Vec<Surfaced>, Declined> {
    let source = DiskSource::new(&[project]).map_err(|_| Declined::LongPath)?;
    let mut cache = Cache::new();
    let read = match decode(&source, &mut cache, project) {
        Ok(r) => r,
        Err(SourceError::Outside) => return Err(Declined::SurfacesOutside),
        Err(_) => return Ok(Vec::new()),
    };
    if read.diagnostics.iter().any(|d| d.message == OUTSIDE) {
        return Err(Declined::SurfacesOutside);
    }
    Ok(read
        .surfaces
        .into_iter()
        .map(|s| Surfaced {
            id: s.id,
            kind: kind_name(s.surface.kind).into(),
            playlist: s.surface.playlist,
        })
        .collect())
}
