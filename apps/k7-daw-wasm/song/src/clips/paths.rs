// clips/paths.rs — where an audio file is, and the one stat it costs; after confineTo in confine.ts.
// A reader would get wrong that the file is never opened: the Source is the confinement, so a stat says whether the path stands.

use crate::files::Clip;
use crate::read::FamilyRead;
use crate::roots::{is_absolute, spell};
use crate::source::path::PathRule;
use crate::source::path::join;
use crate::source::{Source, SourceError};
use indexmap::IndexMap;

/// A clip's id to its confined path, "" when it lies outside every mount.
pub type AudioPaths = IndexMap<String, String>;

fn source_of(clip: &Clip) -> &str {
    match clip {
        Clip::Audio(a) => &a.source.0,
        Clip::Video(v) => &v.source.0,
        Clip::Caption(c) => &c.source.0,
    }
}

/// The path of each clip's source, one stat per clip; a file that is not there keeps its path, which the reference check names (decision 154).
pub fn audio_paths(
    source: &dyn Source,
    project: &str,
    clips: &FamilyRead<Clip>,
    rule: PathRule,
) -> Result<AudioPaths, SourceError> {
    let mut out = AudioPaths::new();
    let project = spell(project, rule).unwrap_or_else(|_| project.to_owned());
    for (id, clip) in &clips.values {
        let src = source_of(clip);
        let path = if is_absolute(src, rule) {
            spell(src, rule)
        } else {
            spell(&join(&project, src), rule)
        };
        let confined = match path {
            // A NUL, or a path no spelling reaches, is no path.
            Err(_) => String::new(),
            Ok(p) => match source.stat(&p) {
                Ok(_) | Err(SourceError::NotFound | SourceError::Io(_)) => p,
                Err(SourceError::Outside) => String::new(),
                Err(e) => return Err(e),
            },
        };
        out.insert(id.clone(), confined);
    }
    Ok(out)
}
