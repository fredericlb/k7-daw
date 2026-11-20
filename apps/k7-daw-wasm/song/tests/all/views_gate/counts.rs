// views_gate/counts.rs — what the gate saw: each count is an EFFECT (a row listed, a fold made), and each floor sits below the first green run.
// A reader would get wrong that a kind absent from the golden corpus is counted over the negative corpus and the sweeps, loudly.

use super::WebCounts;
use serde_json::Value;

#[derive(Clone, Default)]
pub struct Counts {
    pub projects: usize,
    pub negative: usize,
    pub negative_ts: usize,
    pub ts_threw: usize,
    pub bundles: usize,
    pub catalogue: usize,
    pub with_path: usize,
    pub without_path: usize,
    pub with_panel: usize,
    pub extends: usize,
    pub with_plugin: usize,
    pub nodes_above_zero: usize,
    pub base: usize,
    pub base_panel: usize,
    pub surfaces: usize,
    pub by_type: [usize; 4],
    pub mapped: usize,
    pub entries: usize,
    pub null_entries: usize,
    pub surface_findings: usize,
    pub playlists: usize,
    pub stems: usize,
    pub issue_warnings: usize,
    pub io: usize,
    pub io_in: usize,
    pub io_out: usize,
    pub io_own: usize,
    pub io_placed: usize,
    pub io_channels: usize,
    pub io_label: usize,
    pub io_unknown: usize,
    pub hint_lists: [usize; 10],
    pub params: usize,
    pub meta: usize,
    pub paths: usize,
    pub media_folders: usize,
    pub media_files: usize,
    pub racks: usize,
    pub rack_symbols: usize,
    pub slice_racks: usize,
    pub video_tracks: usize,
    pub channels: usize,
    pub favourites: usize,
    pub hidden: usize,
    pub module_assets: usize,
    pub plugin_names: usize,
    pub capped: usize,
    pub max_spent: [usize; 4],
    pub lattice: usize,
    pub wavs: usize,
    pub wavs_measured: usize,
    pub sweep_lists: usize,
    pub sweep_natural: usize,
    pub sweep_folded: usize,
    pub comp_pieces: usize,
    pub nests: usize,
    pub midi_hosts: usize,
    pub crossfades: usize,
    pub frozen: usize,
    pub source_from: usize,
    pub repeat: usize,
    pub skip: usize,
    pub unreadable: usize,
    pub web: WebCounts,
}

fn len(v: &Value) -> usize {
    v.as_array().map_or(0, Vec::len)
}

fn size(v: &Value) -> usize {
    v.as_object().map_or(0, serde_json::Map::len)
}

impl Counts {
    /// Counts what one project's views gave.
    pub fn add_views(&mut self, ours: &Value) {
        for m in ours["catalogue"].as_array().into_iter().flatten() {
            self.catalogue += 1;
            if m.get("path").is_some() {
                self.with_path += 1;
            } else {
                self.without_path += 1;
            }
            self.with_panel += usize::from(m.get("panel").is_some());
            self.extends += usize::from(m.get("extends").is_some());
            self.with_plugin += usize::from(m.get("plugin").is_some());
            self.nodes_above_zero += usize::from(m["nodes"].as_u64().unwrap_or(0) > 0);
        }
        for b in ours["base"].as_array().into_iter().flatten() {
            self.base += 1;
            self.base_panel += usize::from(b.get("panel").is_some());
        }
        let s = &ours["surfaces"];
        for p in s["surfaces"].as_array().into_iter().flatten() {
            self.surfaces += 1;
            let kinds = ["timeline", "viewport", "turntables", "mastering"];
            if let Some(i) = kinds.iter().position(|t| p["surface"]["type"] == *t) {
                self.by_type[i] += 1;
            }
            self.playlists += usize::from(p["surface"].get("playlist").is_some());
            self.stems += usize::from(p["surface"].get("stems").is_some());
            if let Some(m) = p["surface"]["mapping"].as_object() {
                self.mapped += 1;
                self.entries += m.len();
                self.null_entries += m.values().filter(|v| v.is_null()).count();
            }
        }
        self.surface_findings += len(&s["diagnostics"]);
        self.issue_warnings += ours["issues"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|i| i["severity"] == "warning" && i["file"] == "surfaces/")
            .count();
        for p in ours["io"].as_array().into_iter().flatten() {
            self.io += 1;
            self.io_in += usize::from(p["dir"] == "in");
            self.io_out += usize::from(p["dir"] == "out");
            self.io_own += usize::from(p["namespace"] == "");
            self.io_placed += usize::from(p["name"].as_str().is_some_and(|n| n.contains(" · step ")));
            self.io_channels += usize::from(len(&p["channels"]) > 0);
            self.io_label += usize::from(p.get("label").is_some() && len(&p["channels"]) > 0);
            self.io_unknown += usize::from(p.get("unknownLabel").is_some());
        }
        let h = &ours["hints"];
        for (i, k) in [
            "baseModules",
            "modules",
            "ioSinks",
            "tracks",
            "clips",
            "audioClips",
            "videoClips",
            "sequences",
            "chains",
            "curves",
        ]
        .iter()
        .enumerate()
        {
            self.hint_lists[i] += len(&h[*k]);
        }
        self.params += size(&h["paramsByBase"]);
        self.meta += size(&h["paramMetaByBase"]);
        self.paths += size(&h["paths"]);
        self.media_folders += size(&h["media"]);
        self.media_files += h["media"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(_, v)| len(v))
            .sum::<usize>();
        for r in h["racks"].as_object().into_iter().flat_map(|m| m.values()) {
            self.racks += 1;
            self.rack_symbols += len(&r["symbols"]);
            self.slice_racks += usize::from(r["rackType"] == "slice");
        }
        self.video_tracks += len(&h["videoTracks"]);
        self.channels += len(&h["channels"]);
        self.module_assets += size(&h["moduleAssets"]);
        self.plugin_names += size(&h["pluginNames"]);
        self.web.add(ours);
        let st = &ours["stats"];
        self.favourites += st["favourites"].as_u64().unwrap_or(0) as usize;
        self.hidden += st["hidden"].as_u64().unwrap_or(0) as usize;
        self.capped += usize::from(st["capped"] == true);
        for (i, k) in ["hints", "io", "catalogue", "base"].iter().enumerate() {
            let spent = st["spent"][*k].as_u64().unwrap_or(0) as usize;
            self.max_spent[i] = self.max_spent[i].max(spent);
        }
    }

