// stages/modules.rs — the stage gate's module half: nodes, the vocabulary, the findings, compared with the TypeScript's dump.
// A reader would get wrong what is allowed: an extra finding file only where a decision names it, and never a missing one.

use super::*;
use song::{Roots, Source};
use std::collections::HashSet;
use support::{dump_modules, strip_checked, strip_module};

const FAMILIES: [&str; 8] = [
    "modules",
    "chains",
    "assemblies",
    "console",
    "samples",
    "irs",
    "wavetables",
    "models",
];

impl Gate {
    /// Each library family folder that is a link out of the mounts: a Recorder is refused it, and the bundle says so (743).
    fn linked_families(&mut self, name: &str, roots: &Roots, mounts: &[String], files: &[String]) {
        for lib in &roots.libraries {
            for family in FAMILIES {
                let dir = format!("{}/{family}", lib.root);
                let is_link = fs::symlink_metadata(&dir).is_ok_and(|m| m.file_type().is_symlink());
                if !is_link || !self.totals.linked_seen.insert(dir.clone()) {
                    continue;
                }
                let disk = DiskSource::with_files(mounts, files).expect("a source");
                let rec = Recorder::new(&disk);
                let on_disk = rec.list(&dir).map(|l| l.len());
                let bundle = rec.into_bundle_with_files(mounts, files);
                let refused = bundle.refused.contains(&dir);
                let again = MemSource::from_bundle(bundle).map(|m| m.list(&dir).map(|l| l.len()));
                match (&on_disk, again) {
                    (Err(song::SourceError::Outside), Ok(r)) if r == on_disk && refused => self.totals.linked_out += 1,
                    (Err(song::SourceError::Outside), other) => self.failures.push(format!(
                        "{name} {dir}: refused on disk, replayed as {other:?}, in refused: {refused}"
                    )),
                    // A link whose target is mounted lists as it is; the replay must list the same.
                    (_, Ok(r)) if r == on_disk => {}
                    (_, other) => self.failures.push(format!(
                        "{name} {dir}: listed {on_disk:?} on disk and {other:?} replayed"
                    )),
                }
            }
        }
    }

