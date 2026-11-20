// modules/ui.rs — the panel a module carries: a grid of widgets, optional tabs, an optional narrow `vertical` twin.
// A reader would get wrong what is strict: a widget and the panel strip unknown keys like zod's object, the tabs and the layout refuse them.

use super::scalars::HexColor;
use crate::files::scalar::{NonEmpty, Pos, PosInt, js_space};
use indexmap::IndexMap;
use k7x::PathSeg;
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};

/// The longest a tab's name may be, in UTF-16 units: it is drawn in a fixed column.
pub const TAB_NAME_MAX: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WidgetType {
    Label,
    Knob,
    Slider,
    Fader,
    Number,
    Toggle,
    Button,
    Meter,
    Waveform,
    Dropdown,
    Filepicker,
    Lcdscreen,
    Tap,
    Group,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Curve {
    Lin,
    Log,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WaveSource {
    Input,
    Output,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WidgetOption {
    pub label: String,
    pub value: f64,
}

/// One control. Its refinements (a field that belongs to another type) are `Widget::check`, placed on the key.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Widget {
    #[serde(rename = "type")]
    pub widget_type: WidgetType,
    pub col: u64,
    pub row: u64,
    #[serde(default, rename = "colSpan", skip_serializing_if = "Option::is_none")]
    pub col_span: Option<PosInt>,
    #[serde(default, rename = "rowSpan", skip_serializing_if = "Option::is_none")]
    pub row_span: Option<PosInt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind: Option<NonEmpty>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<Pos>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curve: Option<Curve>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<WaveSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<WidgetOption>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub library: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<HexColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invert: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pair: Option<NonEmpty>,
}

fn key(name: &str) -> (Vec<PathSeg>, String) {
    (vec![PathSeg::Key(name.to_owned())], String::new())
}

impl Widget {
    /// The first refinement the widget breaks: the key it is about and why.
    pub fn check(&self) -> Option<(Vec<PathSeg>, String)> {
        use WidgetType as T;
        let wrong = |field: &str, why: &str| {
            let (path, _) = key(field);
            Some((
                path,
                format!(
                    "`{field}` is not a {} field — {why}",
                    format!("{:?}", self.widget_type).to_lowercase()
                ),
            ))
        };
        let ranged = matches!(self.widget_type, T::Knob | T::Slider | T::Fader | T::Number);
        if !ranged {
            for (name, set) in [
                ("min", self.min.is_some()),
                ("max", self.max.is_some()),
                ("step", self.step.is_some()),
                ("curve", self.curve.is_some()),
            ] {
                if set {
                    return wrong(name, "only knob, slider, fader and number carry a range");
                }
            }
        }
        if self.widget_type != T::Label && self.text.is_some() {
            return wrong("text", "only a label draws text");
        }
        if self.widget_type == T::Lcdscreen && self.bind.is_some() {
            return wrong("bind", "an lcdscreen drives no param — widgets.js draws it");
        }
        if self.widget_type != T::Knob && self.pair.is_some() {
            return wrong("pair", "only a knob crossfades two params");
        }
        if let Some(pair) = &self.pair
            && self.bind.as_ref().is_none_or(|b| b.0 == pair.0)
        {
            let (path, _) = key("pair");
            return Some((
                path,
                "a crossfade knob binds the wet param and pairs a DIFFERENT, dry one".to_owned(),
            ));
        }
        if self.widget_type != T::Group && self.color.is_some() {
            return wrong("color", "only a group paints a background");
        }
        if self.widget_type != T::Group && self.invert.is_some() {
            return wrong("invert", "only a group's on/off switch reads its param backwards");
        }
        if self.widget_type != T::Waveform && self.source.is_some() {
            return wrong("source", "only a waveform picks an audio tap");
        }
        if self.widget_type != T::Dropdown {
            if self.options.is_some() {
                return wrong("options", "only a dropdown has settings");
            }
            if self.library.is_some() {
                return wrong("library", "only a dropdown lists a library");
            }
        }
        if self.widget_type == T::Filepicker && self.bind.is_none() {
            let (path, _) = key("bind");
            return Some((
                path,
                "a filepicker needs `bind` — the file-valued param it writes".to_owned(),
            ));
        }
        None
    }
}

/// A tab's name: trimmed, one to ten UTF-16 units.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct TabName(pub String);

impl<'de> Deserialize<'de> for TabName {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        let name = raw.trim_matches(js_space);
        if name.is_empty() {
            Err(de::Error::custom("expected a non-empty string"))
        } else if name.encode_utf16().count() > TAB_NAME_MAX {
            Err(de::Error::custom(format!(
                "a tab name is at most {TAB_NAME_MAX} characters"
            )))
        } else {
            Ok(TabName(name.to_owned()))
        }
    }
}

/// One page of a panel: a name and the widgets drawn while it is open.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tab {
    pub name: TabName,
    pub widgets: IndexMap<String, Widget>,
}

