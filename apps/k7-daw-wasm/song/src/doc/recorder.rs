// doc/recorder.rs — the recorder a project with no timeline records its output with, placed last on the master run.
// A reader would get wrong that a project WITH a timeline gets none: it records takes on lanes. The surfaces are read by `views` (decision 643).

use super::notes::Notes;
use super::{PluginDoc, ProjectDoc};
use crate::cache::Cache;
use crate::files::Finding;
use crate::lanes::OUT_NODE;
use crate::routing::{Chain, split_port};
use crate::source::{Source, SourceError};
use crate::views::surfaces::{SurfaceType, project_surfaces};
use indexmap::IndexMap;

/// The id of the recorder, which no node of the graph names.
pub const OUTPUT_RECORDER_ID: &str = "MIX:@record";
/// The engine's own recorder.
pub const RECORDER_PLUGIN: &str = "Recorder";

/// Whether the project records its output: no timeline surface, and some run reaching OUT (decision 330).
pub(super) fn records_output(
    source: &dyn Source,
    project: &str,
    cache: &mut Cache,
    chains: &IndexMap<String, Chain>,
    notes: &mut Notes,
) -> Result<bool, SourceError> {
    let reaches = chains
        .values()
        .any(|c| c.out.iter().any(|o| split_port(&o.to).0 == OUT_NODE));
    if !reaches {
        return Ok(false);
    }
    let found = project_surfaces(source, cache, project)?;
    for d in &found.diagnostics {
        notes.push(Finding {
            file: d.file.clone(),
            line: d.line,
            col: d.col,
            message: d.message.clone(),
            severity: d.severity,
        });
    }
    Ok(!found.surfaces.iter().any(|p| p.surface.kind == SurfaceType::Timeline))
}

/// The hidden recorder, as the master run's last plugin; its levels at unity.
pub fn recorder_plugin() -> PluginDoc {
    let mut p = PluginDoc::new(OUTPUT_RECORDER_ID);
    p.plugin_type = RECORDER_PLUGIN.to_owned();
    p.name = "@record".to_owned();
    p.role = String::new();
    p.params.insert("@mix".to_owned(), 1.0);
    p.params.insert("@input-volume".to_owned(), 0.0);
    p.params.insert("@output-volume".to_owned(), 0.0);
    p
}

/// `doc` with the recorder placed last on the master run when the project records its output.
pub fn with_output_recorder(doc: &mut ProjectDoc, records: bool) {
    if records {
        doc.master_plugins.push(recorder_plugin());
    }
}
