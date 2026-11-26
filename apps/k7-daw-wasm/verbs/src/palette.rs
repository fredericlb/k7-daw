// palette.rs — the palette's catalogue: every command with each argument's display class, provider and clue.
// A reader would get wrong that a bad word aborts the build: it is reported and dropped, as a throw would empty the whole catalogue.

use crate::catalogue::{ArgSpec, Readable, Spec, specs};
use crate::complete::provider_info::{class_hint, is_provider, provider_class, provider_hint, provider_names};
use serde::Serialize;
use std::sync::OnceLock;

/// Where a right-click may offer a command, mirrored on the wire as `MENU_ENTITIES` in `web-protocol/commandTypes.ts`.
pub const MENU_ENTITIES: [&str; 16] = [
    "track", "group", "node", "event", "param", "curve", "ruler", "marker", "timeline", "assembly", "warp", "shot",
    "picture", "note", "tap", "rack",
];

/// The scalar shapes a one-token argument may declare.
const ARG_KINDS: [&str; 3] = ["timecode", "number", "note"];

/// One argument as the palette shows it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CommandArg {
    pub name: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    pub hint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,
}

/// One command as `GET /verb/catalogue` serves it, the shape of `CommandInfo` in `web-protocol/commandTypes.ts`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CommandInfo {
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readable: Option<Readable>,
    pub args: Vec<CommandArg>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub menu: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirm: Option<bool>,
}

/// The display class of an argument: its provider's, else `choice` for a literal set, else its shape, else `text`.
fn arg_class(a: &ArgSpec, provider: Option<&str>) -> String {
    if let Some(class) = provider.and_then(provider_class) {
        return class.to_owned();
    }
    if a.values.is_some() {
        return "choice".to_owned();
    }
    a.kind.clone().unwrap_or_else(|| "text".to_owned())
}

/// The clue beside the field: the argument's own, else its provider's, else its set, else its class. Empty: none.
fn arg_hint(a: &ArgSpec, provider: Option<&str>, class: &str) -> String {
    if let Some(h) = a.hint.as_deref().filter(|h| !h.is_empty()) {
        return h.to_owned();
    }
    if let Some(h) = provider.and_then(provider_hint) {
        return h.to_owned();
    }
    match &a.values {
        Some(v) => v.join(" · "),
        None => class_hint(class).to_owned(),
    }
}

fn arg_info(command: &str, a: &ArgSpec, problems: &mut Vec<String>) -> CommandArg {
    let mut provider = None;
    if let Some(want) = a.complete.as_deref() {
        if is_provider(want) {
            provider = Some(want);
        } else {
            let known = provider_names().collect::<Vec<_>>().join(", ");
            problems.push(format!(
                "\"{command}\" arg \"{}\" names an unknown completion provider \"{want}\" — known: {known}",
                a.name
            ));
        }
    }
    if let Some(kind) = a.kind.as_deref().filter(|k| !ARG_KINDS.contains(k)) {
        problems.push(format!(
            "\"{command}\" arg \"{}\" names an unknown type \"{kind}\" — known: {}",
            a.name,
            ARG_KINDS.join(", ")
        ));
    }
    let kind = arg_class(a, provider);
    CommandArg {
        name: a.name.clone(),
        provider: provider.map(str::to_owned),
        hint: arg_hint(a, provider, &kind),
        kind,
        optional: a.optional.then_some(true),
    }
}

/// The `menu:` words of one entry, unknown ones reported and dropped — a typo would silence a menu.
fn menu_list(command: &str, words: &[String], problems: &mut Vec<String>) -> Vec<String> {
    let mut kept = Vec::new();
    for w in words {
        if MENU_ENTITIES.contains(&w.as_str()) {
            kept.push(w.clone());
        } else {
            problems.push(format!(
                "\"{command}\" menu: names an unknown entity \"{w}\" — known: {}",
                MENU_ENTITIES.join(", ")
            ));
        }
    }
    kept
}

fn info_of(spec: &Spec, problems: &mut Vec<String>) -> CommandInfo {
    CommandInfo {
        command: spec.command.clone(),
        help: spec.help.clone(),
        readable: spec.readable.clone(),
        args: spec.args.iter().map(|a| arg_info(&spec.command, a, problems)).collect(),
        menu: spec.menu.as_ref().map(|w| menu_list(&spec.command, w, problems)),
        confirm: spec.confirm.then_some(true),
    }
}

/// The catalogue and what was wrong with it; an entry that names no surfaces runs on `cli`, the only one.
pub fn build(specs: &[Spec]) -> (Vec<CommandInfo>, Vec<String>) {
    let mut problems = Vec::new();
    let out = specs
        .iter()
        .filter(|s| s.surfaces.as_ref().is_none_or(|v| v.iter().any(|x| x == "cli")))
        .map(|s| info_of(s, &mut problems))
        .collect();
    (out, problems)
}

