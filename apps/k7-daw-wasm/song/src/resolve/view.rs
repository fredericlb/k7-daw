// resolve/view.rs — what a resolved module SHOWS: its type, its rack, its panel with the hidden widgets dropped, its MIDI map.
// A reader would get wrong `cols` and `rows` of 0: a merged panel without a grid reads as the TypeScript's `?? 0`, a `u32` here.

use super::Resolved;
use crate::modules::panels::CcBinding;
use crate::modules::{ModuleType, RackSound, RackType, Tab, Widget};
use indexmap::IndexMap;
use serde::Serialize;

/// What a module IS: a plugin slot or a note-to-clip rack; "plugin" when unstated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InstrumentType {
    Plugin,
    Rack,
}

pub fn resolved_type(m: Option<&Resolved>) -> InstrumentType {
    match m.and_then(|m| m.authored.module_type) {
        Some(ModuleType::Rack) => InstrumentType::Rack,
        _ => InstrumentType::Plugin,
    }
}

/// The rack a module runs: total, so a module with no rack answers an empty one.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ResolvedRack {
    pub rack_type: RackType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub symbols: IndexMap<String, RackSound>,
}

pub fn resolved_rack(m: Option<&Resolved>) -> ResolvedRack {
    match m.and_then(|m| m.authored.rack.as_ref()) {
        Some(r) => ResolvedRack {
            rack_type: r.rack_type,
            source: r.source.clone(),
            symbols: r.symbols.clone(),
        },
        None => ResolvedRack {
            rack_type: RackType::Samples,
            source: None,
            symbols: IndexMap::new(),
        },
    }
}

/// The narrow twin of a shown panel.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "WebModuleLayout", optional_fields))]
pub struct ResolvedLayout {
    pub cols: u32,
    pub rows: u32,
    #[cfg_attr(feature = "ts", ts(as = "crate::rec::Rec<Widget>"))]
    pub widgets: IndexMap<String, Widget>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tabs: Option<Vec<Tab>>,
}

/// The widget grid a module SHOWS, normalised, with `hidden` widgets dropped; the script rides on it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "WebModuleUI", optional_fields))]
pub struct ResolvedUi {
    pub cols: u32,
    pub rows: u32,
    #[cfg_attr(feature = "ts", ts(as = "crate::rec::Rec<Widget>"))]
    pub widgets: IndexMap<String, Widget>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tabs: Option<Vec<Tab>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vertical: Option<ResolvedLayout>,
}

fn shown(ws: &IndexMap<String, Widget>) -> IndexMap<String, Widget> {
    ws.iter()
        .filter(|(_, w)| w.hidden != Some(true))
        .map(|(k, w)| (k.clone(), w.clone()))
        .collect()
}

fn shown_tabs(tabs: &Option<Vec<Tab>>) -> Option<Vec<Tab>> {
    let tabs = tabs.as_ref().filter(|t| !t.is_empty())?;
    Some(
        tabs.iter()
            .map(|t| Tab {
                name: t.name.clone(),
                widgets: shown(&t.widgets),
            })
            .collect(),
    )
}

fn grid(n: u64) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

pub fn resolved_ui(m: Option<&Resolved>) -> Option<ResolvedUi> {
    let ui = m?.authored.ui.as_ref()?;
    Some(ResolvedUi {
        cols: grid(ui.cols.0),
        rows: grid(ui.rows.0),
        widgets: shown(&ui.widgets),
        tabs: shown_tabs(&ui.tabs),
        script: ui.script.clone().filter(|s| !s.is_empty()),
        vertical: ui.vertical.as_ref().map(|v| ResolvedLayout {
            cols: grid(v.cols.0),
            rows: grid(v.rows.0),
            widgets: shown(&v.widgets),
            tabs: shown_tabs(&v.tabs),
        }),
    })
}

/// The MIDI map a module plays by; an empty map answers none.
pub fn resolved_cc(m: Option<&Resolved>) -> Option<IndexMap<String, CcBinding>> {
    m?.cc.clone().filter(|c| !c.is_empty())
}
