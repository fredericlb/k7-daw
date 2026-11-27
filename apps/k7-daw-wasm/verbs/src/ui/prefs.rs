// prefs.rs — what a person has made of the command catalogue: what each command is called, its menus, and the order it is listed in.
// A reader would get wrong that prefs hold unknown fields away: an entry is a JSON object, and only the keys named here are read or written.

use super::order::locale_cmp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use song_edit::js_text::{is_js_space, js_trim};
use std::collections::HashSet;

/// Keyed by the canonical line, never by what is displayed: the one id both halves of the catalogue have.
pub type Prefs = Map<String, Value>;

/// The least a command has to have for these rules: its line, and its sentence when it has one.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Like {
    pub canonical: String,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub readable: Option<String>,
}

fn pref<'a>(prefs: &'a Prefs, canonical: &str) -> Option<&'a Map<String, Value>> {
    prefs.get(canonical).and_then(Value::as_object)
}

/// A person's name for a command in one locale, as authored: never trimmed here, which is what broke the space bar on the settings page.
#[must_use]
pub fn label_in(prefs: &Prefs, canonical: &str, locale: &str) -> String {
    pref(prefs, canonical)
        .and_then(|p| p.get("label"))
        .and_then(Value::as_object)
        .and_then(|l| l.get(locale))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned()
}

/// The entity a command acts on: the first word of its line, so nothing has to be authored and nothing can drift.
#[must_use]
pub fn entity_of(canonical: &str) -> &str {
    let head = js_trim(canonical).split(is_js_space).next().unwrap_or("");
    if head.is_empty() { canonical } else { head }
}

fn heads_of(commands: &[Like]) -> HashSet<&str> {
    commands.iter().map(|c| entity_of(&c.canonical)).collect()
}

/// The entity a command is listed under: its head, folded to the singular when the singular is a head too.
#[must_use]
pub fn entity_group<'a>(canonical: &'a str, heads: &HashSet<&str>) -> &'a str {
    let head = entity_of(canonical);
    match head.strip_suffix('s') {
        Some(singular) if !singular.is_empty() && heads.contains(singular) => singular,
        _ => head,
    }
}

/// What to call a command: this person's name in this locale, else the catalogue's sentence, else the line: never a blank row.
#[must_use]
pub fn label_of(c: &Like, prefs: &Prefs, locale: &str) -> String {
    // Trimmed here and not in `label_in`, which answers the settings field.
    let own = label_in(prefs, &c.canonical, locale);
    let own = js_trim(&own);
    if !own.is_empty() {
        return own.to_owned();
    }
    match c.readable.as_deref().map(js_trim) {
        Some(r) if !r.is_empty() => r.to_owned(),
        _ => c.canonical.clone(),
    }
}

/// One command as the settings page and the palette list it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RowView {
    pub group: String,
    /// The plural command of its group: the one that acts on several at once.
    pub plural: bool,
    pub label: String,
}

/// The catalogue as this person sees it: each row's group and label, and every entity present in a stable order.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct View {
    pub rows: Vec<RowView>,
    pub entities: Vec<String>,
}

