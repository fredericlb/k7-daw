// node.rs — a lane's gutter menu: its module's other attribute sets, and its freeze.
// A reader would get wrong that this is derived from the catalogue: it is this project's data, so every row is written out with its own words.

use super::{Item, Text};
use serde::{Deserialize, Serialize};

/// Where a node is frozen: at its own output, or inside somebody else's render.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum Frozen {
    At,
    Inside,
}

/// What a lane's gutter badge opens on: the lane's node, as the timeline holds it.
#[derive(Clone, Debug, Default, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NodeMenuTarget {
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub module: Option<String>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub alternatives: Option<Vec<String>>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub alternative: Option<String>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub frozen: Option<Frozen>,
}

fn row(label: Text, description: Text, command: String, complete: bool, verb: &str) -> Item {
    Item {
        label,
        description,
        command,
        complete,
        confirm: None,
        verb: verb.to_owned(),
        sentence: None,
    }
}

/// A lane's gutter menu: the other sets of its module's attributes, then a new one and a copy, then the freeze.
#[must_use]
pub fn node_menu(n: &NodeMenuTarget) -> Vec<Item> {
    let Some(module) = n.module.as_deref().filter(|m| !m.is_empty()) else {
        return vec![];
    };
    let current = n.alternative.as_deref().unwrap_or("1");
    let mut out: Vec<Item> = n
        .alternatives
        .iter()
        .flatten()
        .map(|name| {
            // A dot marks the one that is on, and the gap on the other rows is two non-breaking spaces, HTML collapsing ordinary ones.
            let prefix = if name == current { "\u{2022} " } else { "\u{a0}\u{a0}" };
            let label = if name == "1" {
                Text::msg("node.altOwn")
            } else {
                Text::plain(name.clone())
            };
            row(
                label.with_prefix(prefix),
                if name == current {
                    Text::msg("node.altOn")
                } else {
                    Text::msg_with("node.altPlay", "name", name.clone())
                },
                format!("node alternative select {module} {name}"),
                true,
                "node alternative select",
            )
        })
        .collect();
    out.push(row(
        Text::msg("node.altNew"),
        Text::msg("node.altNewHelp"),
        format!("node alternative create {module} "),
        false,
        "node alternative create",
    ));
    out.push(row(
        Text::msg("node.altCopy"),
        Text::msg("node.altCopyHelp"),
        format!("node alternative copy {module} "),
        false,
        "node alternative copy",
    ));
    // Freeze: one entry, run on the click, and none at all on a node inside somebody else's render.
    if n.frozen != Some(Frozen::Inside) {
        let at = n.frozen == Some(Frozen::At);
        out.push(row(
            Text::msg(if at { "node.unfreeze" } else { "node.freeze" }),
            Text::msg(if at { "node.unfreezeHelp" } else { "node.freezeHelp" }),
            format!("node freeze {module}"),
            true,
            "node freeze",
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(alts: &[&str], current: Option<&str>, frozen: Option<Frozen>) -> NodeMenuTarget {
        NodeMenuTarget {
            module: Some("lead".into()),
            alternatives: Some(alts.iter().map(|s| (*s).to_owned()).collect()),
            alternative: current.map(Into::into),
            frozen,
        }
    }

    #[test]
    fn a_node_without_a_module_has_no_menu() {
        assert!(node_menu(&NodeMenuTarget::default()).is_empty());
    }

    #[test]
    fn each_set_is_a_row_the_current_one_marked_then_new_copy_and_freeze() {
        let rows = node_menu(&target(&["1", "wide"], Some("wide"), None));
        let cmds: Vec<&str> = rows.iter().map(|r| r.command.as_str()).collect();
        assert_eq!(
            cmds,
            [
                "node alternative select lead 1",
                "node alternative select lead wide",
                "node alternative create lead ",
                "node alternative copy lead ",
                "node freeze lead"
            ]
        );
        assert_eq!(rows[0].label.prefix, "\u{a0}\u{a0}");
        assert_eq!(rows[0].label.key.as_deref(), Some("node.altOwn"));
        assert_eq!(rows[1].label.prefix, "\u{2022} ");
        assert_eq!(rows[1].description.key.as_deref(), Some("node.altOn"));
        assert_eq!(rows[0].description.key.as_deref(), Some("node.altPlay"));
        assert!(!rows[2].complete && rows[1].complete);
    }

    #[test]
    fn a_frozen_node_offers_to_unfreeze_and_one_inside_a_render_offers_nothing() {
        let rows = node_menu(&target(&[], None, Some(Frozen::At)));
        assert_eq!(rows.last().map(|r| r.label.key.as_deref()), Some(Some("node.unfreeze")));
        let rows = node_menu(&target(&[], None, Some(Frozen::Inside)));
        assert!(rows.iter().all(|r| r.verb != "node freeze"));
    }
}
