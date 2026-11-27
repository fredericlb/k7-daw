// home.rs — the HOME page's own command vocabulary: what the bar under the projects list offers, how a line is read, which workspace it acts in.
// A reader would get wrong that this runs a line: what a line DOES (the file service's acts) stays in the page; this says which command it is and what it names.

use crate::catalogue::Readable;
use crate::ui::menu::Text;
use serde::{Deserialize, Serialize};

/// The archetypes a project is created as.
pub const PROJECT_SHAPES: [&str; 5] = ["arrangement", "mix", "video", "live", "live-video"];

/// A param of a home command: its hint is the page's message, or a literal.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct HomeArg {
    pub name: String,
    pub kind: String,
    pub hint: Text,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub optional: Option<bool>,
}

/// One command of the home page's catalogue.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct HomeRow {
    pub canonical: String,
    pub label: String,
    pub names: Vec<String>,
    pub readable: String,
    pub readable_by: Readable,
    pub args: Vec<HomeArg>,
}

fn arg(name: &str, kind: &str, hint: Text, optional: bool) -> HomeArg {
    HomeArg {
        name: name.to_owned(),
        kind: kind.to_owned(),
        hint,
        optional: optional.then_some(true),
    }
}

fn spec(canonical: &str, en: &str, fr: &str, args: Vec<HomeArg>) -> (String, Readable, Vec<HomeArg>) {
    (
        canonical.to_owned(),
        Readable {
            en: en.to_owned(),
            fr: fr.to_owned(),
        },
        args,
    )
}

/// The catalogue in the same shape the editor's is: imperative, no example, no full stop, 60 characters at most in each language.
fn specs() -> Vec<(String, Readable, Vec<HomeArg>)> {
    let m = Text::msg;
    let project_of_ws = || m("hc.hint.projectOfWs");
    vec![
        spec(
            "workspace open",
            "Find a workspace and open its files",
            "Trouver un espace de travail et ouvrir ses fichiers",
            vec![arg("workspace", "choice", m("hc.hint.workspace"), false)],
        ),
        spec(
            "project open",
            "Find a project and open it",
            "Trouver un projet et l'ouvrir",
            vec![arg("project", "choice", m("hc.hint.projectRef"), false)],
        ),
        spec(
            "project new",
            "Create a project in the open workspace",
            "Cr\u{e9}er un projet dans l'espace de travail ouvert",
            vec![
                arg("type", "choice", Text::plain(PROJECT_SHAPES.join(" \u{b7} ")), false),
                arg("name", "text", m("hc.hint.projectName"), false),
            ],
        ),
        spec(
            "project copy",
            "Copy a project into another workspace",
            "Copier un projet vers un autre espace de travail",
            vec![
                arg("name", "file", project_of_ws(), false),
                arg("workspace", "choice", m("hc.hint.destination"), false),
                arg("new_name", "text", m("hc.hint.copyName"), true),
            ],
        ),
        spec(
            "project rename",
            "Rename a project",
            "Renommer un projet",
            vec![
                arg("name", "file", project_of_ws(), false),
                arg("new_name", "text", m("hc.hint.newName"), false),
            ],
        ),
        // Deleting is two verbs: naming the project twice makes the second Enter a decision, and `init` says what will happen.
        spec(
            "project delete init",
            "Say which project to delete, and what that does",
            "D\u{e9}signer le projet \u{e0} supprimer, et dire ce que \u{e7}a fait",
            vec![arg("name", "file", project_of_ws(), false)],
        ),
        spec(
            "project delete confirm",
            "Delete the project just named",
            "Supprimer le projet d\u{e9}sign\u{e9}",
            vec![arg("name", "file", m("hc.hint.sameAgain"), false)],
        ),
        spec("help", "Open the documentation", "Ouvrir la documentation", vec![]),
        // Cameras return as an app feature, so `camera mode` is not listed (docs/projects.md).
        spec(
            "system workspace new",
            "Make an empty workspace",
            "Cr\u{e9}er un espace de travail vide",
            vec![arg("name", "text", m("hc.hint.workspaceName"), false)],
        ),
    ]
}

