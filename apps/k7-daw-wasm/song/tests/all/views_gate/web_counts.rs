// views_gate/web_counts.rs — what the compile-based views showed, as counts, and the floors the gate holds them to.
// A reader would get wrong that a count is a field of an answer: each is an effect read off the Rust's JSON, never a thing it reports.

use serde_json::Value;

/// What the compile-based views showed: each count is an effect, never a field.
#[derive(Clone, Default)]
pub struct WebCounts {
    pub projects: usize,
    pub tracks: usize,
    pub clips: usize,
    pub measured: usize,
    pub natural: usize,
    pub midi: usize,
    pub notes: usize,
    pub velocities: usize,
    pub plugins: usize,
    pub sfz: usize,
    pub mixers: usize,
    pub groups: usize,
    pub grouped: usize,
    pub generated: usize,
    pub highlights: usize,
    pub markers: usize,
    pub warped: usize,
    pub warp_off: usize,
    pub disabled: usize,
    pub loops: usize,
    pub fades_in: usize,
    pub fades_out: usize,
    pub events: usize,
    pub comps: usize,
    pub nests: usize,
    pub timelines: usize,
    pub legacy: usize,
    pub frozen: usize,
    pub frozen_material: usize,
    pub crossfades: usize,
    pub failed: usize,
    pub refusals_by_files: usize,
    pub refusals_by_text: usize,
    pub diagnostics: usize,
    pub format_too_new: usize,
    pub not_supported: usize,
    pub unknown_plugin: usize,
    pub dead_state: usize,
    pub dead_graph: usize,
    pub dead_fields: usize,
    pub jsdsp: usize,
    pub capped: usize,
}

impl WebCounts {
    fn clip(&mut self, c: &Value) {
        self.clips += 1;
        self.midi += usize::from(c["type"] == "midi");
        self.warped += usize::from(c.get("warped").is_some());
        self.warp_off += usize::from(c.get("warpOff").is_some());
        self.disabled += usize::from(c.get("disabled").is_some());
        self.loops += usize::from(c["loopEnd"].as_f64().unwrap_or(0.0) > 0.0);
        self.fades_in += usize::from(c.get("fadeIn").is_some());
        self.fades_out += usize::from(c.get("fadeOut").is_some());
        self.events += usize::from(c.get("event").is_some());
        self.comps += usize::from(c.get("pieces").is_some());
        self.nests += usize::from(c.get("timeline").is_some());
        self.frozen += usize::from(c.get("frozen").is_some());
        self.crossfades += usize::from(c.get("crossfade").is_some());
        for n in c["notes"].as_array().into_iter().flatten() {
            self.notes += 1;
            self.velocities += usize::from(n.get("vel").is_some());
        }
        // A piece of a comp and a clip inside a placed timeline are clips too.
        let inner = c["pieces"]
            .as_array()
            .into_iter()
            .chain(c["timeline"]["clips"].as_array());
        for i in inner.flatten() {
            self.clip(i);
        }
    }

    fn track(&mut self, t: &Value) {
        self.tracks += 1;
        self.mixers += usize::from(t.get("mixer").is_some());
        self.grouped += usize::from(t["group"].as_str().is_some_and(|g| !g.is_empty()));
        self.generated += usize::from(t["generated"] == true);
        self.highlights += usize::from(t.get("highlight").is_some());
        self.frozen_material += usize::from(t.get("frozenMaterial").is_some());
        for p in t["plugins"].as_array().into_iter().flatten() {
            self.plugins += 1;
            self.sfz += usize::from(p.get("sfzFile").is_some());
        }
        for c in t["clips"].as_array().into_iter().flatten() {
            self.clip(c);
        }
    }

    pub fn add(&mut self, ours: &Value) {
        let w = &ours["web"];
        if !w.is_null() {
            self.projects += 1;
            self.natural += ours["natural"].as_u64().unwrap_or(0) as usize;
            self.measured += ours["measured"].as_u64().unwrap_or(0) as usize;
            self.groups += w["groups"].as_array().map_or(0, Vec::len);
            self.markers += w["markers"].as_array().map_or(0, Vec::len);
            for t in w["tracks"].as_array().into_iter().flatten() {
                self.track(t);
            }
            for t in w["timelines"].as_array().into_iter().flatten() {
                self.timelines += 1;
                for l in t["lanes"].as_array().into_iter().flatten() {
                    self.track(l);
                }
            }
        }
        // A folder that is no project has the one synthetic refusal; it is counted as a folder, never as a refusal or a diagnostic.
        let legacy = !ours["legacy"].is_null();
        self.legacy += usize::from(legacy);
        self.format_too_new += usize::from(!ours["errors"]["formatTooNew"].is_null());
        if legacy {
            return;
        }
        self.failed += usize::from(!ours["failed"].is_null());
        for d in ours["errors"]["diagnostics"].as_array().into_iter().flatten() {
            self.diagnostics += 1;
            let m = d["message"].as_str().unwrap_or("");
            // A counter of the gate reads the sentences this crate writes; the crate compares a message only to drop its own repeat.
            self.not_supported += usize::from(m.contains("is not supported"));
            self.unknown_plugin += usize::from(m.contains("which no engine loads"));
            self.dead_state += usize::from(m.contains("is plain plugin state"));
            self.dead_graph += usize::from(m.contains("to automate (it has"));
            self.dead_fields += usize::from(m.contains("is no longer read"));
            self.jsdsp += usize::from(m.contains("its DSP does not build"));
            self.capped += usize::from(m.contains("more findings are not listed"));
        }
    }