    /// Every floor of the gate as (what, seen, least): below the first green run, never zero.
    pub fn floors(&self) -> Vec<(&'static str, usize, usize)> {
        let h = &self.hint_lists;
        let mut all = vec![
            ("catalogue rows", self.catalogue, 60_000),
            ("catalogue rows with a path", self.with_path, 22_000),
            ("catalogue rows without a path", self.without_path, 38_000),
            ("catalogue rows with a panel", self.with_panel, 15_000),
            ("catalogue rows that extend", self.extends, 13_000),
            ("catalogue rows with a plugin", self.with_plugin, 38_000),
            ("catalogue rows counting nodes", self.nodes_above_zero, 1),
            ("base modules", self.base, 55_000),
            ("base modules with a panel", self.base_panel, 12_000),
            ("surfaces", self.surfaces, 170),
            ("timeline surfaces", self.by_type[0], 160),
            ("viewport surfaces", self.by_type[1], 5),
            ("turntables surfaces", self.by_type[2], 1),
            ("mastering surfaces", self.by_type[3], 1),
            ("mapped surfaces", self.mapped, 1),
            ("mapping entries", self.entries, 30),
            ("null mapping entries", self.null_entries, 1),
            ("surface findings", self.surface_findings, 1),
            ("playlists", self.playlists, 1),
            ("stems", self.stems, 1),
            ("surface issue warnings", self.issue_warnings, 1),
            ("io ports", self.io, 500),
            ("io inputs", self.io_in, 165),
            ("io outputs", self.io_out, 330),
            ("io ports of the project's own", self.io_own, 1),
            ("io ports of a placed node", self.io_placed, 1),
            ("io ports with channels", self.io_channels, 1),
            ("io ports with a resolved label", self.io_label, 1),
            ("io ports with an unknown label", self.io_unknown, 1),
            ("hints base refs", h[0], 55_000),
            ("hints modules", h[1], 20_000),
            ("hints sinks", h[2], 1_300),
            ("hints tracks", h[3], 350),
            ("hints clips", h[4], 120),
            ("hints audio clips", h[5], 120),
            ("hints video clips", h[6], 3),
            ("hints sequences", h[7], 55),
            ("hints chains", h[8], 500),
            ("hints curves", h[9], 3),
            ("hints paramsByBase", self.params, 35_000),
            ("hints paramMetaByBase", self.meta, 35_000),
            ("hints paths", self.paths, 850),
            ("hints media folders", self.media_folders, 100),
            ("hints media files", self.media_files, 120),
            ("hints racks", self.racks, 2),
            ("hints rack symbols", self.rack_symbols, 6),
            ("hints slice racks", self.slice_racks, 1),
            ("hints video tracks", self.video_tracks, 8),
            ("hints channels", self.channels, 1),
            ("hints favourites that floated up", self.favourites, 1),
            ("hints hidden modules dropped", self.hidden, 1),
            ("hints modules with assets", self.module_assets, 1),
            ("hints plugin names", self.plugin_names, 1),
            ("lattice entries", self.lattice, 40_000),
            ("wavs", self.wavs, 2000),
            ("wavs measured", self.wavs_measured, 200),
            ("clip lists", self.sweep_lists, 1500),
            ("clip lists with a natural length", self.sweep_natural, 300),
            ("clip lists folded", self.sweep_folded, 200),
            ("comp pieces", self.comp_pieces, 20),
            ("nests", self.nests, 20),
            ("nest hosts of midi", self.midi_hosts, 20),
            ("crossfades", self.crossfades, 20),
            ("frozen clips", self.frozen, 20),
            ("clips with sourceFrom", self.source_from, 20),
            ("notes with repeat", self.repeat, 20),
            ("notes with skip", self.skip, 20),
            ("unreadable lengths", self.unreadable, 20),
            ("bundle replays", self.bundles, 170),
            ("the largest hints spend", self.max_spent[0], 1),
            ("the largest io spend", self.max_spent[1], 1),
            ("the largest catalogue spend", self.max_spent[2], 1),
            ("the largest base spend", self.max_spent[3], 1),
        ];
        all.extend(self.web.floors());
        all
    }

