// views/dead.rs — what the engine refuses or nothing reads, said where a person sees it: dead automations, dead project fields.
// A reader would get wrong that a chain name is a file name: one holding a separator (or a Windows hazard) names no chain file.

use super::{NAME_CUT, Spend, capped_finding};
use crate::cache::Cache;
use crate::clips::js_number;
use crate::doc::{PluginDoc, ProjectDoc};
use crate::files::{Finding, Severity};
use crate::modules::builtin_param_automatable;
use crate::read::read_head;
use crate::source::path::{PathRule, join, windows_hazard};
use crate::source::{Kind, Source, SourceError};
use indexmap::IndexSet;
use serde_json::Value;
use std::collections::HashMap;

/// The most bytes a message of this view keeps.
const MESSAGE_CUT: usize = 300;

/// A string cut to `max` characters, with an ellipsis when it was longer.
fn cut(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((at, _)) => format!("{}…", &s[..at]),
        None => s.to_owned(),
    }
}

/// A message cut to `MESSAGE_CUT` bytes, on a character.
fn bounded(mut s: String) -> String {
    if s.len() > MESSAGE_CUT {
        // The ellipsis is three bytes, and the whole stays within the cut.
        let mut at = MESSAGE_CUT - 3;
        while !s.is_char_boundary(at) {
            at -= 1;
        }
        s.truncate(at);
        s.push('…');
    }
    s
}

/// What `String(x)` makes of a JSON value, for the names a graph declares.
fn js_string(v: Option<&Value>) -> String {
    match v {
        None => "undefined".to_owned(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.as_f64().map_or_else(|| n.to_string(), js_number),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Null) => "null".to_owned(),
        Some(Value::Array(a)) => a
            .iter()
            .map(|x| if x.is_null() { String::new() } else { js_string(Some(x)) })
            .collect::<Vec<_>>()
            .join(","),
        Some(Value::Object(_)) => "[object Object]".to_owned(),
    }
}

/// The names a compiled graph declares, or None when the plugin carries no graph or one that will not read.
fn graph_params(json: Option<&str>) -> Option<IndexSet<String>> {
    let g: Value = serde_json::from_str(json.filter(|j| !j.is_empty())?).ok()?;
    let params = match &g {
        Value::Null => return None,
        Value::Object(o) => o.get("params"),
        _ => None,
    };
    let list = match params {
        None | Some(Value::Null) => return Some(IndexSet::new()),
        Some(Value::Array(a)) => a,
        Some(_) => return None,
    };
    let mut names = IndexSet::new();
    for p in list {
        let name = match p {
            Value::Null => return None,
            Value::Object(o) => o.get("name"),
            _ => None,
        };
        names.insert(js_string(name));
    }
    Some(names)
}

/// Up to `NAME_CUT` characters of the names, joined; no name is copied past the cut, so a list costs its cut and not its size.
fn declared_list(names: &IndexSet<String>) -> String {
    let (mut out, mut chars) = (String::new(), 0);
    for (i, n) in names.iter().enumerate() {
        if chars > NAME_CUT {
            break;
        }
        if i > 0 {
            out.push_str(", ");
            chars += 2;
        }
        let part: String = n.chars().take(NAME_CUT + 1).collect();
        chars += part.chars().count();
        out.push_str(&part);
    }
    if out.is_empty() {
        "none".to_owned()
    } else {
        cut(&out, NAME_CUT)
    }
}

/// Whether a chain name can be a file name of `chains/`: only a separator (or a Windows hazard) rules it out.
fn is_file_name(chain: &str, rule: PathRule) -> bool {
    !chain.contains('/') && !(rule.is_windows() && windows_hazard(chain))
}

/// Where a lane's chain file is: `chains/<chain>.k7x` when a stat says a file is there, else `project.k7x`.
struct Places<'s> {
    source: &'s dyn Source,
    project: &'s str,
    held: HashMap<String, String>,
    rule: PathRule,
}

