// check.rs — a library panel against the two-layout rules, the AI brief's structural rules, and what it offered before.
// A reader would get wrong that a control is "lost" by moving: a migrated panel must keep every bind the committed one had, wherever it went.

use super::ai_check::{at_control_issues, eq_band_issues, lone_group_issues};
use super::geometry::{Pairs, grid_issues, pages, twin_issues};
use super::loose::{bind, entries, kind, num, obj};
use super::write::from_k7;
use crate::files::validate_file;
use serde_json::Value;

/// Every control a panel offers, all its pages: binds, frames and labels aside, in the order they are met.
fn offers(ui: &Value) -> Vec<String> {
    let tabs = ui.get("tabs").and_then(Value::as_array).into_iter().flatten();
    let mut out: Vec<String> = Vec::new();
    for m in std::iter::once(obj(ui, "widgets")).chain(tabs.map(|t| obj(t, "widgets"))) {
        for (_, w) in entries(m) {
            if let Some(b) = bind(w).filter(|_| !matches!(kind(w), "group" | "label"))
                && !out.iter().any(|o| o == b)
            {
                out.push(b.to_owned());
            }
        }
    }
    out
}

fn read(text: &str) -> Result<Value, Vec<String>> {
    let raw = k7x::edit::parse(text).map_err(|e| vec![format!("unreadable: {e}")])?;
    let verdict = validate_file("modules/audio/x/ui.k7x", text);
    if !verdict.ok {
        return Err(verdict
            .issues
            .iter()
            .map(|i| format!("{}: {}", if i.path.is_empty() { "(root)" } else { &i.path }, i.message))
            .collect());
    }
    Ok(from_k7(&raw))
}

/// Every issue a library panel has: its text, the text git holds for it, and the screens its widgets.js draws.
pub fn check_layout(text: &str, committed: Option<&str>, drawn: &[String]) -> Vec<String> {
    let ui = match read(text) {
        Ok(ui) => ui,
        Err(issues) => return issues,
    };
    let mut issues = twin_issues(&ui);
    for (place, ws) in pages(&ui) {
        issues.extend(grid_issues(num(&ui, "cols"), num(&ui, "rows"), &place, &ws));
    }
    let layouts: Vec<&Value> = std::iter::once(&ui)
        .chain(ui.get("vertical").filter(|v| v.is_object()))
        .collect();
    // The brief's structural rules, in BOTH shapes.
    for layout in &layouts {
        for (_, ws) in pages(layout) {
            issues.extend(at_control_issues(&ws));
            issues.extend(eq_band_issues(&ws));
        }
    }
    let top: Pairs = entries(obj(&ui, "widgets"));
    issues.extend(lone_group_issues(&top));
    for t in ui.get("tabs").and_then(Value::as_array).into_iter().flatten() {
        issues.extend(lone_group_issues(&entries(obj(t, "widgets"))));
    }
    if let Some(before) = committed.and_then(|c| read(c).ok()) {
        let now = offers(&ui);
        // An `@` control is one the brief forbids, so dropping it is the fix rather than a loss.
        let lost: Vec<String> = offers(&before)
            .into_iter()
            .filter(|b| !now.contains(b) && !b.starts_with('@'))
            .map(|b| format!("`{b}`"))
            .collect();
        if !lost.is_empty() {
            issues.push(format!(
                "controls the committed panel had are gone: {}",
                lost.join(", ")
            ));
        }
    }
    // Every screen needs its draw function in the panel's widgets.js.
    for layout in &layouts {
        for (_, ws) in pages(layout) {
            for (id, w) in ws {
                if kind(w) == "lcdscreen" && !drawn.contains(id) {
                    issues.push(format!("screen `{id}` has no draw function in widgets.js"));
                }
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

#[cfg(test)]
mod tests {
    use super::check_layout;

    const GOOD: &str = "cols(4)\nrows(2)\nwidgets({ a: { type: \"knob\", col: 0, row: 0, bind: \"A\" } })\n";

    #[test]
    fn a_good_panel_passes_and_a_lost_control_is_named() {
        assert!(check_layout(GOOD, Some(GOOD), &[]).is_empty());
        let moved = "cols(4)\nrows(2)\nwidgets({ b: { type: \"knob\", col: 0, row: 0, bind: \"B\" } })\n";
        assert_eq!(
            check_layout(moved, Some(GOOD), &[]),
            vec!["controls the committed panel had are gone: `A`".to_owned()]
        );
    }

    #[test]
    fn a_screen_without_its_function_and_a_wide_panel_are_named() {
        let wide = "cols(6)\nrows(2)\nwidgets({ s: { type: \"lcdscreen\", col: 0, row: 0 } })\n";
        let got = check_layout(wide, None, &[]);
        assert!(
            got.iter().any(|i| i.contains("6 columns wide and has no `vertical`")),
            "{got:?}"
        );
        assert!(
            got.iter().any(|i| i == "screen `s` has no draw function in widgets.js"),
            "{got:?}"
        );
        assert!(
            check_layout(wide, None, &["s".to_owned()])
                .iter()
                .all(|i| !i.contains("draw function"))
        );
    }

    #[test]
    fn a_text_that_does_not_read_is_unreadable() {
        assert!(check_layout("cols(", None, &[])[0].starts_with("unreadable: "));
    }
}