#[must_use]
pub fn view(commands: &[Like], prefs: &Prefs, locale: &str) -> View {
    let heads = heads_of(commands);
    let rows: Vec<RowView> = commands
        .iter()
        .map(|c| {
            let group = entity_group(&c.canonical, &heads);
            RowView {
                group: group.to_owned(),
                plural: entity_of(&c.canonical) != group,
                label: label_of(c, prefs, locale),
            }
        })
        .collect();
    let mut entities: Vec<String> = rows
        .iter()
        .map(|r| r.group.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    entities.sort_by(|a, b| locale_cmp(a, b));
    View { rows, entities }
}

/// Which entities a command is offered on as things stand: none meaning "wherever it fits", the default.
#[must_use]
pub fn menu_choice(pref: Option<&Map<String, Value>>, catalogue: Option<&[String]>) -> Option<Vec<String>> {
    match pref.and_then(|p| p.get("menu")) {
        Some(Value::Bool(false)) => Some(Vec::new()),
        Some(Value::Array(list)) => Some(list.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect()),
        // `true` is the person overriding a catalogue restriction back to "everywhere".
        Some(Value::Bool(true)) => None,
        _ => catalogue.map(<[String]>::to_vec),
    }
}

/// May a right-click on `entity` offer `canonical`? It takes the line, so the menu's hand-written entries obey too.
#[must_use]
pub fn in_menu(canonical: &str, prefs: &Prefs, entity: Option<&str>, catalogue: Option<&[String]>) -> bool {
    let Some(allowed) = menu_choice(pref(prefs, canonical), catalogue) else {
        return true;
    };
    // No entity named: the caller asks whether any menu offers it, which is the settings page's question.
    entity.map_or(!allowed.is_empty(), |e| allowed.iter().any(|a| a == e))
}

fn order_of(prefs: &Prefs, canonical: &str) -> Option<f64> {
    pref(prefs, canonical)
        .and_then(|p| p.get("order"))
        .and_then(Value::as_f64)
}

/// The catalogue as this person arranged it, as indices: placed commands first by `order`, the rest in catalogue order.
#[must_use]
pub fn order_commands(commands: &[Like], prefs: &Prefs) -> Vec<usize> {
    let mut at: Vec<(usize, Option<f64>)> = commands
        .iter()
        .enumerate()
        .map(|(i, c)| (i, order_of(prefs, &c.canonical)))
        .collect();
    at.sort_by(|(i, a), (j, b)| match (a, b) {
        (None, None) => i.cmp(j),
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(_), None) => std::cmp::Ordering::Less,
        (Some(x), Some(y)) => x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal).then(i.cmp(j)),
    });
    at.into_iter().map(|(i, _)| i).collect()
}

/// Moves `canonical` one place up or down: it renumbers the whole list, "one place up" meaning something only against an arrangement.
#[must_use]
pub fn move_command(commands: &[Like], prefs: &Prefs, canonical: &str, by: i64) -> Prefs {
    let list = order_commands(commands, prefs);
    let Some(at) = list.iter().position(|&i| commands[i].canonical == canonical) else {
        return prefs.clone();
    };
    let to = at as i64 + by;
    if to < 0 || to >= list.len() as i64 {
        // An end is an end.
        return prefs.clone();
    }
    let mut moved = list;
    moved.swap(at, to as usize);
    let mut next = prefs.clone();
    for (n, &i) in moved.iter().enumerate() {
        let mut entry = pref(&next, &commands[i].canonical).cloned().unwrap_or_default();
        entry.insert("order".to_owned(), Value::from(n));
        next.insert(commands[i].canonical.clone(), Value::Object(entry));
    }
    next
}

/// Sets fields of one command's entry, dropping the entry when nothing is left to say; a null or an empty string takes a field back.
#[must_use]
pub fn set_command_pref(prefs: &Prefs, canonical: &str, patch: &Map<String, Value>) -> Prefs {
    let mut merged = pref(prefs, canonical).cloned().unwrap_or_default();
    for (k, v) in patch {
        merged.insert(k.clone(), v.clone());
    }
    merged.retain(|_, v| !v.is_null() && v.as_str() != Some(""));
    let mut next = prefs.clone();
    if merged.is_empty() {
        next.shift_remove(canonical);
    } else {
        next.insert(canonical.to_owned(), Value::Object(merged));
    }
    next
}