    /// The module half of one project: nodes, the vocabulary its roots make, what the walk found; through a disk and a bundle.
    pub(super) fn modules(&mut self, name: &str, theirs: &Value, remember: bool) {
        let Some(m) = theirs.get("modules") else { return };
        let roots: Roots = serde_json::from_value(m["roots"].clone()).expect("the dump's roots");
        let (mounts, files) = roots.mounts().expect("mounts");
        let disk = DiskSource::with_files(&mounts, &files).expect("a source");
        let rec = Recorder::new(&disk);
        let got = match dump_modules(&rec, &mut self.cache, &roots) {
            Ok(g) => g,
            Err(e) => return self.failures.push(format!("{name} modules: {e}")),
        };
        // A file held by its length (decision 14) cannot be decoded from a bundle: those cases are `mem: false`.
        if !remember {
            self.totals.module_unbundled += 1;
        } else {
            match MemSource::from_bundle(rec.into_bundle_with_files(&mounts, &files)) {
                Ok(mem) => match dump_modules(&mem, &mut Cache::new(), &roots) {
                    Ok(again) if again.json == got.json => self.totals.module_remembered += 1,
                    Ok(_) => self
                        .failures
                        .push(format!("{name} modules: the bundle read differently from the disk")),
                    Err(e) => self
                        .failures
                        .push(format!("{name} modules: the bundle was not enough: {e}")),
                },
                Err(e) => self.failures.push(format!("{name} modules: {e}")),
            }
        }
        self.linked_families(name, &roots, &mounts, &files);
        let c = got.counts;
        let t = &mut self.totals.mods;
        t.nodes += c.nodes;
        t.library_findings += c.library_findings;
        t.scan_plugins += c.scan_plugins;
        for (r, module) in got.json["vocab"]["modules"].as_object().into_iter().flatten() {
            let Some(folder) = module["folderAbs"].as_str() else {
                continue;
            };
            let id = format!("{r}|{folder}");
            let seen = &mut self.totals.seen;
            if module["node"].is_string() {
                seen.nodes.insert(id);
            } else if module["engineOf"].is_string() {
                seen.engines.insert(id);
            } else if r.matches('/').count() > 2 {
                seen.category.insert(id);
            } else {
                seen.library.insert(id);
            }
        }
        self.totals.module_projects += 1;
        let (ours, spec) = (&got.json, strip_checked(&m["strip"]));
        let strip = |v: &Value, _node: bool| {
            let mut out = v.clone();
            for module in out.as_object_mut().into_iter().flat_map(|o| o.values_mut()) {
                strip_module(module, &spec);
            }
            out
        };
        self.resolved(name, &m["resolved"], &ours["resolved"]);
        count_resolved(&mut self.totals.res, name, &got.json);
        self.routing(name, m, ours);
        if m["nodes"]["chain"].as_bool().unwrap_or(false) != got.chain {
            self.failures
                .push(format!("{name} nodes: the chain project is told differently"));
        } else {
            let ts_files_named = m["nodes"]["files"].as_array().is_some_and(|a| !a.is_empty());
            let (o, t) = (
                strip(&ours["nodes"]["values"], true),
                strip(&m["nodes"]["values"], true),
            );
            self.record(name, "/modules/nodes", &t, &o);
            self.subset(
                name,
                "/modules/nodes/files",
                &m["nodes"]["files"],
                &ours["nodes"]["files"],
                // The plugin rule is raised by the Rust beside decode findings; the TypeScript throws it only when none failed.
                move |_| ts_files_named,
            );
        }
        let (o, t) = (
            strip(&ours["vocab"]["modules"], false),
            strip(&m["vocab"]["modules"], false),
        );
        self.record(name, "/modules/vocab", &t, &o);
        // The TypeScript's library node file that did not read is silent; the ruling is that it warns (decision 38).
        self.subset(
            name,
            "/modules/vocab/findings",
            &m["vocab"]["findings"],
            &ours["vocab"]["findings"],
            |f| !f.ends_with("/module.k7x"),
        );
        // The severity of each finding is compared with its file.
        self.subset(
            name,
            "/modules/vocab/findings_severity",
            &m["vocab"]["findings_severity"],
            &ours["vocab"]["findings_severity"],
            |f| !f.split('|').next().unwrap_or("").ends_with("/module.k7x"),
        );
        // The refs a new module may extend: a set, whose dot entries are decision 20's.
        self.same_set(
            name,
            "/modules/vocab/base",
            &m["vocab"]["base"],
            &ours["vocab"]["base"],
            |r| r.split('/').any(|s| s.starts_with('.')),
        );
        self.record(
            name,
            "/modules/vocab/siblings",
            &m["vocab"]["siblings"],
            &ours["vocab"]["siblings"],
        );
    }

    /// Two lists equal as sets, but for what the TypeScript has that the Rust leaves out by `allowed`.
    pub(super) fn same_set(
        &mut self,
        name: &str,
        at: &str,
        theirs: &Value,
        ours: &Value,
        allowed: impl Fn(&str) -> bool,
    ) {
        let list = |v: &Value| -> Vec<String> {
            v.as_array()
                .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
                .unwrap_or_default()
        };
        let (t, o) = (list(theirs), list(ours));
        for r in t.iter().filter(|r| !o.contains(r)) {
            if allowed(r) {
                self.set_aside.push(json!({"project": name, "pointer": format!("{at}/{r}"), "reason": "a dot folder is skipped by the walk (decision 20)"}));
            } else {
                self.failures
                    .push(format!("{name} {at}: the TypeScript names {r} and the Rust does not"));
            }
        }
        for r in o.iter().filter(|r| !t.contains(r)) {
            self.failures
                .push(format!("{name} {at}: the Rust names {r} and the TypeScript does not"));
        }
    }