/// The palette's catalogue, built once; what was reported goes to stderr as the page's console did.
pub fn catalogue() -> &'static [CommandInfo] {
    static INFO: OnceLock<Vec<CommandInfo>> = OnceLock::new();
    INFO.get_or_init(|| {
        let (out, problems) = build(specs());
        for p in &problems {
            eprintln!("commands.yaml: {p}");
        }
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::complete::provider_info::ARG_CLASSES;
    use serde_json::json;

    const WIRE: &str = include_str!("../../../k7-daw-frontend/packages/web-protocol/commandTypes.ts");
    const PROVIDERS_SRC: &str = include_str!("complete/providers.rs");

    fn specs_of(v: serde_json::Value) -> Vec<Spec> {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn every_command_of_the_file_is_in_the_catalogue_and_none_is_reported() {
        let (out, problems) = build(specs());
        assert!(problems.is_empty(), "{problems:#?}");
        assert_eq!(out.len(), specs().len());
        let names: Vec<&str> = out.iter().map(|c| c.command.as_str()).collect();
        for s in specs() {
            assert!(names.contains(&s.command.as_str()), "{} is missing", s.command);
        }
    }

    #[test]
    fn a_readable_sentence_has_both_languages_within_the_row() {
        for c in catalogue() {
            let Some(r) = &c.readable else { continue };
            for (lang, text) in [("en", &r.en), ("fr", &r.fr)] {
                assert!(text.chars().count() <= 60, "{} {lang}: {text}", c.command);
            }
        }
        assert!(
            specs().iter().all(|s| s.readable.is_some()),
            "a command has no readable"
        );
    }

    #[test]
    fn a_known_command_has_the_shape_the_palette_draws() {
        let c = catalogue().iter().find(|c| c.command == "track rename").unwrap();
        assert_eq!(c.args.len(), 2);
        assert_eq!(
            serde_json::to_value(&c.args[0]).unwrap(),
            json!({ "name": "track", "kind": "track", "provider": "tracks", "hint": "a track of this project" })
        );
        assert_eq!(c.args[1].kind, "text");
        assert_eq!(c.args[1].hint, "the new name, prefix included — e.g. vo.lead");
        assert_eq!(c.menu.as_deref(), Some(&["track".to_owned()][..]));
        assert_eq!(
            c.readable.as_ref().map(|r| r.en.as_str()),
            Some("Rename a track and its file")
        );
        let v = serde_json::to_value(c).unwrap();
        assert!(v.get("confirm").is_none() && v.get("optional").is_none());
    }

    #[test]
    fn a_command_with_a_menu_and_a_confirm_carries_them() {
        assert!(catalogue().iter().any(|c| c.confirm == Some(true)));
        let menu = catalogue().iter().find_map(|c| c.menu.as_ref()).unwrap();
        assert!(menu.iter().all(|w| MENU_ENTITIES.contains(&w.as_str())));
    }

    #[test]
    fn an_argument_is_classed_by_provider_then_set_then_shape() {
        let (out, problems) = build(&specs_of(json!([{
            "command": "x", "sid": "x.y",
            "args": [
                { "name": "a", "complete": "nodes" },
                { "name": "b", "values": ["on", "off"] },
                { "name": "c", "type": "timecode" },
                { "name": "d", "type": "note", "hint": "mine", "optional": true },
                { "name": "e" },
            ],
        }])));
        assert!(problems.is_empty());
        let kinds: Vec<_> = out[0].args.iter().map(|a| (a.kind.as_str(), a.hint.as_str())).collect();
        assert_eq!(
            kinds[0],
            ("module", "a node of this project — one of its uses of a module")
        );
        assert_eq!(kinds[1], ("choice", "on · off"));
        assert_eq!(kinds[2], ("timecode", "bar:beat:sixteenth — 5, 5:1, 5:1:1"));
        assert_eq!(kinds[3], ("note", "mine"));
        assert_eq!(kinds[4], ("text", ""));
        assert_eq!(out[0].args[3].optional, Some(true));
        assert_eq!(out[0].args[0].optional, None);
    }

    #[test]
    fn a_typo_is_reported_and_dropped_never_thrown() {
        let (out, problems) = build(&specs_of(json!([{
            "command": "x", "sid": "x.y", "menu": ["track", "trak"],
            "args": [
                { "name": "a", "complete": "tracksy" },
                { "name": "b", "type": "numbr" },
            ],
        }])));
        assert_eq!(out[0].menu.as_deref(), Some(&["track".to_owned()][..]));
        assert_eq!(out[0].args[0].provider, None);
        assert_eq!(out[0].args[0].kind, "text");
        assert_eq!(out[0].args[1].kind, "numbr");
        assert_eq!(problems.len(), 3);
        assert!(problems[0].contains("unknown completion provider \"tracksy\""));
        assert!(problems[1].contains("unknown type \"numbr\""));
        assert!(problems[2].contains("unknown entity \"trak\""));
    }

    #[test]
    fn an_entry_off_the_only_surface_is_left_out() {
        let (out, _) = build(&specs_of(json!([
            { "command": "a", "sid": "a", "surfaces": ["cli"] },
            { "command": "b", "sid": "b", "surfaces": ["tray"] },
            { "command": "c", "sid": "c" },
        ])));
        let names: Vec<_> = out.iter().map(|c| c.command.as_str()).collect();
        assert_eq!(names, ["a", "c"]);
    }

    #[test]
    fn a_provider_has_a_class_and_a_clue() {
        assert_eq!(provider_class("samples"), Some("file"));
        assert_eq!(provider_class("assemblyTemplates"), Some("module"));
        assert_eq!(provider_class("nobody"), None);
        assert!(provider_hint("playlists").unwrap().contains("_playlists/"));
        for name in provider_names() {
            assert!(
                PROVIDERS_SRC.contains(&format!("\"{name}\" =>")),
                "{name} has no provider"
            );
            assert!(ARG_CLASSES.contains(&provider_class(name).unwrap()));
        }
    }

    #[test]
    fn the_wire_names_the_same_classes_and_entities() {
        for class in ARG_CLASSES {
            assert!(WIRE.contains(&format!("| \"{class}\"")), "{class} is not on the wire");
        }
        for entity in MENU_ENTITIES {
            assert!(WIRE.contains(&format!("\"{entity}\",")), "{entity} is not on the wire");
        }
        for name in provider_names() {
            assert!(WIRE.contains(&format!("| \"{name}\"")), "{name} is not a wire provider");
        }
    }
}
