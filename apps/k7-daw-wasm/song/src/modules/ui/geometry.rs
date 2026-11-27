// geometry.rs — the widget grid's rules over loose JSON: groups framing widgets, the cells a widget covers, a panel's two shapes.
// A reader would get wrong that a group holds nothing: membership is geometry, never nesting. Pages: apps/k7-daw-frontend/packages/ui-module-builder/docs/layouts.md.

use super::loose::{Obj, bind, entries, jnum, kind, num, obj, span, with};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::collections::{BTreeSet, HashMap};

/// The horizontal layout's height, and the vertical one's width.
pub const LAYOUT_MAX: usize = 4;

struct Boxed {
    col: f64,
    row: f64,
    cs: f64,
    rs: f64,
}

fn boxed(w: &Value) -> Boxed {
    Boxed {
        col: num(w, "col"),
        row: num(w, "row"),
        cs: span(w, "colSpan"),
        rs: span(w, "rowSpan"),
    }
}

/// Whether two boxes share at least one cell.
pub fn overlaps(a: &Value, b: &Value) -> bool {
    let (a, b) = (boxed(a), boxed(b));
    a.col < b.col + b.cs && b.col < a.col + a.cs && a.row < b.row + b.rs && b.row < a.row + a.rs
}

/// Whether `inner` lies wholly inside `outer`.
pub fn inside(inner: &Value, outer: &Value) -> bool {
    let (i, o) = (boxed(inner), boxed(outer));
    i.col >= o.col && i.row >= o.row && i.col + i.cs <= o.col + o.cs && i.row + i.rs <= o.row + o.rs
}

/// Whether two widgets collide: they overlap, unless one is a group wholly framing the other (never two groups).
pub fn clash(a: &Value, b: &Value) -> bool {
    if !overlaps(a, b) {
        return false;
    }
    let (ga, gb) = (kind(a) == "group", kind(b) == "group");
    if ga && !gb {
        return !inside(b, a);
    }
    if gb && !ga {
        return !inside(a, b);
    }
    true
}

/// The ids of the widgets `group` frames: every non-group wholly inside its box.
pub fn framed_by(widgets: &Obj, group: &Value) -> Vec<String> {
    widgets
        .iter()
        .filter(|(_, w)| kind(w) != "group" && inside(w, group))
        .map(|(id, _)| id.clone())
        .collect()
}

/// The id of the group that frames widget `id`, if any: at most one, since groups never overlap.
pub fn group_of(widgets: &Obj, id: &str) -> Option<String> {
    let w = widgets.get(id)?;
    if kind(w) == "group" {
        return None;
    }
    widgets
        .iter()
        .find(|(_, g)| kind(g) == "group" && inside(w, g))
        .map(|(id, _)| id.clone())
}

/// Widgets of one page with their ids.
pub type Pairs<'a> = Vec<(&'a String, &'a Value)>;

/// A number as a template literal prints it.
pub fn js_num(n: f64) -> String {
    match jnum(n) {
        Value::Number(n) => n.to_string(),
        _ => "NaN".to_owned(),
    }
}

/// Every way a set of widgets drawn together can break a grid: out of bounds, or on top of another.
pub fn grid_issues(cols: f64, rows: f64, place: &str, ws: &Pairs) -> Vec<String> {
    let mut out = Vec::new();
    for (id, w) in ws {
        let b = boxed(w);
        if b.col < 0.0 || b.row < 0.0 || b.col + b.cs > cols || b.row + b.rs > rows {
            out.push(format!(
                "{place}: `{id}` runs outside the {}×{} grid",
                js_num(cols),
                js_num(rows)
            ));
        }
    }
    for i in 0..ws.len() {
        for j in i + 1..ws.len() {
            if !clash(ws[i].1, ws[j].1) {
                continue;
            }
            let (a, b) = (ws[i].0, ws[j].0);
            out.push(if kind(ws[i].1) == "group" || kind(ws[j].1) == "group" {
                format!(
                    "{place}: `{a}` and `{b}` cross — a widget lies wholly inside a group or wholly outside it, and groups never overlap"
                )
            } else {
                format!("{place}: `{a}` overlaps `{b}`")
            });
        }
    }
    out
}

/// What a page offers: its controls' binds, counted; frames and labels aside.
fn offered<'a>(ws: &Pairs<'a>) -> (HashMap<&'a str, usize>, Vec<&'a str>) {
    let mut counts = HashMap::new();
    let mut order = Vec::new();
    for (_, w) in ws {
        if let Some(b) = bind(w).filter(|_| !matches!(kind(w), "group" | "label")) {
            if !counts.contains_key(b) {
                order.push(b);
            }
            *counts.entry(b).or_insert(0) += 1;
        }
    }
    (counts, order)
}

