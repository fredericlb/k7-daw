// resolve/findings.rs — the two plugin findings that need a RESOLVED node: a plugin that is not supported, and one no engine loads.
// A reader would get wrong that the plugin may be INHERITED: a node on `@meta/not-supported` need not restate it.

use super::Resolved;
use crate::files::{Finding, Severity};
use crate::modules::files::reaches_engine;
use crate::modules::types::{NOT_SUPPORTED_TYPE, hosts_plugin};
use crate::modules::{LoadedModule, ModuleKind, Plugin};
use crate::source::path::join;
use indexmap::IndexMap;

/// The type a cold module's offline pass uses; it has no engine plugin on purpose.
const OFFLINE_TYPE: &str = "offline";

/// One finding per project node whose RESOLVED plugin `judge` has something to say about, at the node's own file and `plugin` key.
fn each(
    nodes: &IndexMap<String, LoadedModule>,
    resolved: &IndexMap<String, Resolved>,
    judge: impl Fn(&Plugin, &str, &LoadedModule) -> Option<(Severity, String)>,
) -> Vec<Finding> {
    let mut out = Vec::new();
    for (name, m) in resolved {
        let Some(plugin) = m.authored.plugin.as_ref() else {
            continue;
        };
        let Some((severity, message)) = judge(plugin, name, m) else {
            continue;
        };
        let own = nodes.get(name);
        let (line, col) = own.and_then(|o| o.authored.plugin_at).unwrap_or((1, 1));
        out.push(Finding {
            file: own.map_or_else(|| join("nodes", &format!("{name}.k7x")), LoadedModule::own_label),
            line,
            col,
            message,
            severity,
        });
    }
    out
}

fn shown_name<'p>(p: &'p Plugin, key: &'p str) -> &'p str {
    p.name.as_deref().filter(|n| !n.is_empty()).unwrap_or(key)
}

/// A plugin the engine does not support is bypassed, which is a warning.
pub fn not_supported_findings(
    nodes: &IndexMap<String, LoadedModule>,
    resolved: &IndexMap<String, Resolved>,
) -> Vec<Finding> {
    each(nodes, resolved, |p, key, _| {
        (p.plugin_type.as_deref().unwrap_or("").to_lowercase() == NOT_SUPPORTED_TYPE).then(|| {
            (
                Severity::Warning,
                format!(
                    "plugin \"{}\" is not supported — it will be bypassed",
                    shown_name(p, key)
                ),
            )
        })
    })
}

/// A plugin type no engine loads compiles to NOTHING, so it is said here as an error rather than played as silence.
pub fn unknown_plugin_findings(
    nodes: &IndexMap<String, LoadedModule>,
    resolved: &IndexMap<String, Resolved>,
) -> Vec<Finding> {
    each(nodes, resolved, |p, key, m| {
        if m.kind != Some(ModuleKind::Plugin) || !hosts_plugin(m.category.as_deref()) {
            return None;
        }
        let raw = p.plugin_type.as_deref().unwrap_or("");
        let lower = raw.to_lowercase();
        if lower == NOT_SUPPORTED_TYPE || lower == OFFLINE_TYPE || reaches_engine(&lower) {
            return None;
        }
        Some((
            Severity::Error,
            format!(
                "plugin \"{}\" has type \"{raw}\", which no engine loads — it plays nothing (a built-in is \"internal\")",
                shown_name(p, key)
            ),
        ))
    })
}

/// Both findings over the resolved nodes of a node project.
pub fn plugin_findings(nodes: &IndexMap<String, LoadedModule>, resolved: &IndexMap<String, Resolved>) -> Vec<Finding> {
    let mut out = not_supported_findings(nodes, resolved);
    out.extend(unknown_plugin_findings(nodes, resolved));
    out
}
