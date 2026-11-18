//! chains/live.rs — `liveChains`: each chain and why it cannot be played on the bench's live rig, or null.
//! What a reader gets wrong: a cold-flagged live module needs the disk too, and a folder chain's node may not take a rig name.

use serde_json::{Value, json};
use song::templates::Layout;

use super::Chains;
use crate::space::Declined;

/// The rig's own node names, which a folder chain's nodes must not reuse once copied beside them.
const RIG_NODES: [&str; 4] = ["IN", "fader", "MIX", "OUT"];

/// Why chain `i` cannot be played live, or `None`; a refused template or resolution declines.
pub(crate) fn refusal_at(c: &Chains<'_>, i: usize) -> Result<Option<String>, Declined> {
    let d = &c.dirs[i];
    if let Some(m) = c.unreadable(i) {
        return Ok(Some(format!("it does not read — {m}")));
    }
    let t = c.template(i)?;
    if let Some(first) = t.missing.first() {
        return Ok(Some(format!("it lacks {first}")));
    }
    if t.steps.is_empty() && t.input.is_none() {
        return Ok(Some("it holds no step".to_owned()));
    }
    for s in &t.steps {
        if d.layout == Layout::Folder && RIG_NODES.contains(&s.name.as_str()) {
            return Ok(Some(format!(
                "its node \"{}\" has the name of one of the rig's own",
                s.name
            )));
        }
        let f = c.facts(s)?;
        if f.offline || f.cold {
            return Ok(Some(format!(
                "\"{}\" only works on audio already on disk",
                s.label
            )));
        }
    }
    Ok(None)
}

/// `refusal` of every chain, in the walk's order; a refused template or resolution declines the whole list.
pub(crate) fn refusals(c: &Chains<'_>) -> Result<Vec<Option<String>>, Declined> {
    (0..c.dirs.len()).map(|i| refusal_at(c, i)).collect()
}

/// `{chains: [{ref, unavailable}]}`.
pub fn live_chains(c: &Chains<'_>) -> Result<Value, Declined> {
    let why = refusals(c)?;
    let rows: Vec<Value> = c
        .dirs
        .iter()
        .zip(why)
        .map(|(d, u)| json!({ "ref": d.ref_, "unavailable": u }))
        .collect();
    Ok(json!({ "chains": rows }))
}