    /// What the golden corpus alone must show of the web project and the errors (the work order's table).
    pub fn golden_floors(&self) -> Vec<(&'static str, usize, usize)> {
        vec![
            ("golden web projects", self.projects, 170),
            ("golden web tracks", self.tracks, 340),
            ("golden web clips", self.clips, 340),
            ("golden clips with a natural length", self.natural, 250),
            ("golden midi clips", self.midi, 70),
            ("golden notes", self.notes, 280),
            ("golden notes with vel", self.velocities, 70),
            ("golden plugins", self.plugins, 300),
            ("golden plugins with sfz", self.sfz, 12),
            ("golden mixers", self.mixers, 2),
            ("golden groups", self.groups, 4),
            ("golden grouped tracks", self.grouped, 8),
            ("golden generated lanes", self.generated, 6),
            ("golden highlights", self.highlights, 1),
            ("golden markers", self.markers, 1),
            ("golden warped clips", self.warped, 1),
            ("golden warpOff clips", self.warp_off, 1),
            ("golden disabled clips", self.disabled, 1),
            ("golden looped clips", self.loops, 1),
            ("golden fades in", self.fades_in, 1),
            ("golden fades out", self.fades_out, 1),
            ("golden clips with an event", self.events, 3),
            ("golden failed compiles", self.failed, 2),
            ("golden refusals compared by the throw's text", self.refusals_by_text, 2),
            ("golden error diagnostics", self.diagnostics, 1),
        ]
    }

    /// Every floor: below the first green run, never zero.
    pub fn floors(&self) -> Vec<(&'static str, usize, usize)> {
        vec![
            ("web projects", self.projects, 170),
            ("web tracks", self.tracks, 340),
            ("web clips", self.clips, 340),
            ("web lengths measured", self.measured, 100),
            ("web clips with a natural length", self.natural, 250),
            ("web midi clips", self.midi, 70),
            ("web notes", self.notes, 280),
            ("web notes with vel", self.velocities, 70),
            ("web plugins", self.plugins, 300),
            ("web plugins with sfz", self.sfz, 12),
            ("web mixers", self.mixers, 2),
            ("web groups", self.groups, 4),
            ("web grouped tracks", self.grouped, 8),
            ("web generated lanes", self.generated, 6),
            ("web highlights", self.highlights, 1),
            ("web markers", self.markers, 1),
            ("web warped clips", self.warped, 1),
            ("web warpOff clips", self.warp_off, 1),
            ("web disabled clips", self.disabled, 1),
            ("web looped clips", self.loops, 1),
            ("web fades in", self.fades_in, 1),
            ("web fades out", self.fades_out, 1),
            ("web clips with an event", self.events, 3),
            ("web comps", self.comps, 1),
            ("web nests", self.nests, 1),
            ("web timelines", self.timelines, 1),
            ("web frozen clips", self.frozen, 1),
            ("web frozen material", self.frozen_material, 1),
            ("web crossfades", self.crossfades, 1),
            ("failed compiles", self.failed, 10),
            (
                "refusals compared by the files the TypeScript names",
                self.refusals_by_files,
                3,
            ),
            ("refusals compared by the throw's text", self.refusals_by_text, 6),
            ("error diagnostics", self.diagnostics, 40),
            ("formatTooNew", self.format_too_new, 1),
            ("not-supported findings", self.not_supported, 1),
            ("unknown-plugin findings", self.unknown_plugin, 1),
            ("dead automations of a plain-state control", self.dead_state, 1),
            ("dead automations of an undeclared name", self.dead_graph, 1),
            ("dead project fields", self.dead_fields, 1),
            ("jsdsp findings", self.jsdsp, 1),
        ]
    }

    pub fn report(&self) -> String {
        format!(
            "  web: {} projects, {} tracks ({} mixers, {} grouped, {} generated, {} highlighted, {} plugins, {} sfz, {} frozen material), {} clips \
             ({} midi, {} measured lengths, {} natural, {} warped, {} warpOff, {} disabled, {} looped, {} fades in, {} out, {} events, {} comps, {} nests, \
             {} frozen, {} crossfades), {} notes ({} with vel), {} groups, {} markers, {} timelines, {} folders that are no project\n  \
             errors: {} failed compiles ({} compared by files, {} by the throw's text), {} diagnostics ({} formatTooNew, {} not supported, {} unknown plugin, {} dead state, {} dead undeclared, \
             {} dead fields, {} jsdsp, {} capped)",
            self.projects,
            self.tracks,
            self.mixers,
            self.grouped,
            self.generated,
            self.highlights,
            self.plugins,
            self.sfz,
            self.frozen_material,
            self.clips,
            self.midi,
            self.measured,
            self.natural,
            self.warped,
            self.warp_off,
            self.disabled,
            self.loops,
            self.fades_in,
            self.fades_out,
            self.events,
            self.comps,
            self.nests,
            self.frozen,
            self.crossfades,
            self.notes,
            self.velocities,
            self.groups,
            self.markers,
            self.timelines,
            self.legacy,
            self.failed,
            self.refusals_by_files,
            self.refusals_by_text,
            self.diagnostics,
            self.format_too_new,
            self.not_supported,
            self.unknown_plugin,
            self.dead_state,
            self.dead_graph,
            self.dead_fields,
            self.jsdsp,
            self.capped
        )
    }
}