/// The narrow layout of the same panel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    pub cols: PosInt,
    pub rows: PosInt,
    pub widgets: IndexMap<String, Widget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tabs: Option<Vec<Tab>>,
}

/// A module's whole widget grid.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ui {
    pub cols: PosInt,
    pub rows: PosInt,
    pub widgets: IndexMap<String, Widget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tabs: Option<Vec<Tab>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertical: Option<Layout>,
    /// The `widgets.js` beside a panel file, as text: never read from an inline `ui:`, which strips the key.
    #[serde(default, skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
}

fn at(base: &[PathSeg], rest: Vec<PathSeg>) -> Vec<PathSeg> {
    let mut p = base.to_vec();
    p.extend(rest);
    p
}

fn widgets_check(base: &[PathSeg], widgets: &IndexMap<String, Widget>) -> Option<(Vec<PathSeg>, String)> {
    for (id, w) in widgets {
        if let Some((rest, message)) = w.check() {
            let mut path = at(base, vec![PathSeg::Key("widgets".to_owned()), PathSeg::Key(id.clone())]);
            path.extend(rest);
            return Some((path, message));
        }
    }
    None
}

fn tabs_check(base: &[PathSeg], tabs: &[Tab]) -> Option<(Vec<PathSeg>, String)> {
    for (i, t) in tabs.iter().enumerate() {
        let tab = at(base, vec![PathSeg::Key("tabs".to_owned()), PathSeg::Index(i)]);
        if let Some(found) = widgets_check(&tab, &t.widgets) {
            return Some(found);
        }
    }
    None
}

impl Ui {
    /// The first refinement the panel breaks, the path starting at `base` (the key `ui` is under).
    pub fn check(&self, base: &[PathSeg]) -> Option<(Vec<PathSeg>, String)> {
        if let Some(found) = widgets_check(base, &self.widgets) {
            return Some(found);
        }
        if let Some(found) = tabs_check(base, self.tabs.as_deref().unwrap_or_default()) {
            return Some(found);
        }
        if let Some(v) = &self.vertical {
            let vbase = at(base, vec![PathSeg::Key("vertical".to_owned())]);
            if let Some(found) = widgets_check(&vbase, &v.widgets) {
                return Some(found);
            }
            if let Some(found) = tabs_check(&vbase, v.tabs.as_deref().unwrap_or_default()) {
                return Some(found);
            }
        }
        let tabs = self.tabs.as_deref().unwrap_or_default();
        let mut seen: Vec<&str> = Vec::new();
        for (i, t) in tabs.iter().enumerate() {
            if seen.contains(&t.name.0.as_str()) {
                let path = at(
                    base,
                    vec![
                        PathSeg::Key("tabs".to_owned()),
                        PathSeg::Index(i),
                        PathSeg::Key("name".to_owned()),
                    ],
                );
                return Some((path, format!("two tabs are called \"{}\"", t.name.0)));
            }
            seen.push(&t.name.0);
        }
        if let Some(v) = &self.vertical {
            let twin = v.tabs.as_deref().unwrap_or_default();
            let same = tabs.len() == twin.len() && tabs.iter().zip(twin).all(|(a, b)| a.name == b.name);
            if !same {
                let path = at(
                    base,
                    vec![PathSeg::Key("vertical".to_owned()), PathSeg::Key("tabs".to_owned())],
                );
                return Some((
                    path,
                    "the vertical layout must have the same tabs as the panel, in the same order".to_owned(),
                ));
            }
        }
        None
    }
}
