// palette.rs — the palette's catalogue (the backend's commands plus the shell's own) and each argument's vocabulary.
// A reader would get wrong who lists the shell's lists: the page hands in what only it holds (its files, its languages), and Rust says which list a field takes.

use super::export::{MIX_FORMATS, TIMELINE_FORMATS, VIDEO_FORMATS};
use crate::catalogue::Readable;
use crate::palette::{CommandArg, CommandInfo};
use serde::{Deserialize, Serialize};
use song_edit::sugar::strip_doc_ext;

/// One row of the palette: the command, the sentence it is listed by in the reader's language, and its params.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PaletteRow {
    pub canonical: String,
    pub label: String,
    pub names: Vec<String>,
    pub readable: String,
    /// The whole table too, for the one screen that offers a field per language.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub readable_by: Option<Readable>,
    pub args: Vec<CommandArg>,
    /// Where the catalogue offers it on a right-click; the shell's own commands say nothing, so every menu their arguments fit may.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<crate::ui::wire::MenuEntity>>", optional))]
    pub menu: Option<Vec<String>>,
}

/// A command the shell answers itself: its name, its sentences and its params.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LocalCommand {
    pub name: String,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub readable: Option<Readable>,
    #[serde(default)]
    pub args: Vec<CommandArg>,
}

fn row(
    canonical: &str,
    args: Vec<CommandArg>,
    readable: Option<&Readable>,
    menu: Option<Vec<String>>,
    fr: bool,
) -> PaletteRow {
    let sentence = readable
        .map(|r| if fr { &r.fr } else { &r.en })
        .filter(|s| !s.is_empty());
    PaletteRow {
        canonical: canonical.to_owned(),
        label: canonical.to_owned(),
        names: vec![canonical.to_owned()],
        readable: sentence.cloned().unwrap_or_else(|| canonical.to_owned()),
        readable_by: readable.cloned(),
        args,
        menu,
    }
}

/// The palette's full catalogue: the backend's commands plus the shell's own, each description resolved once by the locale.
#[must_use]
pub fn build_commands(backend: &[CommandInfo], locals: &[LocalCommand], locale: &str) -> Vec<PaletteRow> {
    let fr = locale.starts_with("fr");
    let mut out: Vec<PaletteRow> = backend
        .iter()
        .map(|c| row(&c.command, c.args.clone(), c.readable.as_ref(), c.menu.clone(), fr))
        .collect();
    out.extend(
        locals
            .iter()
            .filter(|l| !backend.iter().any(|b| b.command == l.name))
            .map(|l| row(&l.name, l.args.clone(), l.readable.as_ref(), None, fr)),
    );
    out
}

/// Which command a canonical line runs: the longest name that prefixes it, exactly as the dispatcher picks it.
#[must_use]
pub fn command_name_of(line: &str, canonicals: &[String]) -> Option<String> {
    let s = song_edit::js_text::js_trim(line);
    canonicals
        .iter()
        .filter(|c| s == c.as_str() || s.strip_prefix(c.as_str()).is_some_and(|r| r.starts_with(' ')))
        .fold(None::<&String>, |best, c| match best {
            Some(b) if b.len() >= c.len() => Some(b),
            _ => Some(c),
        })
        .cloned()
}

/// The commands whose `path` or `folder` argument names any tree entry rather than an openable file.
const ENTRY_COMMANDS: [&str; 9] = [
    "fs file new",
    "fs folder new",
    "fs file rename",
    "fs file move",
    "fs file delete",
    "fs file upload",
    "ui pin",
    "ui copy path",
    "project import",
];

/// What only the page can list: the lists a shell command's field takes.
#[derive(Clone, Debug, Default, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ShellLists {
    /// What the editor can open: `ui file`'s vocabulary.
    #[serde(default)]
    pub files: Vec<String>,
    /// Every path the tree draws, folders included.
    #[serde(default)]
    pub entries: Vec<String>,
    /// The surfaces this project declares, by id.
    #[serde(default)]
    pub surfaces: Vec<String>,
    /// The languages the shell is compiled for.
    #[serde(default)]
    pub locales: Vec<String>,
    /// The project's timelines but Main, by id.
    #[serde(default)]
    pub timelines: Vec<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub wave_modes: Vec<String>,
    #[serde(default)]
    pub roll_modes: Vec<String>,
}

/// Where a field's vocabulary comes from: known here, or asked of the backend's completion with this line.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "source", rename_all = "lowercase")]
pub enum ArgSource {
    Values {
        values: Vec<String>,
    },
    /// The line must end with a space, even for an empty partial: that is how `complete` tells an argument from a command name.
    Ask {
        line: String,
        prefix: String,
    },
}

