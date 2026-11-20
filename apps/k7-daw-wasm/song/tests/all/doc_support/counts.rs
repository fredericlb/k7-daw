// doc_support/counts.rs — what a document DID, counted off its JSON: the gate's minimums count the effect and never the field.
// A reader would get wrong that a count is taken only over documents that compared equal to the TypeScript's.

use serde_json::Value;
use std::collections::BTreeMap;

/// Named counts, added up across documents.
#[derive(Clone, Debug, Default)]
pub struct Counts(pub BTreeMap<&'static str, usize>);

impl Counts {
    pub fn add(&mut self, name: &'static str, n: usize) {
        *self.0.entry(name).or_default() += n;
    }

    pub fn get(&self, name: &str) -> usize {
        self.0.get(name).copied().unwrap_or(0)
    }

    pub fn merge(&mut self, o: &Counts) {
        for (k, v) in &o.0 {
            self.add(k, *v);
        }
    }
}

fn list<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key).and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

fn has(v: &Value, key: &str) -> bool {
    v.get(key).is_some()
}

fn flag(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool) == Some(true)
}

fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

/// Counts what one compiled document holds.
pub fn count_doc(doc: &Value, c: &mut Counts) {
    c.add("documents", 1);
    let tracks = list(doc, "tracks");
    c.add("tracks", tracks.len());
    c.add("master plugins", list(doc, "masterPlugins").len());
    c.add(
        "recorders",
        list(doc, "masterPlugins")
            .iter()
            .filter(|p| text(p, "id") == "MIX:@record")
            .count(),
    );
    c.add("groups", list(doc, "groups").len());
    let recorded = list(doc, "masterPlugins")
        .iter()
        .any(|p| text(p, "id") == "MIX:@record");
    c.add("documents without a recorder", usize::from(!recorded));
    let members = |g: &Value| tracks.iter().any(|t| text(t, "group") == text(g, "id"));
    for g in list(doc, "groups") {
        c.add("groups muted", usize::from(flag(g, "mute") && members(g)));
        c.add("groups soloed", usize::from(flag(g, "solo") && members(g)));
    }
    c.add("timeline documents", usize::from(has(doc, "timelines")));
    c.add("markers", list(doc, "markers").len());
    let mut lanes: BTreeMap<&str, usize> = BTreeMap::new();
    for t in tracks {
        let id = text(t, "id");
        *lanes.entry(id.split('#').next().unwrap_or(id)).or_default() += 1;
        c.add("clips", list(t, "clips").len());
        let effect = text(t, "role") == "effect";
        c.add("effect tracks", usize::from(effect));
        c.add("effect tracks muted", usize::from(effect && flag(t, "mute")));
        c.add("tracks in a group", usize::from(!text(t, "group").is_empty()));
        c.add("labelled strips", usize::from(text(t, "name") != id));
        c.add("generated lanes", usize::from(flag(t, "generated")));
        c.add("monitor not auto", usize::from(text(t, "monitor") != "auto"));
        c.add("highlights", usize::from(flag(t, "highlight")));
        c.add("sends", list(t, "sends").len());
        c.add(
            "sends not at unity",
            list(t, "sends")
                .iter()
                .filter(|s| s.get("level").and_then(Value::as_f64) != Some(1.0))
                .count(),
        );
        c.add("automation lanes", list(t, "automation").len());
        c.add(
            "automation points",
            list(t, "automation").iter().map(|a| list(a, "points").len()).sum(),
        );
        c.add("input from", usize::from(has(t, "inputFrom")));
        c.add(
            "live inputs with channels",
            usize::from(!list(t, "inputChannels").is_empty()),
        );
        c.add(
            "cue buses heard",
            usize::from(effect && !list(t, "outputChannels").is_empty()),
        );
        c.add(
            "cue buses silent",
            usize::from(effect && flag(t, "mute") && list(t, "outputChannels").is_empty()),
        );
        c.add(
            match text(t, "contentType") {
                "notes" => "content notes",
                "audio notes" => "content audio notes",
                _ => "content audio",
            },
            1,
        );
        c.add(
            "frozen clips",
            list(t, "clips")
                .iter()
                .filter(|k| text(k, "id").ends_with("__frozen"))
                .count(),
        );
        c.add("frozen material kept", usize::from(has(t, "frozenMaterial")));
        c.add(
            "lanes emptied",
            usize::from(has(t, "frozenMaterial") && list(t, "clips").is_empty()),
        );
        c.add(
            "render on a bus",
            usize::from(effect && list(t, "clips").iter().any(|k| text(k, "id").ends_with("__frozen"))),
        );
        let plugins = list(t, "plugins");
        let mixer = plugins.iter().rposition(|p| text(p, "type") == "Mixer");
        let console = plugins.iter().position(|p| text(p, "id").starts_with("@console:"));
        if let Some(at) = console {
            c.add(
                "console after a mixer",
                usize::from(at > 0 && text(&plugins[at - 1], "type") == "Mixer"),
            );
            c.add(
                "console at the end of a lane",
                usize::from(mixer.is_none() && at + 1 == plugins.len()),
            );
        }
        if flag(t, "highlight")
            && let Some(m) = mixer
        {
            c.add("highlights raising a stage", 1);
            let out = plugins[m]
                .get("params")
                .and_then(|p| p.get("@output-volume"))
                .and_then(Value::as_f64);
            c.add("highlights at the ceiling", usize::from(out == Some(12.0)));
        }
        c.add("plugins", plugins.len());
        count_plugins(plugins, c);
    }
    c.add("plugins", list(doc, "masterPlugins").len());
    count_plugins(list(doc, "masterPlugins"), c);
    c.add("lanes split on halves", lanes.values().filter(|n| **n > 1).count());
}