/// The pages of a layout, each with the widgets every page shares: `(name, widgets drawn)`.
pub fn pages(layout: &Value) -> Vec<(String, Pairs<'_>)> {
    let top = entries(obj(layout, "widgets"));
    let tabs = layout.get("tabs").and_then(Value::as_array).filter(|t| !t.is_empty());
    let Some(tabs) = tabs else {
        return vec![("panel".to_owned(), top)];
    };
    tabs.iter()
        .map(|t| {
            let mut ws = top.clone();
            ws.extend(entries(obj(t, "widgets")));
            let name = t.get("name").and_then(Value::as_str).unwrap_or("undefined");
            (format!("tab {name}"), ws)
        })
        .collect()
}

/// Every way a panel's two shapes break the rules: sizes, the twin's own geometry, and controls one shape lacks.
pub fn twin_issues(ui: &Value) -> Vec<String> {
    let mut issues: Vec<String> = Vec::new();
    let max = LAYOUT_MAX as f64;
    if num(ui, "rows") > max {
        issues.push(format!(
            "the panel is {} rows tall: the horizontal layout holds at most {LAYOUT_MAX}",
            js_num(num(ui, "rows"))
        ));
    }
    let Some(v) = ui.get("vertical").filter(|v| !v.is_null()) else {
        if num(ui, "cols") > max {
            issues.push(format!(
                "the panel is {} columns wide and has no `vertical` layout: one of at most {LAYOUT_MAX} columns is needed",
                js_num(num(ui, "cols"))
            ));
        }
        return issues;
    };
    if num(v, "cols") > max {
        issues.push(format!(
            "the vertical layout is {} columns wide: it holds at most {LAYOUT_MAX}",
            js_num(num(v, "cols"))
        ));
    }
    let hp = pages(ui);
    let vp = pages(v);
    for (place, ws) in &vp {
        issues.extend(grid_issues(
            num(v, "cols"),
            num(v, "rows"),
            &format!("vertical {place}"),
            ws,
        ));
    }
    for (i, (place, ws)) in hp.iter().enumerate() {
        let empty = Vec::new();
        let twin = vp.get(i).map_or(&empty, |p| &p.1);
        let (a, mut order) = offered(ws);
        let (b, rest) = offered(twin);
        for k in rest {
            if !order.contains(&k) {
                order.push(k);
            }
        }
        let differ: Vec<String> = order
            .iter()
            .filter(|k| a.get(*k).copied().unwrap_or(0) != b.get(*k).copied().unwrap_or(0))
            .map(|k| format!("`{k}`"))
            .collect();
        if !differ.is_empty() {
            issues.push(format!(
                "{place}: the vertical layout does not offer the same controls — {}",
                differ.join(", ")
            ));
        }
    }
    // A screen is drawn by the panel's script, keyed by its id: a twin's screen must be one the script already draws.
    let mut screens: BTreeSet<&str> = BTreeSet::new();
    let tabs = ui.get("tabs").and_then(Value::as_array).into_iter().flatten();
    for m in std::iter::once(obj(ui, "widgets")).chain(tabs.map(|t| obj(t, "widgets"))) {
        for (id, w) in entries(m) {
            if kind(w) == "lcdscreen" {
                screens.insert(id);
            }
        }
    }
    for (_, ws) in &vp {
        for (id, w) in ws {
            if kind(w) == "lcdscreen" && !screens.contains(id.as_str()) {
                issues.push(format!(
                    "vertical: screen `{id}` is not one of the panel's screens — reuse the panel's id, which its draw function is keyed by"
                ));
            }
        }
    }
    let mut seen: Vec<String> = Vec::new();
    issues.retain(|i| {
        let fresh = !seen.contains(i);
        if fresh {
            seen.push(i.clone());
        }
        fresh
    });
    issues
}

