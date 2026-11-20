// modules/panels.rs — a module's panel (`ui.k7x`, `widgets.js`) and MIDI map (`controller.k7x`), found beside its folder.
// A reader would get wrong that a file that does not decode is a WARNING and no claim: a farther library's file for the ref wins.

use super::scalars::KeyRule;
use super::tree::ModuleFolder;
use super::types::{MODULE_FILE, engine_ref, meta_ref, type_meta};
use super::unions::Keyed;
use super::{LoadedModule, ModuleKind, Tab, Ui, Widget};
use crate::cache::Cache;
use crate::files::scalar::{PosInt, checked_num};
use crate::files::{Check, Finding, Severity, decode_with, from_map};
use crate::source::path::join;
use crate::source::{Kind, Source, SourceError, Span};
use indexmap::IndexMap;
use k7x::PathSeg;
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};

pub const UI_FILE: &str = "ui.k7x";
pub const CONTROLLER_FILE: &str = "controller.k7x";
pub const WIDGETS_SCRIPT: &str = "widgets.js";

/// A panel's script is output that ships to the browser: past this it is absent, with a warning.
pub const MAX_SCRIPT: u64 = 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UiFields {
    cols: PosInt,
    rows: PosInt,
    widgets: IndexMap<String, Widget>,
    #[serde(default)]
    tabs: Option<Vec<Tab>>,
    #[serde(default)]
    vertical: Option<super::Layout>,
}

/// One `ui.k7x`: the panel beside a module's `module.k7x`, strict: a key it does not know, `script` included, is refused.
#[derive(Clone, Debug, PartialEq)]
pub struct UiFile(pub Ui);

impl<'de> Deserialize<'de> for UiFile {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let f = UiFields::deserialize(d)?;
        Ok(UiFile(Ui {
            cols: f.cols,
            rows: f.rows,
            widgets: f.widgets,
            tabs: f.tabs,
            vertical: f.vertical,
            script: None,
        }))
    }
}

impl Check for UiFile {
    fn check(&self) -> Option<(Vec<PathSeg>, String)> {
        self.0.check(&[])
    }
}

pub fn decode_ui_file(file: &str, text: &str) -> Result<UiFile, Finding> {
    decode_with(file, text, from_map::<UiFile>)
}

checked_num!(Channel(u8), |v| (1..=16).contains(v), "expected a channel from 1 to 16");
checked_num!(
    CcNumber(u8),
    |v| *v <= 127,
    "expected a controller number from 0 to 127"
);

/// One control that moves one param: a channel as a musician counts it (1 to 16) and a CC number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CcBinding {
    pub ch: u8,
    pub cc: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CcFields {
    ch: Channel,
    cc: CcNumber,
}

impl<'de> Deserialize<'de> for CcBinding {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let f = CcFields::deserialize(d)?;
        Ok(CcBinding { ch: f.ch.0, cc: f.cc.0 })
    }
}

/// A param name a map may bind: not empty.
struct ParamName;

impl KeyRule for ParamName {
    fn ok(key: &str) -> bool {
        !key.is_empty()
    }
    const MESSAGE: &'static str = "expected a non-empty string";
}

/// One `controller.k7x`: the params it binds, each to one control; an empty file is an empty map.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(type = "{ [param: string]: CcBinding }"))]
pub struct ControllerFile(pub IndexMap<String, CcBinding>);

impl Serialize for ControllerFile {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

impl<'de> Deserialize<'de> for ControllerFile {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Keyed::<ParamName, CcBinding>::deserialize(d).map(|k| ControllerFile(k.0))
    }
}

impl Check for ControllerFile {}

pub fn decode_controller(file: &str, text: &str) -> Result<ControllerFile, Finding> {
    decode_with(file, text, from_map::<ControllerFile>)
}

/// What a panel weighs in the vocabulary's budget: one per widget, in the tabs and the vertical twin too.
pub fn ui_weight(ui: &Ui) -> usize {
    let tabs = |t: &Option<Vec<Tab>>| t.iter().flatten().map(|t| t.widgets.len()).sum::<usize>();
    ui.widgets.len() + tabs(&ui.tabs) + ui.vertical.as_ref().map_or(0, |v| v.widgets.len() + tabs(&v.tabs))
}

