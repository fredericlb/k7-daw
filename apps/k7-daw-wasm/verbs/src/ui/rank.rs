// rank.rs — the command bar's rankings: a catalogue against what was typed, a field's options, the bar when empty.
// A reader would get wrong that history invents a match: a recent command only reorders what already matched.

use super::fuzzy::fuzzy_score;
use super::order::locale_cmp;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashSet;

/// How many commands are remembered: the bonus decays across this many places.
pub const MAX_RECENT: usize = 12;
/// What a command run lately is worth: a nudge of about one letter.
const RECENT_BONUS: f64 = 4.0;
/// How many rows a list shows.
pub const DEFAULT_LIMIT: usize = 30;
/// How many recent commands the empty bar offers: a glance, not the whole history.
pub const EMPTY_RECENT_MAX: usize = 5;
/// The first verbs a newcomer needs: make a lane, find a file, see the mix, read the help.
const STARTERS: [&str; 4] = ["track new", "ui file", "ui mixer", "ui help"];

fn default_limit() -> usize {
    DEFAULT_LIMIT
}

/// What ranking reads of a command: the name and the sentence in the reader's language.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Row {
    pub canonical: String,
    #[serde(default)]
    pub readable: String,
}

/// `rank_commands`' arguments.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RankCommands {
    pub commands: Vec<Row>,
    pub query: String,
    #[serde(default)]
    pub recent: Vec<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn recency_bonus(canonical: &str, recent: &[String]) -> f64 {
    recent
        .iter()
        .position(|r| r == canonical)
        .map_or(0.0, |i| RECENT_BONUS * (1.0 - i as f64 / MAX_RECENT as f64))
}

fn by_score(a: f64, b: f64) -> Ordering {
    // `b.score - a.score` first, as the page's sort did.
    let d = b - a;
    if d > 0.0 {
        Ordering::Greater
    } else if d < 0.0 {
        Ordering::Less
    } else {
        Ordering::Equal
    }
}

/// The indices of the catalogue's best matches, best first: matched on the sentence and on the name, history a nudge.
#[must_use]
pub fn rank_commands(a: &RankCommands) -> Vec<usize> {
    let q = js_trim(&a.query);
    if q.is_empty() {
        return (0..a.commands.len().min(a.limit)).collect();
    }
    let mut scored: Vec<(usize, f64)> = a
        .commands
        .iter()
        .enumerate()
        .map(|(i, c)| {
            // The name scores slightly higher on a tie: someone typing "mute" means the command.
            let s = fuzzy_score(q, &c.readable).max(fuzzy_score(q, &c.canonical) + 0.5);
            (i, s)
        })
        // Filtered BEFORE the bonus: history reorders what matched and never invents a match.
        .filter(|(_, s)| *s >= 0.0)
        .map(|(i, s)| (i, s + recency_bonus(&a.commands[i].canonical, &a.recent)))
        .collect();
    scored.sort_by(|(i, x), (j, y)| {
        by_score(*x, *y).then_with(|| locale_cmp(&a.commands[*i].canonical, &a.commands[*j].canonical))
    });
    scored.into_iter().take(a.limit).map(|(i, _)| i).collect()
}

fn js_trim(s: &str) -> &str {
    song_edit::js_text::js_trim(s)
}

/// One value a field offers: what is inserted, and what the list shows when that is unreadable.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ArgOption {
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub label: Option<String>,
}

/// `rank_options`' arguments.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RankOptions {
    pub options: Vec<ArgOption>,
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

/// The indices of a field's options ranked on the value OR the label: a node's uuid is found by its module.
#[must_use]
pub fn rank_options(a: &RankOptions) -> Vec<usize> {
    let q = js_trim(&a.query);
    if q.is_empty() {
        return (0..a.options.len().min(a.limit)).collect();
    }
    let shown = |o: &ArgOption| o.label.clone().unwrap_or_else(|| o.value.clone());
    let mut scored: Vec<(usize, f64)> = a
        .options
        .iter()
        .enumerate()
        .map(|(i, o)| {
            let label = o.label.as_deref().map_or(-1.0, |l| fuzzy_score(q, l));
            (i, fuzzy_score(q, &o.value).max(label))
        })
        .filter(|(_, s)| *s >= 0.0)
        .collect();
    scored.sort_by(|(i, x), (j, y)| {
        by_score(*x, *y).then_with(|| locale_cmp(&shown(&a.options[*i]), &shown(&a.options[*j])))
    });
    scored.into_iter().take(a.limit).map(|(i, _)| i).collect()
}

/// One completed line as `/complete` answers it.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Completion {
    pub line: String,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub label: Option<String>,
}

