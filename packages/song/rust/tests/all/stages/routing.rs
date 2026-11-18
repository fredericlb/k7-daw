// stages/routing.rs — the stage gate's routing half: segments, derived chains, a chain project's nodes, against the TypeScript's dump.
// A reader would get wrong that the strip list is EMPTY (decision 112): each side computes its own lane set, and a difference there is one.

use super::*;
use song::routing::{Step, decode_chain, decode_graph};

/// What the routing half held, counted for the minimums: the effect, not the field.
#[derive(Default)]
pub struct RouteCounts {
    pub chain_projects: usize,
    pub chain_files: usize,
    pub chain_nodes: usize,
    /// The same three, over the staged corpus alone: the negative one cannot make them up.
    pub staged_projects: usize,
    pub staged_files: usize,
    pub staged_nodes: usize,
    pub graph_segments: usize,
    pub subchain_nodes: usize,
    pub picture_lanes: usize,
    pub levels: usize,
    pub bypassed: usize,
    pub bus_only: usize,
    pub port_keys: usize,
    pub port_dests: usize,
    pub tap_feeds: usize,
    pub positions: usize,
    pub hidden: usize,
    pub calls: usize,
    pub parallels: usize,
    pub runs: usize,
    pub negative: usize,
    pub negative_ts: usize,
    pub max_weight: usize,
}

fn len_of(v: &Value) -> usize {
    v.as_array()
        .map(Vec::len)
        .or_else(|| v.as_object().map(Map::len))
        .unwrap_or(0)
}

impl Gate {
    /// The routing of one project: ours against theirs, and what it held.
    pub(super) fn routing(&mut self, name: &str, theirs: &Value, ours: &Value) {
        let (t, o) = (&theirs["routing"], &ours["routing"]);
        if t.is_null() {
            return self
                .failures
                .push(format!("{name} routing: the TypeScript dumped none"));
        }
        if t["chain"] != o["chain_project"] {
            return self
                .failures
                .push(format!("{name} routing: the chain project is told differently"));
        }
        let rust_error = o["files"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|f| f.as_str().is_some_and(|f| f.ends_with("|error")));
        for key in ["threw", "derive_threw"] {
            if t[key].is_string() && !rust_error {
                self.failures.push(format!(
                    "{name} routing: the TypeScript's {key} has no error finding in the Rust"
                ));
            }
        }
        for (at, key) in [
            ("segments", "segments"),
            ("levels", "levels"),
            ("tap_pairs", "tap_pairs"),
        ] {
            self.record(name, &format!("/routing/{at}"), &t[key], &o[key]);
        }
        if !t["derived"].is_null() {
            self.record(name, "/routing/derived", &t["derived"], &o["derived"]);
            self.record(name, "/routing/modules_map", &t["modules_map"], &o["modules_map"]);
        }
        for key in ["bypassed", "bus_only", "lanes"] {
            self.same_set(name, &format!("/routing/{key}"), &t[key], &o[key], |_| false);
        }
        if o["chain_project"].as_bool() == Some(true) {
            for key in ["chains", "hidden"] {
                self.same_set(name, &format!("/routing/{key}"), &t[key], &o[key], |_| false);
            }
            self.record(name, "/routing/nodes", &t["nodes"], &o["nodes"]);
        }
        // Every file the TypeScript names is named here; the Rust adds a layout that does not read (103) and the budget's stop (105).
        let files = |v: &Value| -> Vec<String> {
            v.as_array()
                .into_iter()
                .flatten()
                .filter_map(|x| x.as_str().map(str::to_owned))
                .collect()
        };
        let (tf, of) = (files(&t["files"]), files(&o["files"]));
        for f in tf.iter().filter(|f| !of.contains(f)) {
            self.failures.push(format!(
                "{name} /routing/files: the TypeScript names {f} and the Rust does not"
            ));
        }
        for f in of.iter().filter(|f| !tf.contains(f)) {
            let why = if f.starts_with("layout.k7x|") {
                "a layout.k7x that does not read is an error named layout.k7x here (decision 103)"
            } else if o["capped"] == true {
                "the budget stopped the routing (decision 105)"
            } else {
                self.failures.push(format!(
                    "{name} /routing/files: the Rust names {f} and the TypeScript does not"
                ));
                continue;
            };
            self.set_aside
                .push(json!({"project": name, "pointer": format!("/routing/files/{f}"), "reason": why}));
        }
        self.count_routing(name, o);
    }

    fn count_routing(&mut self, name: &str, o: &Value) {
        let c = &mut self.totals.route;
        if o["chain_project"].as_bool() == Some(true) {
            c.chain_projects += 1;
            c.chain_files += len_of(&o["chains"]);
            c.chain_nodes += len_of(&o["nodes"]);
            if !name.starts_with("negative/") {
                c.staged_projects += 1;
                c.staged_files += len_of(&o["chains"]);
                c.staged_nodes += len_of(&o["nodes"]);
            }
            c.subchain_nodes += o["nodes"]
                .as_object()
                .into_iter()
                .flatten()
                .filter(|(_, n)| n["sub"].is_string())
                .count();
            c.picture_lanes += o["nodes"]
                .as_object()
                .into_iter()
                .flatten()
                .filter(|(_, n)| n["picture"] == true)
                .count();
        } else {
            c.graph_segments += 1;
        }
        // A level applied is an out whose gain it changed, not an entry of the table.
        c.levels += o["derived"]
            .as_object()
            .into_iter()
            .flat_map(Map::values)
            .flat_map(|chain| chain["out"].as_array().into_iter().flatten())
            .filter(|out| out["gain"].as_f64().is_some_and(|g| (g - 1.0).abs() > f64::EPSILON))
            .count();
        c.bypassed += len_of(&o["bypassed"]);
        c.bus_only += len_of(&o["bus_only"]);
        let modules = o["segments"]["modules"].as_object();
        c.port_keys += modules.map_or(0, |m| m.keys().filter(|k| k.contains('#')).count());
        c.port_dests += modules.map_or(0, |m| {
            m.values()
                .flat_map(|v| v.as_array().into_iter().flatten())
                .filter(|d| d.as_str().is_some_and(|d| d.contains('#')))
                .count()
        });
        c.tap_feeds += len_of(&o["segments"]["tap_feeds"]);
        c.positions += len_of(&o["segments"]["positions"]);
        c.hidden += len_of(&o["hidden"]);
        c.max_weight = c.max_weight.max(
            o["weight"]
                .as_u64()
                .map_or(0, |w| usize::try_from(w).unwrap_or(usize::MAX)),
        );
    }

    /// The kinds the golden corpus lacks, counted over a negative case's own files: calls, parallel steps and `runs`.
    pub(super) fn count_negative_routing(&mut self, case: &Case) {
        let c = &mut self.totals.route;
        c.negative += 1;
        c.negative_ts += usize::from(case.ts);
        for (rel, bytes) in &case.files {
            let Ok(text) = std::str::from_utf8(bytes) else { continue };
            if rel.starts_with("chains/")
                && let Ok(chain) = decode_chain(rel, text, song::PathRule::HOST)
            {
                for (_, steps) in chain.step_lists() {
                    c.calls += steps.iter().filter(|s| s.is_call()).count();
                    c.parallels += steps.iter().filter(|s| matches!(s, Step::Parallel(_))).count();
                }
            }
            if rel == "graph.k7x" && text.contains("runs(") && decode_graph(rel, text).is_ok() {
                c.runs += 1;
            }
        }
    }
}
