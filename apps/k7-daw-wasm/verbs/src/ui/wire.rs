// wire.rs — the words the catalogue's wire types are closed over: an argument's class and provider, and what a right-click can be on.
// A reader would get wrong that the strings in `palette.rs` are checked here at build: a test holds each list to its table, and ts-rs writes the unions.

use serde::{Deserialize, Serialize};

/// The display class of an argument: it picks the colour, and the scalars say their own shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum ArgClass {
    Track,
    Module,
    File,
    Marker,
    Choice,
    Timecode,
    Number,
    Note,
    Text,
}

/// The precise type of thing an argument names, where the class is only its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ArgProvider {
    Tracks,
    Timelines,
    LaneTargets,
    TimelineLanes,
    ModuleAlternatives,
    ModuleEngines,
    ClipTargets,
    Chains,
    Curves,
    Modules,
    Nodes,
    BaseModules,
    EngineModules,
    Categories,
    ModuleTargets,
    MarkerNames,
    SequenceFiles,
    AudioFiles,
    Samples,
    Playlists,
    Libraries,
    PictureLanes,
    SurfaceTypes,
    Assemblies,
    AssemblyTemplates,
}

/// What a right-click can be on, as `commands.yaml`'s `menu:` names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum MenuEntity {
    Track,
    Group,
    Node,
    Event,
    Param,
    Curve,
    Ruler,
    Marker,
    Timeline,
    Assembly,
    Warp,
    Shot,
    Picture,
    Note,
    Tap,
    Rack,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::complete::provider_info::{ARG_CLASSES, provider_names};
    use crate::palette::MENU_ENTITIES;
    use serde_json::json;

    fn words<T: Serialize>(all: &[T]) -> Vec<String> {
        all.iter()
            .map(|v| json!(v).as_str().unwrap_or_default().to_owned())
            .collect()
    }

    #[test]
    fn the_classes_are_the_tables_classes() {
        use ArgClass::*;
        let all = [Track, Module, File, Marker, Choice, Timecode, Number, Note, Text];
        assert_eq!(words(&all), ARG_CLASSES);
    }

    #[test]
    fn the_entities_are_the_tables_entities() {
        use MenuEntity::*;
        let all = [
            Track, Group, Node, Event, Param, Curve, Ruler, Marker, Timeline, Assembly, Warp, Shot, Picture, Note, Tap,
            Rack,
        ];
        assert_eq!(words(&all), MENU_ENTITIES);
    }

    #[test]
    fn the_providers_are_the_tables_providers() {
        use ArgProvider::*;
        let all = [
            Tracks,
            Timelines,
            LaneTargets,
            TimelineLanes,
            ModuleAlternatives,
            ModuleEngines,
            ClipTargets,
            Chains,
            Curves,
            Modules,
            Nodes,
            BaseModules,
            EngineModules,
            Categories,
            ModuleTargets,
            MarkerNames,
            SequenceFiles,
            AudioFiles,
            Samples,
            Playlists,
            Libraries,
            PictureLanes,
            SurfaceTypes,
            Assemblies,
            AssemblyTemplates,
        ];
        let mut have = words(&all);
        let mut want: Vec<String> = provider_names().map(str::to_owned).collect();
        have.sort();
        want.sort();
        assert_eq!(have, want);
    }
}