/// The panels and maps of every module folder, by ref, nearest library first.
#[derive(Debug, Default)]
pub struct Panels {
    pub ui: IndexMap<String, Ui>,
    pub maps: IndexMap<String, IndexMap<String, CcBinding>>,
    /// Warnings: a file that did not decode, a script that was refused, a path the Source refused.
    pub findings: Vec<Finding>,
    /// What was read, weighed as the vocabulary weighs: a widget or a binding each.
    pub weight: usize,
    pub capped: bool,
}

fn warn(file: String, message: String) -> Finding {
    Finding {
        file,
        line: 1,
        col: 1,
        message,
        severity: Severity::Warning,
    }
}

/// The `widgets.js` beside a panel as text, or None with the warning that says why it is not used.
fn script_beside(
    source: &dyn Source,
    dir: &str,
    named: &str,
    findings: &mut Vec<Finding>,
) -> Result<Option<String>, SourceError> {
    let path = join(dir, WIDGETS_SCRIPT);
    let st = match source.stat(&path) {
        Ok(st) if st.kind == Kind::File => st,
        Ok(_) | Err(SourceError::NotFound | SourceError::Io(_)) => return Ok(None),
        Err(SourceError::Outside) => {
            findings.push(warn(
                named.to_owned(),
                "outside the folders this compile may read — ignored".into(),
            ));
            return Ok(None);
        }
        Err(e) => return Err(e),
    };
    let too_big = |findings: &mut Vec<Finding>| {
        findings.push(warn(
            named.to_owned(),
            format!("the script is over {MAX_SCRIPT} bytes — ignored"),
        ));
    };
    if st.len > MAX_SCRIPT {
        too_big(findings);
        return Ok(None);
    }
    // A whole read, so a recording Source keeps the text; the stat above already refused a file past the limit.
    let bytes = match source.read(&path, Span::All) {
        Ok(b) => b,
        Err(SourceError::NotFound | SourceError::Io(_) | SourceError::Outside) => return Ok(None),
        Err(e) => return Err(e),
    };
    if bytes.len() as u64 > MAX_SCRIPT {
        too_big(findings);
        return Ok(None);
    }
    match String::from_utf8(bytes) {
        Ok(text) if text.is_empty() => Ok(None),
        Ok(text) => Ok(Some(text)),
        Err(_) => {
            findings.push(warn(named.to_owned(), "the script is not valid UTF-8 — ignored".into()));
            Ok(None)
        }
    }
}

/// A file of a folder that is probed by `stat`, so a case-insensitive file system finds `UI.K7X`.
fn probe(source: &dyn Source, path: &str, named: &str, findings: &mut Vec<Finding>) -> Result<bool, SourceError> {
    match source.stat(path) {
        Ok(st) => Ok(st.kind == Kind::File),
        Err(SourceError::NotFound | SourceError::Io(_)) => Ok(false),
        Err(SourceError::Outside) => {
            findings.push(warn(
                named.to_owned(),
                "outside the folders this compile may read — ignored".into(),
            ));
            Ok(false)
        }
        Err(e) => Err(e),
    }
}

