// node_ui.rs — the panels the graph draws for its built-in cards: a lane's strip, the sums, a rack's pads, a live input, a lens.
// A reader would get wrong that nothing stands behind them in a module file: they are made here, so a card's size and its panel agree.

use crate::graph::Step;
use crate::panel::{Panel, Widget, WidgetOption, Widgets};
use serde_json::Value;
use std::borrow::Cow;

pub const PROJECT_BIND: &str = "@project:";
pub const RACK_BIND: &str = "@rack:";
pub const RACK_ADD_BIND: &str = "@rack+";
pub const CAMERA_BIND: &str = "@camera";
pub const DEVICE_BIND: &str = "@device";
pub const BASE_BIND: &str = "@base";
pub const SHADER_BIND: &str = "@shader";
pub const OUTPUT_VOLUME_PARAM: &str = "@output-volume";
pub const VOLUME_RANGE: (f64, f64) = (-60.0, 12.0);
pub const TAP_COLS: f64 = 2.0;
pub const TAP_ROWS: f64 = 2.0;
pub const PADS_ACROSS: f64 = 4.0;
pub const PADS_UP: f64 = 3.0;
const RACK_COLS: f64 = 4.0;
const RACK_MIN: f64 = 2.0;
const FIRST_RACK_NOTE: f64 = 36.0;

fn option(label: &str, value: f64) -> WidgetOption {
    WidgetOption {
        label: label.to_owned(),
        value,
        arg: None,
    }
}

fn option_arg(label: &str, value: f64, arg: &str) -> WidgetOption {
    WidgetOption {
        arg: Some(arg.to_owned()),
        ..option(label, value)
    }
}

fn dropdown(col: f64, row: f64, col_span: Option<f64>, bind: &str, label: &str, options: Vec<WidgetOption>) -> Widget {
    Widget {
        col_span,
        options: Some(options),
        ..Widget::at("dropdown", col, row).bound(bind, label)
    }
}

fn monitor_options() -> Vec<WidgetOption> {
    vec![option("off", 0.0), option("auto", 1.0), option("on", 2.0)]
}

/// The built-in panel every lane card carries: five mixer controls on `@` binds.
pub fn track_ui() -> Panel {
    let button = |col, bind, label| Widget::at("button", col, 0.0).bound(bind, label);
    Panel::new(
        3.0,
        2.0,
        vec![
            ("mute", button(0.0, "@mute", "M")),
            ("solo", button(1.0, "@solo", "S")),
            ("arm", button(2.0, "@arm", "A")),
            (
                "vol",
                Widget::at("knob", 0.0, 1.0)
                    .bound(OUTPUT_VOLUME_PARAM, "vol")
                    .range(-60.0, 6.0),
            ),
            ("pan", Widget::at("knob", 1.0, 1.0).bound("@pan", "pan").range(0.0, 1.0)),
        ],
    )
}

/// A picture lane's strip: mute and solo, the two words a picture has.
pub fn video_track_ui() -> Panel {
    let button = |col, bind, label| Widget::at("button", col, 0.0).bound(bind, label);
    Panel::new(
        2.0,
        1.0,
        vec![("mute", button(0.0, "@mute", "M")), ("solo", button(1.0, "@solo", "S"))],
    )
}

/// What the render card carries: the frame's shape.
pub fn render_ui() -> Panel {
    let frame = dropdown(
        0.0,
        0.0,
        Some(2.0),
        "@project:aspect",
        "frame",
        vec![option("16:9", 0.0), option("9:16", 1.0)],
    );
    Panel::new(2.0, 1.0, vec![("aspect", frame)])
}

/// The main output's: the song's level on an ordinary bind.
pub fn main_out_ui() -> Panel {
    let fader = Widget {
        row_span: Some(2.0),
        step: Some(0.5),
        ..Widget::at("fader", 0.0, 0.0)
            .bound("Volume", "out")
            .range(VOLUME_RANGE.0, VOLUME_RANGE.1)
    };
    Panel::new(1.0, 2.0, vec![("vol", fader)])
}

/// The shape a pad grid takes for `n` sounds: square-ish, never under 2x2, never over four wide.
pub fn rack_grid(n: usize) -> (f64, f64) {
    let n = n.max(1) as f64;
    let cols = RACK_MIN.max(RACK_COLS.min(n.sqrt().ceil()));
    (cols, RACK_MIN.max((n / cols).ceil()))
}

/// An octave's tab name, by its C: `C2` for note 36.
pub fn octave_name(octave: f64) -> String {
    format!("C{}", octave - 1.0)
}

/// One sound of a rack: the name its pad answers to and the note it plays.
pub struct RackSound {
    pub symbol: String,
    pub note: f64,
}