/// The catalogue as the bar takes it, with the sentence resolved once by the locale.
#[must_use]
pub fn catalogue(locale: &str) -> Vec<HomeRow> {
    let fr = locale.starts_with("fr");
    specs()
        .into_iter()
        .map(|(canonical, readable, args)| HomeRow {
            label: canonical.clone(),
            names: vec![canonical.clone()],
            readable: if fr { readable.fr.clone() } else { readable.en.clone() },
            readable_by: readable,
            canonical,
            args,
        })
        .collect()
}

/// What a line is: nothing, a verb this screen does not have, or one of its commands and the rest of the line.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "is", rename_all = "lowercase")]
pub enum Parsed {
    Empty,
    Unknown,
    Command { canonical: String, rest: String },
}

/// Which command a line runs: the longest name first, so `project delete init` is not read as `project delete` with a word after it.
#[must_use]
pub fn parse_line(raw: &str) -> Parsed {
    let line = song_edit::js_text::js_trim(raw);
    if line.is_empty() {
        return Parsed::Empty;
    }
    let mut names: Vec<String> = specs().into_iter().map(|s| s.0).collect();
    // A stable sort by length, longest first, as the table's own order breaks a tie.
    names.sort_by_key(|n| std::cmp::Reverse(n.len()));
    for c in names {
        if line == c || line.strip_prefix(c.as_str()).is_some_and(|r| r.starts_with(' ')) {
            return Parsed::Command {
                rest: song_edit::js_text::js_trim(&line[c.len()..]).to_owned(),
                canonical: c,
            };
        }
    }
    Parsed::Unknown
}

/// A project of a workspace, as the screen lists it.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProjectRef {
    pub path: String,
}

/// A workspace of the account, as the screen lists it.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRef {
    pub name: String,
    #[serde(default)]
    pub is_single_project: bool,
    pub projects: Vec<ProjectRef>,
}

/// What the screen knows when a line is read: the workspaces it holds and the one the address has unfolded.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct HomeContext {
    pub workspaces: Vec<WorkspaceRef>,
    #[serde(default)]
    pub open_ws: Option<String>,
}

/// The workspace a `project ...` line acts in, by position, or the sentence refusing by name.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "is", rename_all = "lowercase")]
pub enum Resolved {
    Workspace { index: usize },
    Error { text: Text },
}

