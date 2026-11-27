// group_edge.rs — which widgets share a group's LEFT edge with its name, and how far each is drawn in.
// The inset is drawing only: the file, the grid and the builder's checks keep every widget on its cells.
// A reader would get wrong that a group holds nothing: membership is geometry. Rationale: ui-chains-react's docs/widgets.md, "The group".

use crate::panel::{Widget, Widgets};
use indexmap::IndexMap;

/// How far a control on a group's named edge is drawn in: clear of the 6px name and the 8px dot.
pub const GROUP_EDGE_INSET: f64 = 12.0;

/// …and a field there, whose caption is flush left and so meets the name first.
pub const GROUP_FIELD_INSET: f64 = 16.0;

// The widgets drawn as a field: they fill their box's width, so they take the wider inset.
const FIELDS: [&str; 3] = ["dropdown", "filepicker", "number"];

fn span(n: Option<f64>) -> f64 {
    n.unwrap_or(1.0).max(1.0)
}

fn inside(a: &Widget, g: &Widget) -> bool {
    a.col >= g.col
        && a.row >= g.row
        && a.col + span(a.col_span) <= g.col + span(g.col_span)
        && a.row + span(a.row_span) <= g.row + span(g.row_span)
}

fn is_set(s: &Option<String>) -> bool {
    s.as_deref().is_some_and(|s| !s.is_empty())
}

/// Each widget drawn inset, by id: framed by a group that draws a name or a switch, on its left edge.
pub fn group_edge_insets(widgets: &Widgets) -> IndexMap<String, f64> {
    let edges: Vec<&Widget> = widgets
        .values()
        .filter(|g| g.kind == "group" && (is_set(&g.label) || is_set(&g.bind)))
        .collect();
    let mut out = IndexMap::new();
    for (id, w) in widgets {
        if w.kind == "group" {
            continue;
        }
        if edges.iter().any(|e| w.col == e.col && inside(w, e)) {
            let inset = if FIELDS.contains(&w.kind.as_str()) {
                GROUP_FIELD_INSET
            } else {
                GROUP_EDGE_INSET
            };
            out.insert(id.clone(), inset);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(kind: &str, col: f64, row: f64, col_span: f64, row_span: Option<f64>) -> Widget {
        Widget {
            col_span: Some(col_span),
            row_span,
            ..Widget::at(kind, col, row)
        }
    }

    fn panel() -> Widgets {
        let group = Widget {
            label: Some("FILTER".into()),
            ..at("group", 2.0, 0.0, 6.0, Some(3.0))
        };
        [
            ("g", group),
            ("a", at("knob", 2.0, 0.0, 2.0, Some(2.0)).bound("Cutoff", "")),
            ("b", at("knob", 4.0, 0.0, 2.0, Some(2.0)).bound("Res", "")),
            ("d", at("dropdown", 6.0, 0.0, 2.0, None).bound("Type", "")),
            ("out", at("knob", 0.0, 0.0, 2.0, Some(2.0)).bound("Gain", "")),
        ]
        .into_iter()
        .map(|(k, w)| (k.to_owned(), w))
        .collect()
    }

    fn pairs(m: &IndexMap<String, f64>) -> Vec<(&str, f64)> {
        m.iter().map(|(k, v)| (k.as_str(), *v)).collect()
    }

    #[test]
    fn the_controls_in_a_named_groups_first_column_are_inset() {
        assert_eq!(pairs(&group_edge_insets(&panel())), [("a", GROUP_EDGE_INSET)]);
    }

    #[test]
    fn a_field_on_the_edge_is_inset_further_whatever_its_row() {
        let mut corner = panel();
        corner["a"].kind = "dropdown".into();
        assert_eq!(pairs(&group_edge_insets(&corner)), [("a", GROUP_FIELD_INSET)]);
        let mut lower = panel();
        lower["a"] = at("number", 2.0, 2.0, 2.0, None).bound("Drive", "");
        assert_eq!(pairs(&group_edge_insets(&lower)), [("a", GROUP_FIELD_INSET)]);
    }

    #[test]
    fn a_switch_insets_too_and_a_bare_frame_does_not() {
        let mut bare = panel();
        bare["g"].label = None;
        assert!(group_edge_insets(&bare).is_empty());
        let mut sw = panel();
        sw["g"].label = None;
        sw["g"].bind = Some("On".into());
        assert_eq!(pairs(&group_edge_insets(&sw)), [("a", GROUP_EDGE_INSET)]);
    }
}