/// A step's rack, read off its carried `rack` array: the sounds' symbols and notes.
fn rack_of(s: &Step) -> Option<Vec<RackSound>> {
    let Value::Array(sounds) = s.rest.get("rack")? else {
        return None;
    };
    Some(
        sounds
            .iter()
            .map(|v| RackSound {
                symbol: v.get("symbol").and_then(Value::as_str).unwrap_or_default().to_owned(),
                note: v.get("note").and_then(Value::as_f64).unwrap_or(0.0),
            })
            .collect(),
    )
}

/// The panel a rack carries: a tab per octave, each a 4x3 grid of its notes, a pad or an empty cell that adds one.
pub fn rack_ui(rack: &[RackSound], fixed: bool) -> Panel {
    let pad_rows = if fixed { TAP_ROWS + 1.0 } else { TAP_ROWS };
    let mut by_note: Vec<(f64, &RackSound)> = Vec::new();
    for s in rack {
        if !by_note.iter().any(|(n, _)| *n == s.note) {
            by_note.push((s.note, s));
        }
    }
    let mut used: Vec<f64> = Vec::new();
    for s in rack {
        let o = (s.note / 12.0).floor();
        if !used.contains(&o) {
            used.push(o);
        }
    }
    used.sort_by(|a, b| a.total_cmp(b));
    let octaves: Vec<f64> = if fixed {
        used
    } else if let Some(&last) = used.last() {
        let mut all = used.clone();
        let next = 10.0_f64.min(last + 1.0);
        if !all.contains(&next) {
            all.push(next);
        }
        all
    } else {
        vec![(FIRST_RACK_NOTE / 12.0).floor()]
    };
    let cell = |note: f64| {
        let i = note.rem_euclid(12.0);
        (
            (i % PADS_ACROSS) * TAP_COLS,
            (PADS_UP - 1.0 - (i / PADS_ACROSS).floor()) * pad_rows,
        )
    };
    let tabs: Vec<crate::panel::Tab> = octaves
        .iter()
        .map(|&o| {
            let mut widgets = Widgets::new();
            let mut note = o * 12.0;
            while note < o * 12.0 + 12.0 && note <= 127.0 {
                let (col, row) = cell(note);
                let found = by_note.iter().find(|(n, _)| *n == note).map(|(_, s)| *s);
                if let Some(s) = found {
                    let tap = Widget::at("tap", col, row)
                        .span(TAP_COLS, TAP_ROWS)
                        .bound(&format!("{RACK_BIND}{}", s.symbol), &s.symbol);
                    widgets.insert(format!("rack:{}", s.symbol), tap);
                    if fixed {
                        let gain = Widget {
                            step: Some(1.0),
                            ..Widget::at("number", col, row + TAP_ROWS)
                                .span(TAP_COLS, 1.0)
                                .bound(&format!("Key {}", s.note), "dB")
                                .range(-24.0, 12.0)
                        };
                        widgets.insert(format!("kit:gain:{}", s.symbol), gain);
                    }
                } else if !fixed {
                    let add = Widget {
                        bind: Some(format!("{RACK_ADD_BIND}{note}")),
                        ..Widget::at("tap", col, row).span(TAP_COLS, TAP_ROWS)
                    };
                    widgets.insert(format!("rack+:{note}"), add);
                }
                note += 1.0;
            }
            crate::panel::Tab {
                name: octave_name(o),
                widgets,
                ..Default::default()
            }
        })
        .collect();
    let grid_cols = PADS_ACROSS * TAP_COLS;
    let mut widgets = Widgets::new();
    if fixed {
        let volume = Widget {
            label: Some("Volume".to_owned()),
            bind: Some("Volume".to_owned()),
            ..Widget::at("knob", grid_cols, 0.0)
                .span(TAP_COLS, TAP_ROWS)
                .range(-40.0, 12.0)
        };
        widgets.insert("kit:volume".to_owned(), volume);
    }
    let cols = grid_cols + if fixed { TAP_COLS } else { 0.0 };
    let rows = PADS_UP * pad_rows;
    if tabs.len() == 1 {
        let mut tabs = tabs;
        widgets.extend(tabs.remove(0).widgets);
        return Panel {
            cols,
            rows,
            widgets,
            ..Panel::default()
        };
    }
    Panel {
        cols,
        rows,
        widgets,
        tabs: Some(tabs),
        ..Panel::default()
    }
}

/// How the machine's interface groups its inputs: a name and the channels it carries.
#[derive(Clone, Debug, Default, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct InputDevice {
    pub name: String,
    #[serde(default)]
    pub channels: Vec<f64>,
}

fn join(channels: &[f64]) -> String {
    channels.iter().map(f64::to_string).collect::<Vec<_>>().join(",")
}

