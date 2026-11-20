// sequence_loops.rs — after a sequence is DOUBLED, the loop count of every placement of it is halved, so the arrangement does not move.
// A reader would get wrong what is left alone: an odd count and a placement with a window of its own are reported, never halved.

use crate::chain_item_edit::{chain_files, edit_chain_item_in, event_item_ids, lane_folder};
use crate::doc_names::docs_in;
use crate::error::Res;
use crate::fs::Fs;
use crate::paths::join;
use crate::sugar::strip_doc_ext;
use crate::track_file::{KeptLoop, halve_sequence_loops};
use crate::workspace_doc::{ws_text, ws_write_text};
use k7x::edit::Document;

/// What the halving reached, and what it deliberately did not.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoopHalving {
    pub files: Vec<String>,
    pub halved: usize,
    /// One entry per placement left as it was, with the lane it is on.
    pub kept: Vec<(KeptLoop, String)>,
}

/// The same over a chain file: every node item holding events of its own.
fn halve_in_chain(doc: &mut Document, ref_: &str) -> Res<(usize, Vec<KeptLoop>)> {
    let (mut halved, mut kept) = (0, vec![]);
    for id in event_item_ids(doc.text()) {
        edit_chain_item_in(doc, &id, |item| {
            let (h, k) = halve_sequence_loops(item, ref_)?;
            halved += h;
            kept.extend(k);
            Ok(())
        })?;
    }
    Ok((halved, kept))
}

/// Halves the loop of every placement of `ref_` across the project's lanes; `snapshot` sees a path before it is overwritten.
///
/// # Errors
/// A chain folder that does not list, or a write's.
pub fn halve_loops_of(
    fs: &mut dyn Fs,
    dir: &str,
    ref_: &str,
    mut snapshot: Option<&mut dyn FnMut(&str)>,
) -> Res<LoopHalving> {
    let mut out = LoopHalving::default();
    let folder = lane_folder(fs, dir);
    let names = docs_in(fs, &join(&[dir, folder]));
    let mut rels: Vec<String> = names.iter().map(|f| format!("{folder}/{f}")).collect();
    rels.extend(chain_files(fs, dir)?);
    for rel in rels {
        let path = join(&[dir, &rel]);
        let Ok(text) = ws_text(fs, &path) else { continue };
        let mut doc = Document::new(&text);
        let is_chain = !rel.starts_with(&format!("{folder}/"));
        let (halved, kept) = if is_chain {
            halve_in_chain(&mut doc, ref_)?
        } else {
            halve_sequence_loops(&mut doc.at(&[]), ref_)?
        };
        let lane = strip_doc_ext(rel.split_once('/').map_or(rel.as_str(), |(_, r)| r)).to_owned();
        out.kept.extend(kept.into_iter().map(|k| (k, lane.clone())));
        if halved == 0 {
            continue;
        }
        if let Some(s) = snapshot.as_deref_mut() {
            s(&path);
        }
        ws_write_text(fs, &path, doc.text())?;
        out.files.push(rel);
        out.halved += halved;
    }
    Ok(out)
}