    /// Every file the TypeScript names is named here; each extra must pass `extra` and is set aside with its reason.
    fn subset(&mut self, name: &str, at: &str, theirs: &Value, ours: &Value, extra: impl Fn(&str) -> bool) {
        let list = |v: &Value| -> Vec<String> {
            v.as_array()
                .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
                .unwrap_or_default()
        };
        let (t, o) = (list(theirs), list(ours));
        for f in t.iter().filter(|f| !o.contains(f)) {
            self.failures
                .push(format!("{name} {at}: the TypeScript names {f} and the Rust does not"));
        }
        for f in o.iter().filter(|f| !t.contains(f)) {
            if extra(f) {
                let panel = ["/ui.k7x", "/controller.k7x", "/widgets.js"]
                    .iter()
                    .any(|n| f.split('|').next().unwrap_or("").ends_with(n));
                let reason = if panel {
                    "a panel, map or script that is refused is a warning here and an unread issue in the TypeScript: decisions 61 and 62"
                } else {
                    "a file the Rust names beyond the TypeScript's: decision 38 (a library node file warns) or 23 (the plugin rule beside decode findings)"
                };
                self.set_aside
                    .push(json!({"project": name, "pointer": format!("{at}/{f}"), "reason": reason}));
            } else {
                self.failures
                    .push(format!("{name} {at}: the Rust names {f} and the TypeScript does not"));
            }
        }
    }
}

/// What the resolved half held, counted for the minimums: each kind by what it is, so a library shared by projects counts once.
#[derive(Default)]
pub struct ResCounts {
    pub panels: HashSet<String>,
    pub scripts: HashSet<String>,
    pub maps: HashSet<String>,
    pub presets: HashSet<String>,
    pub clips: HashSet<String>,
    pub files: HashSet<String>,
    pub project_samples: HashSet<String>,
    pub folds: HashSet<String>,
    pub alternatives: HashSet<String>,
    pub engines: HashSet<String>,
    pub max_weight: usize,
    pub not_supported: usize,
    pub unknown_plugin: usize,
}

fn count_resolved(c: &mut ResCounts, project: &str, dump: &Value) {
    for r in dump["resolve"]["panels"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        c.panels.insert(r.to_owned());
    }
    for r in dump["resolve"]["maps"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        c.maps.insert(r.to_owned());
    }
    c.max_weight = dump["resolve"]["weights"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_u64)
        .fold(c.max_weight, |a, w| a.max(usize::try_from(w).unwrap_or(usize::MAX)));
    // A pick other than the first, and an `extends` that resolves, told by the id the dump gives.
    let told = |key: &str| -> HashSet<String> {
        let id = |s: &str| {
            s.strip_prefix("n:")
                .map_or_else(|| s.to_owned(), |k| format!("n:{project}:{k}"))
        };
        dump["resolve"][key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(id)
            .collect()
    };
    c.alternatives.extend(told("picked"));
    c.folds.extend(told("folds"));
    // A vocabulary module is told by its ref, a node by its project and name.
    let views = [("v", &dump["resolved"]["vocab"]), ("n", &dump["resolved"]["nodes"])];
    for (kind, part) in views {
        for (key, m) in part.as_object().into_iter().flatten() {
            let id = if kind == "v" {
                format!("v:{key}")
            } else {
                format!("n:{project}:{key}")
            };
            let note = |set: &mut HashSet<String>, yes: bool| {
                if yes {
                    set.insert(id.clone());
                }
            };
            note(&mut c.scripts, m["ui"]["script"].is_string());
            note(&mut c.presets, m["preset"].is_string());
            note(&mut c.engines, m["engineRefs"].is_object() && m["engine"].is_string());
            note(&mut c.files, ["model", "onnx", "ir"].iter().any(|k| m[*k].is_string()));
            let symbols = m["rack"]["symbols"].as_object();
            note(
                &mut c.clips,
                symbols.is_some_and(|s| s.values().any(|v| v["clip"].is_string())),
            );
            let project_sample =
                |v: &Value| v["sample"].as_str().is_some_and(|s| s.starts_with("@project/")) && v["clip"].is_string();
            note(
                &mut c.project_samples,
                symbols.is_some_and(|s| s.values().any(project_sample)),
            );
        }
    }
    for f in dump["resolved"]["plugin"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        c.not_supported += usize::from(f.ends_with("|warning"));
        c.unknown_plugin += usize::from(f.ends_with("|error"));
    }
}

impl Gate {
    /// The resolved nodes and vocabulary against the TypeScript's, key by key; the plugin findings by file and severity.
    fn resolved(&mut self, name: &str, theirs: &Value, ours: &Value) {
        for part in ["vocab", "nodes"] {
            self.record(name, &format!("/modules/resolved/{part}"), &theirs[part], &ours[part]);
        }
        self.same_set(
            name,
            "/modules/resolved/plugin",
            &theirs["plugin"],
            &ours["plugin"],
            |_| false,
        );
    }
}
