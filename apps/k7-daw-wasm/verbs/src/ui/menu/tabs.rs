// tabs.rs — the right-click menu's tabs: one per kind of thing its rows act on, read off the command's first word.
// A reader would get wrong that a tab is chosen: it exists only when a row falls in it, and `ui` sits last since the clicked thing comes first.

use serde::{Deserialize, Serialize};

/// What the tabs read of a row: its verb, and whether it asks before it runs.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TabRow {
    pub verb: String,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub confirm: Option<bool>,
}

/// One tab and the rows (by position) that fall in it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TabGroup {
    pub tab: String,
    pub indices: Vec<usize>,
}

/// Heads that are another head's, said once: the range verbs act on events too.
fn same_as(head: &str) -> &str {
    if head == "events" { "event" } else { head }
}

/// The tab a row belongs to: its command's first word.
#[must_use]
pub fn menu_tab(verb: &str) -> String {
    same_as(verb.split(' ').next().unwrap_or("")).to_owned()
}

/// The rows grouped into tabs, in the order the menu offers them: the shell's own `ui` tab last.
#[must_use]
pub fn menu_tabs(rows: &[TabRow]) -> Vec<TabGroup> {
    let mut tabs: Vec<TabGroup> = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        let tab = menu_tab(&r.verb);
        match tabs.iter_mut().find(|t| t.tab == tab) {
            Some(t) => t.indices.push(i),
            None => tabs.push(TabGroup { tab, indices: vec![i] }),
        }
    }
    let (ui, rest): (Vec<_>, Vec<_>) = tabs.into_iter().partition(|t| t.tab == "ui");
    rest.into_iter().chain(ui).collect()
}

/// The last words of a verb that take something away: with the catalogue's `confirm`, what the menu draws in red, apart and last.
const TAKES_AWAY: [&str; 3] = ["delete", "remove", "unlink"];

/// Whether a row destroys rather than changes: it asks first, or its verb ends by removing.
#[must_use]
pub fn is_destructive(row: &TabRow) -> bool {
    row.confirm == Some(true) || row.verb.split(' ').next_back().is_some_and(|w| TAKES_AWAY.contains(&w))
}

/// A tab's rows split in two, each half in its own order: the menu draws the destructive half last, after a separator.
#[must_use]
pub fn danger_split(rows: &[TabRow]) -> (Vec<usize>, Vec<usize>) {
    (0..rows.len()).partition(|&i| !is_destructive(&rows[i]))
}

/// Where Up, Down, Home and End move the focus among `n` rows from `at` (-1 when none has it), or none for another key.
#[must_use]
pub fn move_focus(key: &str, at: i64, n: i64) -> Option<i64> {
    if n == 0 {
        return None;
    }
    match key {
        "ArrowDown" => Some(if at < 0 { 0 } else { (at + 1) % n }),
        "ArrowUp" => Some(if at < 0 { n - 1 } else { (at - 1 + n) % n }),
        "Home" => Some(0),
        "End" => Some(n - 1),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(verb: &str, confirm: bool) -> TabRow {
        TabRow {
            verb: verb.into(),
            confirm: confirm.then_some(true),
        }
    }

    #[test]
    fn a_row_is_in_the_tab_of_its_first_word_and_events_are_the_events() {
        assert_eq!(menu_tab("track mute"), "track");
        assert_eq!(menu_tab("events move"), "event");
        assert_eq!(menu_tab("ui file"), "ui");
    }

    #[test]
    fn tabs_keep_the_order_rows_first_appear_in_and_the_shell_last() {
        let rows = [
            row("ui file", false),
            row("track mute", false),
            row("event split", false),
            row("events move", false),
            row("track solo", false),
        ];
        let tabs = menu_tabs(&rows);
        assert_eq!(
            tabs,
            vec![
                TabGroup {
                    tab: "track".into(),
                    indices: vec![1, 4]
                },
                TabGroup {
                    tab: "event".into(),
                    indices: vec![2, 3]
                },
                TabGroup {
                    tab: "ui".into(),
                    indices: vec![0]
                },
            ]
        );
        assert!(menu_tabs(&[]).is_empty());
    }

    #[test]
    fn what_takes_away_or_asks_first_goes_last() {
        let rows = [
            row("track delete", false),
            row("track mute", false),
            row("track gain", true),
            row("graph unlink", false),
        ];
        let (safe, danger) = danger_split(&rows);
        assert_eq!(safe, vec![1]);
        assert_eq!(danger, vec![0, 2, 3]);
        assert!(!is_destructive(&row("project marker new", false)));
    }

    #[test]
    fn the_arrows_wrap_and_home_and_end_go_to_the_ends() {
        assert_eq!(move_focus("ArrowDown", -1, 3), Some(0));
        assert_eq!(move_focus("ArrowDown", 2, 3), Some(0));
        assert_eq!(move_focus("ArrowUp", -1, 3), Some(2));
        assert_eq!(move_focus("ArrowUp", 0, 3), Some(2));
        assert_eq!(move_focus("Home", 1, 3), Some(0));
        assert_eq!(move_focus("End", 0, 3), Some(2));
        assert_eq!(move_focus("a", 0, 3), None);
        assert_eq!(move_focus("ArrowDown", 0, 0), None);
    }
}