/// Which workspace a `project ...` line acts in: the address first, then an account holding exactly one; anything else refused by name.
#[must_use]
pub fn resolve_workspace(ctx: &HomeContext) -> Resolved {
    if let Some(open) = ctx.open_ws.as_deref().filter(|o| !o.is_empty())
        && let Some(index) = ctx.workspaces.iter().position(|w| w.name == open)
    {
        return Resolved::Workspace { index };
    }
    match ctx.workspaces.len() {
        1 => Resolved::Workspace { index: 0 },
        0 => Resolved::Error {
            text: Text::msg("hc.noWorkspaces"),
        },
        _ => Resolved::Error {
            text: Text::msg_with(
                "hc.whichRepo",
                "names",
                ctx.workspaces
                    .iter()
                    .map(|w| w.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
        },
    }
}

/// The stem of a project's folder: its last segment without its extension.
fn stem(path: &str) -> &str {
    let last = path.rsplit('/').next().unwrap_or(path);
    match last.rfind('.') {
        Some(i) if i + 1 < last.len() => &last[..i],
        _ => last,
    }
}

/// A project of that workspace, named by its path, by its folder's name or by the folder's stem (`hiver`), as an index.
#[must_use]
pub fn find_project(ws: &WorkspaceRef, name: &str) -> Option<usize> {
    let want = song_edit::js_text::js_trim(name);
    let last = |p: &ProjectRef| p.path.rsplit('/').next().unwrap_or(&p.path).to_owned();
    ws.projects
        .iter()
        .position(|p| p.path == want)
        .or_else(|| ws.projects.iter().position(|p| last(p) == want))
        .or_else(|| ws.projects.iter().position(|p| stem(&p.path) == want))
}

/// One param's whole vocabulary for the field that is open, answered at once: this page already polls both services.
#[must_use]
pub fn arg_values(canonical: &str, arg_index: usize, ctx: &HomeContext) -> Vec<String> {
    let paths = |ctx: &HomeContext| match resolve_workspace(ctx) {
        Resolved::Workspace { index } => ctx.workspaces[index].projects.iter().map(|p| p.path.clone()).collect(),
        Resolved::Error { .. } => vec![],
    };
    match (canonical, arg_index) {
        ("project new", 0) => PROJECT_SHAPES.iter().map(|s| (*s).to_owned()).collect(),
        ("workspace open", 0) => ctx.workspaces.iter().map(|w| w.name.clone()).collect(),
        ("project open", 0) => ctx
            .workspaces
            .iter()
            .flat_map(|w| w.projects.iter().map(move |p| format!("{}/{}", w.name, p.path)))
            .collect(),
        (c, 0) if c.starts_with("project delete") => paths(ctx),
        ("project rename" | "project copy", 0) => paths(ctx),
        // The same list the dialog offers: the destinations that are not the source and not a single project.
        ("project copy", 1) => {
            let found = resolve_workspace(ctx);
            ctx.workspaces
                .iter()
                .filter(|w| {
                    !w.is_single_project
                        && match &found {
                            Resolved::Workspace { index } => w.name != ctx.workspaces[*index].name,
                            Resolved::Error { .. } => true,
                        }
                })
                .map(|w| w.name.clone())
                .collect()
        }
        _ => vec![],
    }
}

/// The sentence refusing a project no workspace holds, naming the ones there are.
#[must_use]
pub fn project_gripe(ws: &WorkspaceRef, name: &str) -> Text {
    let list = if ws.projects.is_empty() {
        String::new()
    } else {
        format!(
            " \u{2014} {}",
            ws.projects
                .iter()
                .map(|p| p.path.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let mut t = Text::msg("hc.noProject");
    let mut params = serde_json::Map::new();
    params.insert("name".into(), name.into());
    params.insert("workspace".into(), ws.name.clone().into());
    params.insert("list".into(), list.into());
    t.params = Some(params);
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws(name: &str, projects: &[&str]) -> WorkspaceRef {
        WorkspaceRef {
            name: name.into(),
            is_single_project: false,
            projects: projects.iter().map(|p| ProjectRef { path: (*p).to_owned() }).collect(),
        }
    }

    fn ctx(workspaces: Vec<WorkspaceRef>, open: Option<&str>) -> HomeContext {
        HomeContext {
            workspaces,
            open_ws: open.map(Into::into),
        }
    }

    #[test]
    fn every_row_has_a_sentence_in_both_languages_within_the_palettes_row() {
        for lang in ["en", "fr"] {
            for r in catalogue(lang) {
                assert!(!r.readable.is_empty(), "{}", r.canonical);
                assert!(
                    r.readable.chars().count() <= 60,
                    "{lang} {}: {}",
                    r.canonical,
                    r.readable
                );
            }
        }
        assert_eq!(
            catalogue("fr-FR")[0].readable,
            "Trouver un espace de travail et ouvrir ses fichiers"
        );
        assert_eq!(
            catalogue("en")[0].readable_by.fr,
            "Trouver un espace de travail et ouvrir ses fichiers"
        );
    }

    #[test]
    fn the_cameras_verb_is_not_listed_while_cameras_are_away() {
        assert!(catalogue("en").iter().all(|r| r.canonical != "camera mode"));
    }

    #[test]
    fn a_line_names_its_command_longest_first_and_the_rest_is_trimmed() {
        assert_eq!(
            parse_line("  project delete init  hiver "),
            Parsed::Command {
                canonical: "project delete init".into(),
                rest: "hiver".into()
            }
        );
        assert_eq!(
            parse_line("project new live my song"),
            Parsed::Command {
                canonical: "project new".into(),
                rest: "live my song".into()
            }
        );
        assert_eq!(
            parse_line("help"),
            Parsed::Command {
                canonical: "help".into(),
                rest: String::new()
            }
        );
        assert_eq!(parse_line("   "), Parsed::Empty);
        assert_eq!(parse_line("project newer x"), Parsed::Unknown);
        assert_eq!(parse_line("frobnicate"), Parsed::Unknown);
    }

    #[test]
    fn the_workspace_is_the_open_one_then_the_only_one_and_anything_else_is_refused_by_name() {
        let two = vec![ws("a", &[]), ws("b", &[])];
        assert_eq!(
            resolve_workspace(&ctx(two.clone(), Some("b"))),
            Resolved::Workspace { index: 1 }
        );
        assert_eq!(
            resolve_workspace(&ctx(vec![ws("solo", &[])], None)),
            Resolved::Workspace { index: 0 }
        );
        assert!(matches!(
            resolve_workspace(&ctx(vec![], None)),
            Resolved::Error { text } if text.key.as_deref() == Some("hc.noWorkspaces")
        ));
        let Resolved::Error { text } = resolve_workspace(&ctx(two, Some("gone"))) else {
            panic!("resolved");
        };
        assert_eq!(text.key.as_deref(), Some("hc.whichRepo"));
        assert_eq!(text.params.unwrap()["names"], "a, b");
    }

    #[test]
    fn a_project_is_found_by_path_by_folder_or_by_stem() {
        let w = ws("w", &["songs/hiver.k7", "x/other"]);
        assert_eq!(find_project(&w, "songs/hiver.k7"), Some(0));
        assert_eq!(find_project(&w, "hiver.k7"), Some(0));
        assert_eq!(find_project(&w, " hiver "), Some(0));
        assert_eq!(find_project(&w, "other"), Some(1));
        assert_eq!(find_project(&w, "nope"), None);
    }

    #[test]
    fn a_field_lists_what_this_screen_holds() {
        let c = ctx(
            vec![
                ws("a", &["p1", "p2"]),
                WorkspaceRef {
                    is_single_project: true,
                    ..ws("single", &["x"])
                },
                ws("c", &[]),
            ],
            Some("a"),
        );
        assert_eq!(arg_values("project new", 0, &c), PROJECT_SHAPES);
        assert_eq!(arg_values("workspace open", 0, &c), ["a", "single", "c"]);
        assert_eq!(arg_values("project open", 0, &c), ["a/p1", "a/p2", "single/x"]);
        assert_eq!(arg_values("project delete init", 0, &c), ["p1", "p2"]);
        assert_eq!(arg_values("project rename", 0, &c), ["p1", "p2"]);
        assert_eq!(arg_values("project copy", 0, &c), ["p1", "p2"]);
        // Not the source, and not a single project.
        assert_eq!(arg_values("project copy", 1, &c), ["c"]);
        assert!(arg_values("project copy", 2, &c).is_empty());
        let none = ctx(vec![ws("a", &["p"]), ws("b", &[])], None);
        assert!(arg_values("project rename", 0, &none).is_empty());
    }

    #[test]
    fn a_refusal_names_the_projects_there_are() {
        let t = project_gripe(&ws("w", &["a", "b"]), "z");
        assert_eq!(t.params.as_ref().unwrap()["list"], " \u{2014} a, b");
        assert_eq!(project_gripe(&ws("w", &[]), "z").params.unwrap()["list"], "");
    }
}