/// Says where a right-click may offer a command, in the spelling that says the least.
#[must_use]
pub fn set_menu_entities(
    prefs: &Prefs,
    canonical: &str,
    catalogue: Option<&[String]>,
    next: &[String],
    all: &[String],
) -> Prefs {
    let same = |a: &[String], b: &[String]| a.len() == b.len() && a.iter().all(|x| b.contains(x));
    let menu = if next.is_empty() {
        Value::Bool(false)
    } else if same(next, catalogue.unwrap_or(all)) {
        // Back to the catalogue's word: a null is taken back out of the entry.
        Value::Null
    } else if same(next, all) {
        Value::Bool(true)
    } else {
        Value::from(next.to_vec())
    };
    let mut patch = Map::new();
    patch.insert("menu".to_owned(), menu);
    set_command_pref(prefs, canonical, &patch)
}

/// Names a command in one language, leaving the others; a blank text takes that language's name back.
#[must_use]
pub fn set_command_label(prefs: &Prefs, canonical: &str, locale: &str, text: &str) -> Prefs {
    let mut by_locale = pref(prefs, canonical)
        .and_then(|p| p.get("label"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    by_locale.insert(locale.to_owned(), Value::from(text));
    by_locale.retain(|_, v| v.as_str().is_none_or(|s| !js_trim(s).is_empty()));
    let mut patch = Map::new();
    patch.insert(
        "label".to_owned(),
        if by_locale.is_empty() {
            Value::Null
        } else {
            Value::Object(by_locale)
        },
    );
    set_command_pref(prefs, canonical, &patch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn like(canonical: &str, readable: Option<&str>) -> Like {
        Like {
            canonical: canonical.into(),
            readable: readable.map(Into::into),
        }
    }

    fn prefs(v: Value) -> Prefs {
        v.as_object().cloned().unwrap_or_default()
    }

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn a_command_is_called_by_the_person_then_the_sentence_then_the_line() {
        let p = prefs(json!({ "track mute": { "label": { "en": "  Hush  ", "fr": "" } } }));
        assert_eq!(label_of(&like("track mute", Some("Mute")), &p, "en"), "Hush");
        assert_eq!(label_of(&like("track mute", Some(" Mute ")), &p, "fr"), "Mute");
        assert_eq!(label_of(&like("track solo", Some("  ")), &p, "en"), "track solo");
        assert_eq!(label_of(&like("track solo", None), &p, "en"), "track solo");
        // The settings field reads it as authored, spaces and all.
        assert_eq!(label_in(&p, "track mute", "en"), "  Hush  ");
        assert_eq!(label_in(&p, "track mute", "de"), "");
    }

    #[test]
    fn an_entity_is_the_first_word_and_a_plural_folds_into_its_singular() {
        assert_eq!(entity_of("track mute"), "track");
        assert_eq!(entity_of("  track  mute"), "track");
        assert_eq!(entity_of(""), "");
        let cmds = [
            like("event split", None),
            like("events move", None),
            like("ui help", None),
            like("us x", None),
        ];
        let v = view(&cmds, &Prefs::new(), "en");
        assert_eq!(v.rows[1].group, "event");
        assert!(v.rows[1].plural);
        assert!(!v.rows[0].plural);
        // `us` folds to nothing: `u` is no head.
        assert_eq!(v.rows[3].group, "us");
        assert_eq!(v.entities, strings(&["event", "ui", "us"]));
    }

    #[test]
    fn a_menu_choice_reads_false_as_nowhere_true_as_everywhere_and_a_list_as_itself() {
        let cat = strings(&["track"]);
        let none = prefs(json!({}));
        assert_eq!(menu_choice(pref(&none, "x"), Some(&cat)), Some(cat.clone()));
        assert_eq!(menu_choice(pref(&none, "x"), None), None);
        let p = prefs(json!({ "a": { "menu": false }, "b": { "menu": true }, "c": { "menu": ["clip"] } }));
        assert_eq!(menu_choice(pref(&p, "a"), Some(&cat)), Some(vec![]));
        assert_eq!(menu_choice(pref(&p, "b"), Some(&cat)), None);
        assert_eq!(menu_choice(pref(&p, "c"), Some(&cat)), Some(strings(&["clip"])));
        assert!(!in_menu("a", &p, Some("track"), None));
        assert!(!in_menu("a", &p, None, None));
        assert!(in_menu("b", &p, Some("track"), Some(&cat)));
        assert!(in_menu("c", &p, Some("clip"), None));
        assert!(!in_menu("c", &p, Some("track"), None));
        assert!(in_menu("c", &p, None, None));
    }

    #[test]
    fn placed_commands_come_first_by_order_and_the_rest_in_catalogue_order() {
        let cmds = [like("a", None), like("b", None), like("c", None), like("d", None)];
        let p = prefs(json!({ "c": { "order": 0 }, "b": { "order": 1 } }));
        assert_eq!(order_commands(&cmds, &p), vec![2, 1, 0, 3]);
        assert_eq!(order_commands(&cmds, &Prefs::new()), vec![0, 1, 2, 3]);
    }

    #[test]
    fn moving_renumbers_the_whole_list_and_an_end_is_an_end() {
        let cmds = [like("a", None), like("b", None), like("c", None)];
        let moved = move_command(&cmds, &Prefs::new(), "b", -1);
        assert_eq!(
            Value::Object(moved),
            json!({ "b": { "order": 0 }, "a": { "order": 1 }, "c": { "order": 2 } })
        );
        assert_eq!(move_command(&cmds, &Prefs::new(), "a", -1), Prefs::new());
        assert_eq!(move_command(&cmds, &Prefs::new(), "c", 1), Prefs::new());
        assert_eq!(move_command(&cmds, &Prefs::new(), "zzz", 1), Prefs::new());
    }

    #[test]
    fn a_pref_taken_back_leaves_no_empty_object() {
        let p = prefs(json!({ "a": { "icon": "Scissors", "order": 0 } }));
        let patch = prefs(json!({ "icon": null }));
        assert_eq!(
            Value::Object(set_command_pref(&p, "a", &patch)),
            json!({ "a": { "order": 0 } })
        );
        let both = prefs(json!({ "icon": "", "order": null }));
        assert_eq!(set_command_pref(&p, "a", &both), Prefs::new());
        // A false and a zero are answers, not absences.
        let keep = prefs(json!({ "menu": false, "order": 0 }));
        assert_eq!(
            Value::Object(set_command_pref(&Prefs::new(), "a", &keep)),
            json!({ "a": { "menu": false, "order": 0 } })
        );
    }

    #[test]
    fn a_menu_is_written_in_the_spelling_that_says_the_least() {
        let all = strings(&["track", "clip", "node"]);
        let cat = strings(&["track"]);
        let set = |next: &[&str], catalogue: Option<&[String]>| {
            Value::Object(set_menu_entities(&Prefs::new(), "x", catalogue, &strings(next), &all))
        };
        assert_eq!(set(&[], Some(&cat)), json!({ "x": { "menu": false } }));
        assert_eq!(set(&["track"], Some(&cat)), json!({}));
        assert_eq!(
            set(&["node", "clip", "track"], Some(&cat)),
            json!({ "x": { "menu": true } })
        );
        assert_eq!(set(&["clip"], Some(&cat)), json!({ "x": { "menu": ["clip"] } }));
        assert_eq!(set(&["clip", "node", "track"], None), json!({}));
    }

    #[test]
    fn a_label_is_set_per_language_and_a_blank_one_goes() {
        let p = set_command_label(&Prefs::new(), "a", "en", "Hush");
        let p = set_command_label(&p, "a", "fr", "Chut");
        assert_eq!(
            Value::Object(p.clone()),
            json!({ "a": { "label": { "en": "Hush", "fr": "Chut" } } })
        );
        let p = set_command_label(&p, "a", "en", "   ");
        assert_eq!(Value::Object(p.clone()), json!({ "a": { "label": { "fr": "Chut" } } }));
        assert_eq!(set_command_label(&p, "a", "fr", ""), Prefs::new());
    }
}