fn strings<const N: usize>(v: [&str; N]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

/// `(^|/)<tail>.k7x` at the end of a path, the case folded and the dot any character: the page's pattern for a document by folder.
fn doc_in(path: &str, folder: Option<&str>) -> Option<String> {
    let lower = path.to_lowercase();
    let body = lower.strip_suffix("k7x")?;
    let mut chars = body.chars();
    let last = chars.next_back()?;
    if matches!(last, '\n' | '\r' | '\u{2028}' | '\u{2029}') {
        return None;
    }
    let head = chars.as_str();
    match folder {
        None => (head == "module" || head.ends_with("/module")).then(|| head.to_owned()),
        Some(f) => {
            let at = if head.starts_with(&format!("{f}/")) {
                Some(0)
            } else {
                head.find(&format!("/{f}/")).map(|i| i + 1)
            }?;
            let name = &head[at + f.len() + 1..];
            (!name.is_empty() && !name.contains('/')).then(|| name.to_owned())
        }
    }
}

/// A field's whole vocabulary, given the params before it: fetched whole and ranked in the browser, so the match is fuzzy and costs one round trip.
#[must_use]
pub fn arg_source(canonical: &str, prior: &[String], lists: &ShellLists, entry_arg_is_file: bool) -> ArgSource {
    let canonical = song_edit::js_text::js_trim(canonical);
    let first = prior.is_empty();
    let values = |v: Vec<String>| ArgSource::Values { values: v };
    // The shell's other fixed sets, each the shell's own vocabulary: asking the backend would be a round trip for a literal.
    match (canonical, first) {
        ("ui panel", true) => return values(strings(["event", "graph", "mix", "logs"])),
        ("ui tool", true) => return values(lists.tools.clone()),
        ("ui wave", true) => return values(lists.wave_modes.clone()),
        ("ui roll", true) => return values(lists.roll_modes.clone()),
        ("ui language", true) => return values(lists.locales.clone()),
        _ => {}
    }
    if canonical == "project export" {
        let own = |a: &[&str]| a.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        return values(match (prior.len(), prior.first().map(String::as_str)) {
            (0, _) => strings(["audio", "video", "timeline"]),
            // The format depends on the half: a video has a container and a third word, the cut an interchange format.
            (1, Some("video")) => own(&VIDEO_FORMATS),
            (1, Some("timeline")) => own(&TIMELINE_FORMATS),
            (1, _) => own(&MIX_FORMATS),
            (2, Some("video")) => own(&[MIX_FORMATS[0], MIX_FORMATS[1], "none"]),
            // The audio of one timeline, Main's being the song's.
            (2, Some("audio")) => std::iter::once("main".to_owned())
                .chain(lists.timelines.iter().cloned())
                .collect(),
            _ => vec![],
        });
    }
    if canonical == "fs file upload" && !first {
        return values(strings(["files", "folder"]));
    }
    // ...and the ones that are the project's, but that only this app can answer.
    if first {
        match canonical {
            "ui external" => return values(lists.surfaces.clone()),
            "ui file" => return values(lists.files.clone()),
            "ui builder" => {
                return values(
                    lists
                        .files
                        .iter()
                        .filter(|f| doc_in(f, None).is_some())
                        .cloned()
                        .collect(),
                );
            }
            // A curve and a clip are named by their id, their file's basename.
            "ui curve" | "ui review" => {
                let folder = if canonical == "ui curve" { "curves" } else { "clips" };
                return values(
                    lists
                        .files
                        .iter()
                        .filter(|f| doc_in(f, Some(folder)).is_some())
                        .map(|f| strip_doc_ext(f.rsplit('/').next().unwrap_or(f)).to_owned())
                        .collect(),
                );
            }
            _ => {}
        }
    }
    if ENTRY_COMMANDS.contains(&canonical) {
        // Which of this command's arguments names a path is the shell's declared kind, which is what stops a name field being answered with the tree.
        return values(if entry_arg_is_file {
            lists.entries.clone()
        } else {
            vec![]
        });
    }
    let head = std::iter::once(canonical)
        .chain(prior.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ");
    ArgSource::Ask {
        line: format!("{head} "),
        prefix: format!("{head} "),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(command: &str, en: &str, fr: &str) -> CommandInfo {
        CommandInfo {
            command: command.into(),
            help: None,
            readable: Some(Readable {
                en: en.into(),
                fr: fr.into(),
            }),
            args: vec![],
            menu: None,
            confirm: None,
            quiet: None,
        }
    }

    fn local(name: &str) -> LocalCommand {
        LocalCommand {
            name: name.into(),
            readable: Some(Readable {
                en: "Open help".into(),
                fr: "Aide".into(),
            }),
            args: vec![],
        }
    }

    #[test]
    fn a_row_carries_the_sentence_in_the_readers_language_and_the_whole_table() {
        let b = [info("track mute", "Mute a track", "Couper une piste")];
        let en = build_commands(&b, &[], "en");
        let fr = build_commands(&b, &[], "fr-CA");
        assert_eq!(en[0].readable, "Mute a track");
        assert_eq!(fr[0].readable, "Couper une piste");
        assert_eq!(en[0].readable_by, fr[0].readable_by);
        assert_eq!(en[0].names, ["track mute"]);
    }

    #[test]
    fn a_command_with_no_sentence_falls_back_to_its_name_and_has_no_table() {
        let mut bare = info("track solo", "", "");
        bare.readable = None;
        let rows = build_commands(&[bare], &[], "en");
        assert_eq!(rows[0].readable, "track solo");
        assert_eq!(rows[0].readable_by, None);
    }

    #[test]
    fn the_shells_own_commands_follow_and_none_the_backend_has_is_listed_twice() {
        let b = [info("event move", "Move a clip", "D\u{e9}placer un clip")];
        let rows = build_commands(&b, &[local("ui help"), local("event move")], "en");
        let names: Vec<&str> = rows.iter().map(|r| r.canonical.as_str()).collect();
        assert_eq!(names, ["event move", "ui help"]);
        assert_eq!(rows[1].menu, None);
    }

    #[test]
    fn the_longest_name_that_prefixes_the_line_wins() {
        let all: Vec<String> = ["track mute", "track mutex"].iter().map(|s| (*s).to_owned()).collect();
        assert_eq!(command_name_of("track mute gtr", &all).as_deref(), Some("track mute"));
        assert_eq!(command_name_of("track mute", &all).as_deref(), Some("track mute"));
        assert_eq!(command_name_of("wat is this", &all), None);
        // A name that merely starts the same is not a match.
        assert_eq!(command_name_of("track mutexx", &all), None);
    }

    fn lists() -> ShellLists {
        ShellLists {
            files: [
                "a/module.k7x",
                "b/clips/kick.k7x",
                "curves/wob.k7x",
                "x/curves/lfo.K7X",
                "clips/riff.k7x",
                "notes.md",
            ]
            .iter()
            .map(|s| (*s).to_owned())
            .collect(),
            entries: vec!["a".into(), "a/b.txt".into()],
            surfaces: vec!["timeline".into()],
            locales: vec!["en".into(), "fr".into()],
            timelines: vec!["verse".into()],
            tools: vec!["select".into()],
            wave_modes: vec![],
            roll_modes: vec![],
        }
    }

    fn values(canonical: &str, prior: &[&str], file: bool) -> Vec<String> {
        let prior: Vec<String> = prior.iter().map(|s| (*s).to_owned()).collect();
        match arg_source(canonical, &prior, &lists(), file) {
            ArgSource::Values { values } => values,
            ArgSource::Ask { .. } => panic!("asked"),
        }
    }

    #[test]
    fn the_shells_fixed_sets_are_literals_and_the_projects_are_the_pages_lists() {
        assert_eq!(values("ui panel", &[], false), ["event", "graph", "mix", "logs"]);
        assert_eq!(values("ui language", &[], false), ["en", "fr"]);
        assert_eq!(values("ui external", &[], false), ["timeline"]);
        assert_eq!(values("ui builder", &[], false), ["a/module.k7x"]);
        assert_eq!(values("ui curve", &[], false), ["wob", "lfo"]);
        assert_eq!(values("ui review", &[], false), ["kick", "riff"]);
        assert_eq!(values("fs file upload", &["x"], false), ["files", "folder"]);
    }

    #[test]
    fn project_export_follows_the_half_it_names() {
        assert_eq!(values("project export", &[], false), ["audio", "video", "timeline"]);
        assert_eq!(values("project export", &["video"], false), ["webm", "mp4"]);
        assert_eq!(values("project export", &["timeline"], false), ["otio", "edl"]);
        assert_eq!(values("project export", &["audio"], false), ["wav", "mp3"]);
        assert_eq!(
            values("project export", &["video", "webm"], false),
            ["wav", "mp3", "none"]
        );
        assert_eq!(values("project export", &["audio", "wav"], false), ["main", "verse"]);
        assert!(values("project export", &["audio", "wav", "x"], false).is_empty());
    }

    #[test]
    fn a_path_command_takes_the_tree_only_where_its_field_names_a_file() {
        assert_eq!(values("fs file move", &[], true), ["a", "a/b.txt"]);
        assert!(values("fs file move", &["a"], false).is_empty());
    }

    #[test]
    fn any_other_field_is_asked_of_the_backend_with_the_line_ending_in_a_space() {
        assert_eq!(
            arg_source("track mute", &[], &lists(), false),
            ArgSource::Ask {
                line: "track mute ".into(),
                prefix: "track mute ".into()
            }
        );
        assert_eq!(
            arg_source(" node bypass ", &["lead".into()], &lists(), false),
            ArgSource::Ask {
                line: "node bypass lead ".into(),
                prefix: "node bypass lead ".into()
            }
        );
        // Past its first field a fixed set is no longer the answer.
        assert!(matches!(
            arg_source("ui panel", &["event".into()], &lists(), false),
            ArgSource::Ask { .. }
        ));
    }
}