/// Reads every panel and map the folders hold: nearest first, the first readable file for a ref winning; `used` is the budget spent.
pub fn read_panels(
    source: &dyn Source,
    cache: &mut Cache,
    folders: &[ModuleFolder],
    used: usize,
    cap: usize,
) -> Result<Panels, SourceError> {
    let mut out = Panels::default();
    for f in folders {
        let r = &f.module_ref;
        if !out.ui.contains_key(r) {
            let (path, named) = (join(&f.path, UI_FILE), format!("{r}/{UI_FILE}"));
            if probe(source, &path, &named, &mut out.findings)? {
                match cache.read::<UiFile>(source, &path)? {
                    Ok(file) => {
                        let mut ui = file.0.clone();
                        let w = ui_weight(&ui);
                        if used + out.weight + w > cap {
                            return Ok(stop(out, &named, cap));
                        }
                        out.weight += w;
                        ui.script =
                            script_beside(source, &f.path, &format!("{r}/{WIDGETS_SCRIPT}"), &mut out.findings)?;
                        out.ui.insert(r.clone(), ui);
                    }
                    Err(found) => out.findings.push(Finding {
                        file: named,
                        message: format!("did not load — {}", found.message),
                        severity: Severity::Warning,
                        ..found
                    }),
                }
            }
        }
        if !out.maps.contains_key(r) {
            let (path, named) = (join(&f.path, CONTROLLER_FILE), format!("{r}/{CONTROLLER_FILE}"));
            if probe(source, &path, &named, &mut out.findings)? {
                match cache.read::<ControllerFile>(source, &path)? {
                    Ok(file) => {
                        if used + out.weight + file.0.len() > cap {
                            return Ok(stop(out, &named, cap));
                        }
                        out.weight += file.0.len();
                        out.maps.insert(r.clone(), file.0.clone());
                    }
                    Err(found) => out.findings.push(Finding {
                        file: named,
                        message: format!("did not load — {}", found.message),
                        severity: Severity::Warning,
                        ..found
                    }),
                }
            }
        }
    }
    Ok(out)
}

fn stop(mut out: Panels, file: &str, cap: usize) -> Panels {
    out.capped = true;
    out.findings.push(warn(
        file.to_owned(),
        format!("the panels and maps hold more than {cap} entries — the walk stopped"),
    ));
    out
}

/// The first folder of `module_ref` holding a `module.k7x`: where its own files live; an engine-catalogue ref has none.
pub fn module_folder_for_ref(
    source: &dyn Source,
    folders: &[ModuleFolder],
    module_ref: &str,
) -> Result<Option<String>, SourceError> {
    for f in folders.iter().filter(|f| f.module_ref == module_ref) {
        match source.stat(&join(&f.path, &format!("{MODULE_FILE}.k7x"))) {
            Ok(st) if st.kind == Kind::File => return Ok(Some(f.path.clone())),
            Ok(_) | Err(SourceError::NotFound | SourceError::Io(_) | SourceError::Outside) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(None)
}

/// The map a module declares in its own folder, or an empty one.
pub fn read_module_controller(
    source: &dyn Source,
    cache: &mut Cache,
    folders: &[ModuleFolder],
    module_ref: &str,
) -> Result<ControllerFile, SourceError> {
    let Some(folder) = module_folder_for_ref(source, folders, module_ref)? else {
        return Ok(ControllerFile::default());
    };
    let path = join(&folder, CONTROLLER_FILE);
    match source.stat(&path) {
        Ok(st) if st.kind == Kind::File => {}
        Ok(_) | Err(SourceError::NotFound | SourceError::Io(_) | SourceError::Outside) => {
            return Ok(ControllerFile::default());
        }
        Err(e) => return Err(e),
    }
    Ok(cache
        .read::<ControllerFile>(source, &path)?
        .map_or_else(|_| ControllerFile::default(), |c| (*c).clone()))
}

/// The ref a module of a type folder answers to: an io module's is its bare name, as the TypeScript's `moduleRefOf`.
pub fn module_ref_of(category: &str, name: &str) -> String {
    if type_meta(category).is_some_and(|t| t.kind == ModuleKind::Io) {
        name.to_owned()
    } else {
        format!("{category}/{name}")
    }
}

/// The refs a local module's OWN FILES may be filed under, best first: its own, then what it derives from.
pub fn module_own_refs(m: &LoadedModule) -> Vec<String> {
    let from = m.authored.extends.clone().unwrap_or_default();
    let ambient = if m.kind == Some(ModuleKind::Io) {
        meta_ref(&m.file)
    } else {
        engine_ref(&m.file)
    };
    let Some(category) = m.category.as_deref().filter(|c| !c.is_empty()) else {
        return vec![if from.is_empty() { ambient } else { from }];
    };
    let own = module_ref_of(category, &m.file);
    if from.is_empty() {
        vec![own, ambient]
    } else {
        vec![own, from]
    }
}