impl Places<'_> {
    fn of(&mut self, chain: &str, spend: &mut Spend) -> Result<Option<String>, SourceError> {
        if let Some(f) = self.held.get(chain) {
            return Ok(Some(f.clone()));
        }
        if !spend.step() {
            return Ok(None);
        }
        let rel = format!("chains/{chain}.k7x");
        let file = if is_file_name(chain, self.rule) {
            match self.source.stat(&join(self.project, &rel)) {
                Ok(st) if st.kind == Kind::File => rel,
                Ok(_) | Err(SourceError::NotFound | SourceError::Io(_) | SourceError::Outside) => {
                    "project.k7x".to_owned()
                }
                Err(e) => return Err(e),
            }
        } else {
            "project.k7x".to_owned()
        };
        self.held.insert(chain.to_owned(), file.clone());
        Ok(Some(file))
    }
}

/// Why a curve on `param` of this plugin does nothing, or None when it works.
fn why_dead(p: &PluginDoc, param: &str, declared: Option<&IndexSet<String>>) -> Option<String> {
    if !builtin_param_automatable(&p.plugin_type, param) {
        return Some(format!(
            "{}'s '{}' is plain plugin state, which no curve can drive — pick an engine whose knobs automate",
            cut(&p.plugin_type, NAME_CUT),
            cut(param, NAME_CUT)
        ));
    }
    let names = declared.filter(|d| !d.contains(param))?;
    Some(format!(
        "this module has no '{}' to automate (it has {})",
        cut(param, NAME_CUT),
        declared_list(names)
    ))
}

/// One warning per automation lane the engine cannot apply, at the chain file holding the node (decision 637).
pub fn dead_automations(
    source: &dyn Source,
    project: &str,
    doc: &ProjectDoc,
    rule: PathRule,
) -> Result<Vec<Finding>, SourceError> {
    let (mut out, mut spend) = (Vec::new(), Spend::new());
    let mut places = Places {
        source,
        project,
        held: HashMap::new(),
        rule,
    };
    for t in &doc.tracks {
        if t.automation.is_empty() {
            continue;
        }
        // The first plugin of an id is the one `find` meets, and each plugin's graph is read once.
        let mut at: HashMap<&str, usize> = HashMap::new();
        for (i, p) in t.plugins.iter().enumerate() {
            at.entry(p.id.as_str()).or_insert(i);
        }
        let mut graphs: HashMap<usize, Option<IndexSet<String>>> = HashMap::new();
        for lane in &t.automation {
            let Some(id) = lane.plugin_id.as_deref() else { continue };
            let Some(&i) = at.get(id) else { continue };
            let plugin = &t.plugins[i];
            let declared = graphs
                .entry(i)
                .or_insert_with(|| graph_params(plugin.dsp_graph.as_deref()));
            let Some(why) = why_dead(plugin, &lane.param, declared.as_ref()) else {
                continue;
            };
            let message = bounded(format!(
                "a curve on '{}' does nothing: {why}",
                cut(&lane.param, NAME_CUT)
            ));
            let chain = id.split(':').next().unwrap_or("");
            let file = places.of(chain, &mut spend)?;
            // The finding's file is a copy too, so it is charged with its message.
            let weight = message.len() + file.as_ref().map_or(0, String::len);
            let Some(file) = file.filter(|_| spend.bytes(weight)) else {
                out.push(capped_finding("project.k7x", "the dead automations check"));
                return Ok(out);
            };
            out.push(Finding {
                file,
                line: 1,
                col: 1,
                message,
                severity: Severity::Warning,
            });
        }
    }
    Ok(out)
}

/// The project fields nothing reads any more, each a WARNING named `project.k7x` (decision 638).
pub fn dead_project_fields(source: &dyn Source, cache: &mut Cache, project: &str) -> Result<Vec<Finding>, SourceError> {
    let head = read_head(source, cache, project)?;
    let Some(settings) = head
        .root
        .as_deref()
        .and_then(|r| r.project.as_ref())
        .and_then(|f| f.project.as_ref())
    else {
        return Ok(Vec::new());
    };
    let fields = [
        ("modules_from", settings.modules_from.is_some()),
        ("libraries_from", settings.libraries_from.is_some()),
    ];
    Ok(fields
        .into_iter()
        .filter(|(_, set)| *set)
        .map(|(key, _)| Finding {
            file: "project.k7x".to_owned(),
            line: 1,
            col: 1,
            message: format!(
                "`{key}` is no longer read — the shared folders are found where they sit (../_modules, ../_library, the médiathèque). This line can be deleted."
            ),
            severity: Severity::Warning,
        })
        .collect())
}
