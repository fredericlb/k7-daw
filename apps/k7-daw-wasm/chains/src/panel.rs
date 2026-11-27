// panel.rs — the widget grid a card draws, as the page's `PageModuleUI`: a grid of widgets, tabs, and the narrow twin.
// A reader would get wrong that only the fields the rules read are typed: any other field rides in `rest` and comes back as it came.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// One entry of a dropdown: the label, the number it writes, and the text the page passes with it.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct WidgetOption {
    pub label: String,
    pub value: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arg: Option<String>,
}

/// One control, its place on the grid and the fields the build reads.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Widget {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub col: f64,
    #[serde(default)]
    pub row: f64,
    #[serde(default, rename = "colSpan", skip_serializing_if = "Option::is_none")]
    pub col_span: Option<f64>,
    #[serde(default, rename = "rowSpan", skip_serializing_if = "Option::is_none")]
    pub row_span: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<WidgetOption>>,
    #[serde(flatten)]
    pub rest: Map<String, Value>,
}

impl Widget {
    /// A widget of `kind` at a cell, every other field absent.
    pub fn at(kind: &str, col: f64, row: f64) -> Widget {
        Widget {
            kind: kind.to_owned(),
            col,
            row,
            ..Widget::default()
        }
    }

    pub fn bound(mut self, bind: &str, label: &str) -> Widget {
        self.bind = Some(bind.to_owned());
        self.label = Some(label.to_owned());
        self
    }

    pub fn span(mut self, cols: f64, rows: f64) -> Widget {
        self.col_span = Some(cols);
        self.row_span = Some(rows);
        self
    }

    pub fn range(mut self, min: f64, max: f64) -> Widget {
        self.min = Some(min);
        self.max = Some(max);
        self
    }

    /// How many columns it covers: at least one.
    pub fn cols(&self) -> f64 {
        self.col_span.unwrap_or(1.0).max(1.0)
    }

    pub fn rows(&self) -> f64 {
        self.row_span.unwrap_or(1.0).max(1.0)
    }
}

pub type Widgets = IndexMap<String, Widget>;

/// One page of a panel: its name, the widgets drawn while it is open, and the width a fold gave it.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Tab {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub widgets: Widgets,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cols: Option<f64>,
    #[serde(flatten)]
    pub rest: Map<String, Value>,
}

/// A card's panel: the grid's size, the widgets on every tab, the tabs, and the narrow twin.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Panel {
    #[serde(default)]
    pub cols: f64,
    #[serde(default)]
    pub rows: f64,
    #[serde(default)]
    pub widgets: Widgets,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tabs: Option<Vec<Tab>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertical: Option<Box<Panel>>,
    #[serde(default, rename = "tabsAs", skip_serializing_if = "Option::is_none")]
    pub tabs_as: Option<String>,
    #[serde(flatten)]
    pub rest: Map<String, Value>,
}

impl Panel {
    pub fn new(cols: f64, rows: f64, widgets: Vec<(&str, Widget)>) -> Panel {
        Panel {
            cols,
            rows,
            widgets: widgets.into_iter().map(|(k, w)| (k.to_owned(), w)).collect(),
            ..Panel::default()
        }
    }

    /// Whether the panel draws its tabs: two or more, since a single tab is just the panel.
    pub fn has_tabs(&self) -> bool {
        self.tabs.as_ref().is_some_and(|t| t.len() > 1)
    }

    /// Every widget as (id, widget), the shared ones first, then each tab's in order.
    pub fn all_widgets(&self) -> impl Iterator<Item = (&String, &Widget)> {
        let tabs = self.tabs.iter().flatten().flat_map(|t| t.widgets.iter());
        self.widgets.iter().chain(tabs)
    }
}