fn pack(taken: &mut Vec<Vec<bool>>, ws: Option<&Obj>) -> Value {
    let mut order: Vec<(&String, &Value)> = entries(ws).into_iter().filter(|(_, w)| kind(w) != "group").collect();
    order.sort_by(|(_, a), (_, b)| {
        let (dr, dc) = (num(a, "row") - num(b, "row"), num(a, "col") - num(b, "col"));
        (if dr != 0.0 { dr } else { dc })
            .partial_cmp(&0.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut out = Map::new();
    for (id, w) in order {
        let cw = span(w, "colSpan").min(LAYOUT_MAX as f64) as usize;
        let ch = span(w, "rowSpan") as usize;
        let free = |taken: &[Vec<bool>], r: usize, c: usize| {
            (r..r + ch).all(|y| (c..c + cw).all(|x| !taken.get(y).and_then(|row| row.get(x)).copied().unwrap_or(false)))
        };
        let mut r = 0;
        let c = loop {
            if let Some(c) = (0..=LAYOUT_MAX - cw).find(|c| free(taken, r, *c)) {
                break c;
            }
            r += 1;
        };
        for y in r..r + ch {
            while taken.len() <= y {
                taken.push(Vec::new());
            }
            if taken[y].len() < c + cw {
                taken[y].resize(c + cw, false);
            }
            taken[y][c..c + cw].fill(true);
        }
        let mut placed = with(&with(w, "col", jnum(c as f64)), "row", jnum(r as f64));
        if cw > 1 {
            placed = with(&placed, "colSpan", jnum(cw as f64));
        }
        out.insert(id.clone(), placed);
    }
    Value::Object(out)
}

/// A first vertical layout from the panel: every control at its size (at most 4 wide), packed into 4 columns in reading order.
pub fn reflow_narrow(ui: &Value) -> Value {
    let mut shared: Vec<Vec<bool>> = Vec::new();
    let widgets = pack(&mut shared, obj(ui, "widgets"));
    // Each tab packs around the shared widgets, which every tab draws.
    let tabs: Vec<Value> = ui
        .get("tabs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|t| {
            let ws = pack(&mut shared.clone(), obj(t, "widgets"));
            json!({ "name": t.get("name").cloned().unwrap_or(Value::Null), "widgets": ws })
        })
        .collect();
    let mut rows = 1.0_f64;
    let maps = std::iter::once(&widgets).chain(tabs.iter().filter_map(|t| t.get("widgets")));
    for m in maps {
        for w in m.as_object().into_iter().flat_map(|m| m.values()) {
            rows = rows.max(num(w, "row") + span(w, "rowSpan"));
        }
    }
    let mut out = Map::new();
    out.insert("cols".into(), jnum(LAYOUT_MAX as f64));
    out.insert("rows".into(), jnum(rows));
    out.insert("widgets".into(), widgets);
    if !tabs.is_empty() {
        out.insert("tabs".into(), Value::Array(tabs));
    }
    Value::Object(out)
}

/// The vertical layout's tabs brought in line with the panel's: renamed by position when the count holds, matched by name when it does not.
pub fn align_tabs(ui: &Value) -> Value {
    let Some(v) = ui.get("vertical").filter(|v| !v.is_null()) else {
        return ui.clone();
    };
    let tabs: &[Value] = ui.get("tabs").and_then(Value::as_array).map_or(&[], Vec::as_slice);
    let mine: &[Value] = v.get("tabs").and_then(Value::as_array).map_or(&[], Vec::as_slice);
    let next: Vec<Value> = tabs
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let found = if mine.len() == tabs.len() {
                mine.get(i)
            } else {
                mine.iter().find(|x| x.get("name") == t.get("name"))
            };
            let widgets = found
                .and_then(|f| f.get("widgets"))
                .cloned()
                .unwrap_or_else(|| json!({}));
            json!({ "name": t.get("name").cloned().unwrap_or(Value::Null), "widgets": widgets })
        })
        .collect();
    let mut rest = v.as_object().cloned().unwrap_or_default();
    rest.shift_remove("tabs");
    if !next.is_empty() {
        rest.insert("tabs".into(), Value::Array(next));
    }
    with(ui, "vertical", Value::Object(rest))
}

/// One cell of the grid.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Cell {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub col: f64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub row: f64,
}

fn covers(w: &Value, c: f64, r: f64) -> bool {
    let b = boxed(w);
    c >= b.col && c < b.col + b.cs && r >= b.row && r < b.row + b.rs
}

/// The first cell in the box no widget covers, row-major; a group frames cells rather than filling them.
pub fn first_free(widgets: &Obj, col: f64, row: f64, cols: f64, rows: f64) -> Cell {
    let ws: Vec<&Value> = widgets.values().filter(|w| kind(w) != "group").collect();
    let mut r = row;
    while r < row + rows {
        let mut c = col;
        while c < col + cols {
            if !ws.iter().any(|w| covers(w, c, r)) {
                return Cell { col: c, row: r };
            }
            c += 1.0;
        }
        r += 1.0;
    }
    Cell { col, row }
}

/// The cell a corner's resize keeps still: the widget's opposite corner cell. `corner` is `nw`, `ne`, `sw` or `se`.
pub fn corner_anchor(w: &Value, corner: &str) -> Cell {
    let b = boxed(w);
    Cell {
        col: if corner.contains('w') {
            b.col + b.cs - 1.0
        } else {
            b.col
        },
        row: if corner.contains('n') {
            b.row + b.rs - 1.0
        } else {
            b.row
        },
    }
}

/// The box spanning two cells, whichever way round they are: `[col, row, colSpan, rowSpan]`.
pub fn box_between(a: Cell, b: Cell) -> [f64; 4] {
    [
        a.col.min(b.col),
        a.row.min(b.row),
        (a.col - b.col).abs() + 1.0,
        (a.row - b.row).abs() + 1.0,
    ]
}