/// The panel a live input carries: its socket, its width, its monitoring.
pub fn input_ui(devices: &[InputDevice], channel: Option<&[f64]>) -> Panel {
    let stereo = channel.is_some_and(|c| c.len() == 2);
    let first = channel.and_then(|c| c.first().copied()).unwrap_or(1.0);
    let odd = if first.rem_euclid(2.0) == 1.0 {
        first
    } else {
        first - 1.0
    };
    let want = if stereo { 2 } else { 1 };
    let sets: Vec<WidgetOption> = devices
        .iter()
        .filter(|d| d.channels.len() == want)
        .enumerate()
        .map(|(i, d)| option_arg(&d.name, i as f64, &join(&d.channels)))
        .collect();
    let width = vec![
        option_arg("mono", 0.0, &first.to_string()),
        option_arg("stereo", 1.0, &format!("{odd},{}", odd + 1.0)),
    ];
    Panel::new(
        2.0,
        2.0,
        vec![
            ("channel", dropdown(0.0, 0.0, Some(2.0), "@channel", "in", sets)),
            ("width", dropdown(0.0, 1.0, None, "@channel", "width", width)),
            (
                "monitor",
                dropdown(1.0, 1.0, None, "@monitor", "mon", monitor_options()),
            ),
        ],
    )
}

fn named_options(first: &str, names: &[String]) -> Vec<WidgetOption> {
    let mut out = vec![option_arg(first, 0.0, first)];
    out.extend(names.iter().enumerate().map(|(i, n)| option_arg(n, (i + 1) as f64, n)));
    out
}

/// A lens card's panel: one dropdown of the browser's cameras, and the monitor.
pub fn camera_ui(cameras: &[String]) -> Panel {
    Panel::new(
        2.0,
        2.0,
        vec![
            (
                "camera",
                dropdown(
                    0.0,
                    0.0,
                    Some(2.0),
                    CAMERA_BIND,
                    "cam",
                    named_options("default", cameras),
                ),
            ),
            (
                "monitor",
                dropdown(0.0, 1.0, Some(2.0), "@monitor", "mon", monitor_options()),
            ),
        ],
    )
}

/// A MIDI input card's panel: one dropdown of the browser's devices.
pub fn midi_in_ui(devices: &[String]) -> Panel {
    Panel::new(
        3.0,
        1.0,
        vec![(
            "device",
            dropdown(
                0.0,
                0.0,
                Some(3.0),
                DEVICE_BIND,
                "device",
                named_options("any", devices),
            ),
        )],
    )
}

/// A folder-backed dropdown's options: `none` first, then the names in the order given (the page sorts them for people).
fn with_folder_options<'a>(ui: Cow<'a, Panel>, bind: &str, names: Option<&[String]>) -> Cow<'a, Panel> {
    if !ui.all_widgets().any(|(_, w)| w.bind.as_deref() == Some(bind)) {
        return ui;
    }
    let mut options = vec![option("none", 0.0)];
    options.extend(
        names
            .unwrap_or_default()
            .iter()
            .enumerate()
            .map(|(i, n)| option(n, (i + 1) as f64)),
    );
    let fill = |ws: &Widgets| -> Widgets {
        ws.iter()
            .map(|(k, w)| {
                let w = if w.bind.as_deref() == Some(bind) {
                    Widget {
                        options: Some(options.clone()),
                        ..w.clone()
                    }
                } else {
                    w.clone()
                };
                (k.clone(), w)
            })
            .collect()
    };
    Cow::Owned(Panel {
        widgets: fill(&ui.widgets),
        tabs: ui.tabs.as_ref().map(|ts| {
            ts.iter()
                .map(|t| crate::panel::Tab {
                    widgets: fill(&t.widgets),
                    ..t.clone()
                })
                .collect()
        }),
        ..ui.into_owned()
    })
}

/// What the host tells the build about the machine: the cards whose panel lists it.
#[derive(Clone, Copy, Default)]
pub struct Machine<'a> {
    pub inputs: &'a [InputDevice],
    pub cameras: &'a [String],
    pub midi: &'a [String],
}

/// The panel a step actually draws, so the card's size and its `ui` cannot disagree.
pub fn step_ui<'a>(s: &'a Step, m: Machine) -> Option<Cow<'a, Panel>> {
    if let Some(ui) = &s.ui {
        let with_bases = with_folder_options(Cow::Borrowed(ui), BASE_BIND, s.bases.as_deref());
        return Some(with_folder_options(with_bases, SHADER_BIND, s.shaders.as_deref()));
    }
    if let Some(rack) = rack_of(s) {
        return Some(Cow::Owned(rack_ui(
            &rack,
            s.rest.get("rackKit") == Some(&Value::Bool(true)),
        )));
    }
    if s.camera.is_some() {
        return Some(Cow::Owned(camera_ui(m.cameras)));
    }
    if s.device.is_some() {
        return Some(Cow::Owned(midi_in_ui(m.midi)));
    }
    if s.monitor.is_some() {
        let channel: Option<Vec<f64>> = match s.rest.get("channel") {
            Some(Value::Array(a)) => Some(a.iter().filter_map(Value::as_f64).collect()),
            _ => None,
        };
        return Some(Cow::Owned(input_ui(m.inputs, channel.as_deref())));
    }
    None
}

#[cfg(test)]
mod tests;