fn count_plugins(plugins: &[Value], c: &mut Counts) {
    for p in plugins {
        for (key, name) in [
            ("dspGraph", "dspGraph"),
            ("sfzFile", "sfzFile"),
            ("surgePatch", "surgePatch"),
            ("sidechainFrom", "sidechainFrom"),
            ("namFile", "namFile"),
            ("irFile", "irFile"),
            ("patternSource", "pattern plugins"),
        ] {
            c.add(name, usize::from(has(p, key)));
        }
        c.add("rack sounds", list(p, "rack").len());
        c.add(
            "rack taps",
            usize::from(text(p, "type") == "sampler" && has(p, "tapOuts")),
        );
        c.add(
            "kit taps",
            usize::from(text(p, "type") != "sampler" && has(p, "tapOuts")),
        );
        c.add("plugins disabled", usize::from(!flag(p, "enabled")));
        c.add("preset paths", usize::from(!text(p, "path").is_empty()));
        c.add("console halves", usize::from(text(p, "id").starts_with("@console:")));
        let params = p.get("params");
        for (key, name) in [
            ("@mix", "stage mix"),
            ("@input-volume", "stage input volume"),
            ("@output-volume", "stage output volume"),
        ] {
            c.add(name, usize::from(params.is_some_and(|o| o.get(key).is_some())));
        }
        let save = text(p, "presetSave");
        c.add("presetSave under nodes", usize::from(save.contains("/nodes/")));
        c.add(
            "presetSave under vstpresets",
            usize::from(save.contains("/vstpresets/")),
        );
        c.add("presetSave under modules", usize::from(save.contains("/modules/")));
    }
}

/// What the AutoFreeze list did.
pub fn count_freezes(freezes: &Value, doc: &Value, c: &mut Counts) {
    let live: Vec<&str> = list(doc, "tracks")
        .iter()
        .flat_map(|t| list(t, "plugins"))
        .chain(list(doc, "masterPlugins"))
        .map(|p| text(p, "id"))
        .collect();
    for f in freezes.as_array().into_iter().flatten() {
        c.add("automatic freezes", 1);
        let (ready, hosted) = (flag(f, "ready"), flag(f, "hosted"));
        c.add("freezes played", usize::from(ready && !hosted));
        if ready && !hosted {
            // A render stands in for its module: the plugins of the live cut that the document no longer holds.
            let module = text(f, "module");
            let cut = list(&f["cut"], "tracks").iter().flat_map(|t| list(t, "plugins"));
            let gone = cut
                .map(|p| text(p, "id"))
                .filter(|id| id.split_once(':').is_some_and(|(_, m)| m == module) && !live.contains(id))
                .count();
            c.add("freeze plugins removed", gone);
        }
        c.add("freezes hosted", usize::from(hosted));
        c.add("freezes not ready", usize::from(!ready));
    }
}
