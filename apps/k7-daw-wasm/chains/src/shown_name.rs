// shown_name.rs — what a person reads for a chain node: `<module> · <chain> · step n` when it is unnamed, its own name otherwise.
// The name itself (a uuid in a chain project) stays what commands, the wire and the files carry; this is only ever SHOWN.
// See apps/k7-daw-wasm/song/docs/chainFiles.md.

use indexmap::IndexMap;
use serde::Deserialize;

/// What a step says of itself for its shown name: its node, the module it uses and the place the graph gave it.
#[derive(Clone, Debug, Default, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct ShownStep {
    pub module: Option<String>,
    pub uses: Option<String>,
    pub place: Option<String>,
}

/// What a module ref is CALLED: the last segment, the whole ref staying as the header's `title`.
fn display_name(r: &str) -> &str {
    let name = r.rsplit_once('/').map_or(r, |(_, n)| n);
    if name.is_empty() { r } else { name }
}

/// One step's shown name: its module and place when the graph gave it a place, else its own name.
pub fn step_shown_name(module: &str, uses: Option<&str>, place: Option<&str>) -> String {
    match (place.filter(|p| !p.is_empty()), uses.filter(|u| !u.is_empty())) {
        (None, _) => module.to_owned(),
        (Some(p), Some(u)) => format!("{} · {p}", display_name(u)),
        (Some(p), None) => p.to_owned(),
    }
}

/// Each placed node's shown name, by node name; a node no step places is absent, and shows as itself.
pub fn shown_names(steps: &[ShownStep]) -> IndexMap<String, String> {
    let mut by = IndexMap::new();
    for s in steps {
        let (Some(module), Some(place)) = (
            s.module.as_deref().filter(|m| !m.is_empty()),
            s.place.as_deref().filter(|p| !p.is_empty()),
        ) else {
            continue;
        };
        by.insert(
            module.to_owned(),
            step_shown_name(module, s.uses.as_deref(), Some(place)),
        );
    }
    by
}

#[cfg(test)]
mod tests {
    use super::*;

    const UUID: &str = "30c0f1fa-41cb-499c-b101-705257778fe4";

    #[test]
    fn an_unnamed_node_is_named_by_its_module_and_place() {
        let shown = step_shown_name(UUID, Some("@k7-core/audio/delay"), Some("ge.trem · step 3"));
        assert_eq!(shown, "delay · ge.trem · step 3");
    }

    #[test]
    fn the_place_alone_stands_when_the_node_uses_no_module() {
        assert_eq!(
            step_shown_name(UUID, None, Some("ge.trem · step 2")),
            "ge.trem · step 2"
        );
    }

    #[test]
    fn a_named_nodes_own_name_is_kept() {
        assert_eq!(step_shown_name("delay", Some("@k7-core/audio/delay"), None), "delay");
    }

    fn step(module: &str, uses: Option<&str>, place: Option<&str>) -> ShownStep {
        ShownStep {
            module: Some(module.into()),
            uses: uses.map(str::to_owned),
            place: place.map(str::to_owned),
        }
    }

    #[test]
    fn a_node_is_shown_by_its_place_and_an_unplaced_one_is_left_out() {
        let by = shown_names(&[
            step("ge.trem", None, None),
            step(UUID, Some("@k7-core/audio/amp"), Some("ge.trem · step 2")),
        ]);
        assert_eq!(by.get(UUID).map(String::as_str), Some("amp · ge.trem · step 2"));
        assert!(!by.contains_key("ge.trem"));
    }

    #[test]
    fn laid_out_nodes_are_read_skipping_those_with_no_module() {
        let nameless = ShownStep {
            place: Some("x · step 1".into()),
            ..ShownStep::default()
        };
        let by = shown_names(&[nameless, step(UUID, None, Some("ge.trem · step 4"))]);
        assert_eq!(by.len(), 1);
        assert_eq!(by[UUID], "ge.trem · step 4");
    }
}