/// The ids in paint order: a group first, under what it frames; every other keeps its place.
pub fn groups_first(widgets: &Obj) -> Vec<String> {
    let mut ids: Vec<(&String, bool)> = widgets.iter().map(|(id, w)| (id, kind(w) == "group")).collect();
    ids.sort_by_key(|(_, g)| !*g);
    ids.into_iter().map(|(id, _)| id.clone()).collect()
}

/// A stable, unused id across the whole panel: `w1`, `w2`…
pub fn next_id(taken: &[String]) -> String {
    (1..)
        .map(|i| format!("w{i}"))
        .find(|id| !taken.contains(id))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group() -> Value {
        json!({ "type": "group", "col": 2, "row": 1, "colSpan": 4, "rowSpan": 2 })
    }

    fn knob(col: u32, row: u32) -> Value {
        json!({ "type": "knob", "col": col, "row": row, "colSpan": 2, "rowSpan": 2 })
    }

    #[test]
    fn a_group_contains_what_lies_wholly_inside_and_not_what_straddles_its_edge() {
        assert!(inside(&knob(2, 1), &group()));
        assert!(inside(&knob(4, 1), &group()));
        assert!(!inside(&knob(5, 1), &group()));
        assert!(overlaps(&knob(5, 1), &group()));
    }

    #[test]
    fn a_group_clashes_with_a_straddler_and_another_group_never_with_what_it_frames() {
        assert!(!clash(&group(), &knob(2, 1)));
        assert!(!clash(&knob(2, 1), &group()));
        assert!(clash(&group(), &knob(5, 1)));
        assert!(clash(&group(), &with(&group(), "col", json!(3))));
        assert!(clash(&knob(2, 1), &knob(3, 1)));
        assert!(!clash(&group(), &knob(8, 1)));
    }

    #[test]
    fn a_group_frames_the_widgets_inside_it_and_names_the_group_a_widget_sits_in() {
        let widgets: Obj = json!({ "dry": knob(2, 1), "wet": knob(4, 1), "out": knob(6, 1), "g": group() })
            .as_object()
            .cloned()
            .unwrap_or_default();
        assert_eq!(framed_by(&widgets, &group()), vec!["dry", "wet"]);
        assert_eq!(group_of(&widgets, "dry").as_deref(), Some("g"));
        assert_eq!(group_of(&widgets, "out"), None);
        assert_eq!(group_of(&widgets, "g"), None);
    }

    #[test]
    fn the_free_cell_the_corner_and_the_paint_order() {
        let widgets: Obj = json!({ "a": knob(0, 0), "g": group(), "b": knob(2, 0) })
            .as_object()
            .cloned()
            .unwrap_or_default();
        assert_eq!(first_free(&widgets, 0.0, 0.0, 6.0, 2.0), Cell { col: 4.0, row: 0.0 });
        assert_eq!(first_free(&Obj::new(), 1.0, 1.0, 0.0, 0.0), Cell { col: 1.0, row: 1.0 });
        assert_eq!(corner_anchor(&knob(1, 1), "nw"), Cell { col: 2.0, row: 2.0 });
        assert_eq!(corner_anchor(&knob(1, 1), "se"), Cell { col: 1.0, row: 1.0 });
        assert_eq!(
            box_between(Cell { col: 3.0, row: 1.0 }, Cell { col: 1.0, row: 2.0 }),
            [1.0, 1.0, 3.0, 2.0]
        );
        assert_eq!(groups_first(&widgets), vec!["g", "a", "b"]);
        assert_eq!(next_id(&["w1".to_owned(), "w3".to_owned()]), "w2");
    }

    #[test]
    fn a_wide_panel_with_no_vertical_twin_is_named_and_reflow_packs_four_columns() {
        let ui = json!({ "cols": 6, "rows": 2, "widgets": { "a": knob(0, 0), "b": knob(2, 0), "c": knob(4, 0) } });
        assert_eq!(
            twin_issues(&ui),
            vec!["the panel is 6 columns wide and has no `vertical` layout: one of at most 4 columns is needed"]
        );
        let narrow = reflow_narrow(&ui);
        assert_eq!(narrow["cols"], 4);
        assert_eq!(narrow["rows"], 4);
        assert_eq!(narrow["widgets"]["c"]["row"], 2);
        let tabbed = json!({ "cols": 4, "rows": 2, "widgets": {}, "tabs": [{ "name": "A", "widgets": {} }, { "name": "B", "widgets": {} }],
            "vertical": { "cols": 4, "rows": 2, "widgets": {}, "tabs": [{ "name": "x", "widgets": {} }, { "name": "y", "widgets": {} }] } });
        assert_eq!(align_tabs(&tabbed)["vertical"]["tabs"][1]["name"], "B");
    }
}
