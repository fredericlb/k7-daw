// compat.rs — can this engine play that project: the share of the plugins a project needs that an engine's catalogue has.
// A reader would get wrong that the score is an integer floor: only a full house reads 100.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// One plugin a project asks its engine for, as authored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PluginNeed {
    pub format: String,
    pub name: String,
}

/// One entry of an engine's catalogue, trimmed to what matching reads.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CataloguePlugin {
    pub format: String,
    pub name: String,
    /// A built-in's type name, which is how a module names it.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Compatibility {
    /// 0 to 100: needs the engine has over needs the project has.
    pub score: u32,
    pub missing: Vec<PluginNeed>,
}

fn key(format: &str, name: &str) -> String {
    format!("{}|{}", format.to_lowercase(), name.to_lowercase())
}

/// Scores one catalogue against one project's needs; a project needing nothing is 100 anywhere.
pub fn compatibility(needs: &[PluginNeed], catalogue: &[CataloguePlugin]) -> Compatibility {
    let mut have = HashSet::new();
    for p in catalogue {
        have.insert(key(&p.format, &p.name));
        if let Some(id) = p.id.as_deref().filter(|i| !i.is_empty())
            && p.format.eq_ignore_ascii_case("internal")
        {
            have.insert(key(&p.format, id));
        }
    }
    // A need named twice keeps its first place and its last spelling.
    let mut wanted: Vec<(String, &PluginNeed)> = Vec::new();
    for n in needs {
        let k = key(&n.format, &n.name);
        match wanted.iter_mut().find(|(seen, _)| *seen == k) {
            Some(slot) => slot.1 = n,
            None => wanted.push((k, n)),
        }
    }
    let total = wanted.len();
    if total == 0 {
        return Compatibility {
            score: 100,
            missing: Vec::new(),
        };
    }
    let missing: Vec<PluginNeed> = wanted
        .iter()
        .filter(|(k, _)| !have.contains(k))
        .map(|(_, n)| (*n).clone())
        .collect();
    Compatibility {
        score: ((total - missing.len()) * 100 / total) as u32,
        missing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn need(format: &str, name: &str) -> PluginNeed {
        PluginNeed {
            format: format.into(),
            name: name.into(),
        }
    }

    fn plugin(format: &str, name: &str, id: &str) -> CataloguePlugin {
        CataloguePlugin {
            format: format.into(),
            name: name.into(),
            id: Some(id.into()),
        }
    }

    fn catalogue() -> Vec<CataloguePlugin> {
        vec![
            plugin("internal", "4OSC", "4osc"),
            plugin("internal", "Volume & Pan", "volume"),
            plugin("VST3", "Serum", "/Library/Audio/Serum.vst3"),
        ]
    }

    #[test]
    fn is_100_when_the_engine_has_every_plugin() {
        let c = compatibility(&[need("internal", "4osc"), need("vst3", "Serum")], &catalogue());
        assert_eq!((c.score, c.missing.len()), (100, 0));
    }

    #[test]
    fn matches_a_built_in_by_its_type_name_as_well_as_its_display_name() {
        assert_eq!(compatibility(&[need("internal", "volume")], &catalogue()).score, 100);
        assert_eq!(
            compatibility(&[need("internal", "Volume & Pan")], &catalogue()).score,
            100
        );
    }

    #[test]
    fn is_available_over_required_and_names_what_is_missing() {
        let c = compatibility(
            &[need("vst3", "Serum"), need("vst3", "Diva"), need("internal", "4osc")],
            &catalogue(),
        );
        assert_eq!(c.score, 66);
        assert_eq!(c.missing, vec![need("vst3", "Diva")]);
    }

    #[test]
    fn counts_a_plugin_used_twice_once() {
        let c = compatibility(
            &[need("vst3", "Diva"), need("vst3", "diva"), need("vst3", "Serum")],
            &catalogue(),
        );
        assert_eq!(c.score, 50);
        assert_eq!(c.missing, vec![need("vst3", "diva")]);
    }

    #[test]
    fn does_not_match_a_plugin_across_formats() {
        assert_eq!(compatibility(&[need("vst", "Serum")], &catalogue()).score, 0);
    }

    #[test]
    fn is_100_for_a_project_that_needs_nothing_even_on_an_empty_engine() {
        let c = compatibility(&[], &[]);
        assert_eq!((c.score, c.missing.len()), (100, 0));
    }

    #[test]
    fn floors_in_integers_where_a_float_would_read_29_as_28() {
        let needs: Vec<_> = (0..100).map(|i| need("vst3", &format!("p{i}"))).collect();
        let have: Vec<_> = (0..29).map(|i| plugin("vst3", &format!("p{i}"), "")).collect();
        assert_eq!(compatibility(&needs, &have).score, 29);
    }
}