/// `/complete`'s whole lines as one field's options: the prefix sliced back off, deduped by value, labels kept.
#[must_use]
pub fn options_from_completions(full: &[Completion], prefix: &str) -> Vec<ArgOption> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for c in full {
        // Sliced by the prefix and not indexed by token, which keeps a multi-word command and a spaced value whole.
        let value = c.line.strip_prefix(prefix).unwrap_or("");
        if value.is_empty() || !seen.insert(value.to_owned()) {
            continue;
        }
        let label = c.label.clone().filter(|l| !l.is_empty());
        out.push(ArgOption {
            value: value.to_owned(),
            label,
        });
    }
    out
}

/// `empty_bar_rows`' arguments: the catalogue's names, the recents, and the starters when the page names its own.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct EmptyBar {
    pub commands: Vec<String>,
    #[serde(default)]
    pub recent: Vec<String>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub starters: Option<Vec<String>>,
}

/// One row of the empty bar: which command, and whether it is a recent one rather than a starter.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct EmptyRow {
    pub index: usize,
    pub recent: bool,
}

/// What the bar lists with the keyboard and nothing typed: the recents the catalogue still holds, then the starters it holds.
#[must_use]
pub fn empty_bar_rows(a: &EmptyBar) -> Vec<EmptyRow> {
    // The last of two equal names wins, as a `Map` built from the list does.
    let by_name: std::collections::HashMap<&str, usize> =
        a.commands.iter().enumerate().map(|(i, c)| (c.as_str(), i)).collect();
    let starters: Vec<&str> = a
        .starters
        .as_ref()
        .map_or_else(|| STARTERS.to_vec(), |s| s.iter().map(String::as_str).collect());
    let mut rows = Vec::new();
    let mut seen = HashSet::new();
    for name in &a.recent {
        if rows.len() >= EMPTY_RECENT_MAX {
            break;
        }
        let Some(&index) = by_name.get(name.as_str()) else {
            continue;
        };
        if seen.insert(name.as_str()) {
            rows.push(EmptyRow { index, recent: true });
        }
    }
    for name in starters {
        let Some(&index) = by_name.get(name) else { continue };
        if seen.insert(name) {
            rows.push(EmptyRow { index, recent: false });
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(items: &[(&str, &str)]) -> Vec<Row> {
        items
            .iter()
            .map(|(c, r)| Row {
                canonical: (*c).to_owned(),
                readable: (*r).to_owned(),
            })
            .collect()
    }

    fn catalogue() -> Vec<Row> {
        rows(&[
            ("track mute", "Mute or unmute a track"),
            ("transport play", "Start playback"),
            ("project marker new", "Add a marker to the timeline"),
            ("track color", "Recolour a track"),
        ])
    }

    fn rank(q: &str, recent: &[&str]) -> Vec<String> {
        let c = catalogue();
        let a = RankCommands {
            commands: c.clone(),
            query: q.to_owned(),
            recent: recent.iter().map(|s| (*s).to_owned()).collect(),
            limit: DEFAULT_LIMIT,
        };
        rank_commands(&a).into_iter().map(|i| c[i].canonical.clone()).collect()
    }

    #[test]
    fn it_matches_the_sentence_and_the_name_and_is_fuzzy() {
        assert!(rank("unmute", &[]).contains(&"track mute".to_owned()));
        assert!(rank("playback", &[]).contains(&"transport play".to_owned()));
        assert!(rank("tmut", &[]).contains(&"track mute".to_owned()));
        assert_eq!(rank("track color", &[])[0], "track color");
        assert!(rank("zzzz", &[]).is_empty());
    }

    #[test]
    fn it_matches_the_french_sentence_and_the_name_which_is_never_translated() {
        let c = rows(&[
            ("track mute", "Couper ou r\u{e9}tablir le son d'une piste"),
            ("project marker new", "Ajouter un rep\u{e8}re sur la timeline"),
        ]);
        let ask = |q: &str| {
            let a = RankCommands {
                commands: c.clone(),
                query: q.into(),
                recent: vec![],
                limit: DEFAULT_LIMIT,
            };
            rank_commands(&a)
                .into_iter()
                .map(|i| c[i].canonical.clone())
                .collect::<Vec<_>>()
        };
        assert!(ask("couper").contains(&"track mute".to_owned()));
        assert!(ask("rep\u{e8}re").contains(&"project marker new".to_owned()));
        assert!(ask("marker").contains(&"project marker new".to_owned()));
    }

    #[test]
    fn nothing_typed_keeps_the_catalogue_order_up_to_the_limit() {
        assert_eq!(rank("", &[])[0], "track mute");
        let many = Row {
            canonical: "x".into(),
            readable: String::new(),
        };
        let a = RankCommands {
            commands: vec![many; 80],
            query: "  ".into(),
            recent: vec![],
            limit: DEFAULT_LIMIT,
        };
        assert_eq!(rank_commands(&a).len(), 30);
    }

    #[test]
    fn what_you_used_lately_comes_first_but_only_among_matches() {
        assert_ne!(rank("track", &[])[0], "track color");
        assert_eq!(rank("track", &["track color"])[0], "track color");
        let mut used: Vec<&str> = vec!["project marker new"];
        used.extend(std::iter::repeat_n("x", 8));
        used.push("track color");
        assert_eq!(rank("track", &used)[0], "track color");
        assert_eq!(rank("a", &["track color", "project marker new"])[0], "track color");
        // A nudge, not an override.
        assert_eq!(rank("Mute or unmute", &["transport play"])[0], "track mute");
        assert!(rank("zzzz", &["track mute"]).is_empty());
        assert!(!rank("track", &["gone forever"]).is_empty());
    }

    fn opt(v: &str, l: Option<&str>) -> ArgOption {
        ArgOption {
            value: v.into(),
            label: l.map(Into::into),
        }
    }

    fn ranked(options: &[ArgOption], q: &str) -> Vec<ArgOption> {
        let a = RankOptions {
            options: options.to_vec(),
            query: q.into(),
            limit: DEFAULT_LIMIT,
        };
        rank_options(&a).into_iter().map(|i| options[i].clone()).collect()
    }

    #[test]
    fn a_value_is_found_from_the_middle_and_the_closer_match_is_first() {
        let tracks: Vec<ArgOption> = ["ge.rythm", "ge.lead", "dr.kick", "vx.lead"]
            .iter()
            .map(|v| opt(v, None))
            .collect();
        assert_eq!(ranked(&tracks, "rythm"), vec![tracks[0].clone()]);
        assert_eq!(ranked(&tracks, "lead")[0], tracks[1]);
        assert_eq!(ranked(&tracks, ""), tracks);
        assert!(ranked(&tracks, "qqq").is_empty());
    }

    #[test]
    fn a_node_named_by_a_uuid_is_found_by_its_label_or_its_value() {
        let wah = "0b7c2f4e-1d2a-4c55-9a10-3e8f6a2b9d01";
        let nodes = vec![
            opt(wah, Some("@k7-core/auto-wah \u{b7} fx \u{b7} step 2")),
            opt(
                "5f1e9a3c-7b42-4e0d-8c6a-2d9b1f4e7a30",
                Some("@k7-core/comp \u{b7} fx \u{b7} step 1"),
            ),
            opt("gtr", None),
        ];
        assert_eq!(ranked(&nodes, "auto-wah"), vec![nodes[0].clone()]);
        assert_eq!(ranked(&nodes, wah)[0], nodes[0]);
        assert_eq!(ranked(&nodes, "gtr"), vec![nodes[2].clone()]);
    }

    #[test]
    fn completions_lose_the_sent_prefix_keep_labels_and_drop_duplicates() {
        let wah = "0b7c2f4e-1d2a-4c55-9a10-3e8f6a2b9d01";
        let line = |l: &str, label: Option<&str>| Completion {
            line: l.into(),
            label: label.map(Into::into),
        };
        let got = options_from_completions(
            &[
                line(&format!("node bypass {wah}"), Some("@k7-core/auto-wah")),
                line("node bypass gtr", None),
                line("node bypass gtr", None),
                line("node other", None),
            ],
            "node bypass ",
        );
        assert_eq!(got, vec![opt(wah, Some("@k7-core/auto-wah")), opt("gtr", None)]);
    }

    fn bar(commands: &[&str], recent: &[&str], starters: Option<&[&str]>) -> Vec<String> {
        let a = EmptyBar {
            commands: commands.iter().map(|s| (*s).to_owned()).collect(),
            recent: recent.iter().map(|s| (*s).to_owned()).collect(),
            starters: starters.map(|s| s.iter().map(|x| (*x).to_owned()).collect()),
        };
        empty_bar_rows(&a)
            .iter()
            .map(|r| format!("{}:{}", if r.recent { "R" } else { "S" }, commands[r.index]))
            .collect()
    }

    const CATALOGUE: [&str; 8] = [
        "track new",
        "ui file",
        "ui help",
        "track mute",
        "event split",
        "ui zoom",
        "transport loop",
        "node new",
    ];

    #[test]
    fn the_empty_bar_lists_starters_the_catalogue_holds() {
        assert_eq!(
            bar(&CATALOGUE, &[], Some(&["track new", "ui mixer", "ui help"])),
            ["S:track new", "S:ui help"]
        );
        assert_eq!(bar(&CATALOGUE, &[], None), ["S:track new", "S:ui file", "S:ui help"]);
    }

    #[test]
    fn recents_come_first_and_never_repeat_as_a_starter() {
        assert_eq!(
            bar(&CATALOGUE, &["ui help", "track mute"], Some(&["ui help"])),
            ["R:ui help", "R:track mute"]
        );
    }

    #[test]
    fn the_recents_are_capped_and_gone_names_dropped() {
        let recent = [
            "gone verb",
            "track mute",
            "event split",
            "ui zoom",
            "transport loop",
            "node new",
            "ui file",
        ];
        let rows = bar(&CATALOGUE, &recent, Some(&[]));
        assert_eq!(rows.len(), EMPTY_RECENT_MAX);
        assert_eq!(rows[0], "R:track mute");
    }
}