    pub fn report(&self) -> String {
        format!(
            "views gate: {} projects, {} negative cases ({} with the TypeScript), {} TypeScript throws, {} bundle replays\n  \
             catalogue {} rows ({} with a path, {} without, {} panels, {} extends, {} plugins, {} with nodes); base {} ({} panels)\n  \
             surfaces {} (timeline {}, viewport {}, turntables {}, mastering {}; {} mapped, {} entries, {} null; {} playlists, {} stems; {} findings), {} issue warnings\n  \
             io {} ports ({} in, {} out, {} own, {} placed, {} with channels, {} labelled, {} unknown)\n  \
             hints: base {}, modules {}, sinks {}, tracks {}, clips {}, audio {}, video {}, sequences {}, chains {}, curves {}; params {}, meta {}, paths {}, \
             media {} folders / {} files, racks {} ({} symbols, {} slice), video tracks {}, channels {}, favourites {}, hidden {}, assets {}, plugin names {}\n  \
             spends (max): hints {}, io {}, catalogue {}, base {}; {} capped\n  \
             lattice {}; wavs {} ({} measured); clip lists {} ({} natural, {} folded; {} comp pieces, {} nests, {} midi hosts, {} crossfades, \
             {} frozen, {} sourceFrom, {} repeat, {} skip, {} unreadable)\n{}",
            self.projects,
            self.negative,
            self.negative_ts,
            self.ts_threw,
            self.bundles,
            self.catalogue,
            self.with_path,
            self.without_path,
            self.with_panel,
            self.extends,
            self.with_plugin,
            self.nodes_above_zero,
            self.base,
            self.base_panel,
            self.surfaces,
            self.by_type[0],
            self.by_type[1],
            self.by_type[2],
            self.by_type[3],
            self.mapped,
            self.entries,
            self.null_entries,
            self.playlists,
            self.stems,
            self.surface_findings,
            self.issue_warnings,
            self.io,
            self.io_in,
            self.io_out,
            self.io_own,
            self.io_placed,
            self.io_channels,
            self.io_label,
            self.io_unknown,
            self.hint_lists[0],
            self.hint_lists[1],
            self.hint_lists[2],
            self.hint_lists[3],
            self.hint_lists[4],
            self.hint_lists[5],
            self.hint_lists[6],
            self.hint_lists[7],
            self.hint_lists[8],
            self.hint_lists[9],
            self.params,
            self.meta,
            self.paths,
            self.media_folders,
            self.media_files,
            self.racks,
            self.rack_symbols,
            self.slice_racks,
            self.video_tracks,
            self.channels,
            self.favourites,
            self.hidden,
            self.module_assets,
            self.plugin_names,
            self.max_spent[0],
            self.max_spent[1],
            self.max_spent[2],
            self.max_spent[3],
            self.capped,
            self.lattice,
            self.wavs,
            self.wavs_measured,
            self.sweep_lists,
            self.sweep_natural,
            self.sweep_folded,
            self.comp_pieces,
            self.nests,
            self.midi_hosts,
            self.crossfades,
            self.frozen,
            self.source_from,
            self.repeat,
            self.skip,
            self.unreadable,
            self.web.report()
        )
    }
}
