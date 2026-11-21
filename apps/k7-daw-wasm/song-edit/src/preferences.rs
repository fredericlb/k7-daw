// preferences.rs — the user's GLOBAL preferences (`<state>/preferences.k7x`): the audio interface, channel labels, editor settings.
// A reader would get wrong that a file which does not read is never saved over: every mutator refuses with the problem, so no write replaces it with defaults.

use crate::error::Res;
use crate::js_text::{is_positive_integer, js_string, js_to_number, js_trim};
use k7x::edit::{StringifyOptions, Value, js_number, parse, stringify};

pub const PREFERENCES_FILE: &str = "preferences.k7x";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelDir {
    In,
    Out,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChannelLabel {
    pub ch: Vec<f64>,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioDevicePref {
    pub name: String,
    pub kind: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CueOutput {
    Label(String),
    Channels(Vec<f64>),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Preferences {
    pub audio_device: Option<AudioDevicePref>,
    pub channels_in: Vec<ChannelLabel>,
    pub channels_out: Vec<ChannelLabel>,
    pub input_pairs: Vec<f64>,
    pub editor_vim: bool,
    pub stems_chain: Option<String>,
    pub instrumental_chain: Option<String>,
    pub cue_output: Option<CueOutput>,
}

impl Preferences {
    pub(crate) fn list(&self, dir: ChannelDir) -> &Vec<ChannelLabel> {
        match dir {
            ChannelDir::In => &self.channels_in,
            ChannelDir::Out => &self.channels_out,
        }
    }

    pub(crate) fn list_mut(&mut self, dir: ChannelDir) -> &mut Vec<ChannelLabel> {
        match dir {
            ChannelDir::In => &mut self.channels_in,
            ChannelDir::Out => &mut self.channels_out,
        }
    }
}

fn nonempty_str(v: Option<&Value>) -> Option<&str> {
    match v {
        Some(Value::Str(s)) if !s.is_empty() => Some(s),
        _ => None,
    }
}

fn trimmed(v: Option<&Value>) -> Option<String> {
    match v {
        Some(Value::Str(s)) if !js_trim(s).is_empty() => Some(js_trim(s).to_owned()),
        _ => None,
    }
}

fn positive_numbers<'a>(items: impl Iterator<Item = Option<&'a Value>>) -> Vec<f64> {
    items.map(js_to_number).filter(|n| is_positive_integer(*n)).collect()
}

/// Coerces a parse into a well-formed Preferences, as the TypeScript's `normalizePreferences` does.
#[must_use]
pub fn normalize_preferences(v: &Value) -> Preferences {
    let mut p = Preferences::default();
    if !matches!(v, Value::Map(_) | Value::List(_)) {
        return p;
    }
    match v.get("audioDevice") {
        Some(Value::Str(d)) if !d.is_empty() => {
            p.audio_device = Some(AudioDevicePref {
                name: d.clone(),
                kind: None,
            });
        }
        Some(dev @ Value::Map(_)) => {
            if let Some(name) = nonempty_str(dev.get("name")) {
                p.audio_device = Some(AudioDevicePref {
                    name: name.to_owned(),
                    kind: nonempty_str(dev.get("type")).map(str::to_owned),
                });
            }
        }
        _ => {}
    }
    for dir in [ChannelDir::In, ChannelDir::Out] {
        let key = if dir == ChannelDir::In { "in" } else { "out" };
        let list = match v.get("channels").and_then(|c| c.get(key)) {
            Some(Value::List(l)) => l.as_slice(),
            _ => &[],
        };
        *p.list_mut(dir) = list
            .iter()
            .map(|e| {
                let ch = e.get("ch");
                let ch = match ch {
                    Some(Value::List(l)) => positive_numbers(l.iter().map(Some)),
                    other => positive_numbers(std::iter::once(other)),
                };
                let label = match e.get("label") {
                    None | Some(Value::Null) => String::new(),
                    Some(x) => js_string(x),
                };
                ChannelLabel { ch, label }
            })
            .filter(|e| !e.ch.is_empty() && !e.label.is_empty())
            .collect();
    }
    if let Some(Value::List(l)) = v.get("inputPairs") {
        let mut odd: Vec<f64> = vec![];
        for n in positive_numbers(l.iter().map(Some)) {
            let n = if n % 2.0 == 1.0 { n } else { n - 1.0 };
            if !odd.contains(&n) {
                odd.push(n);
            }
        }
        odd.sort_by(f64::total_cmp);
        p.input_pairs = odd;
    }
    if let Some(Value::Bool(b)) = v.get("editor").and_then(|e| e.get("vim")) {
        p.editor_vim = *b;
    }
    p.stems_chain = trimmed(v.get("stemsChain"));
    p.instrumental_chain = trimmed(v.get("instrumentalChain"));
    match v.get("cueOutput") {
        Some(Value::List(l)) => {
            let ch = positive_numbers(l.iter().map(Some));
            if !ch.is_empty() {
                p.cue_output = Some(CueOutput::Channels(ch));
            }
        }
        Some(Value::Str(s)) if !js_trim(s).is_empty() => p.cue_output = Some(CueOutput::Label(js_trim(s).to_owned())),
        _ => {}
    }
    p
}

fn labels_value(list: &[ChannelLabel]) -> Value {
    Value::List(
        list.iter()
            .map(|e| {
                Value::object(vec![
                    ("ch".to_owned(), numbers(&e.ch)),
                    ("label".to_owned(), Value::Str(e.label.clone())),
                ])
            })
            .collect(),
    )
}

fn numbers(n: &[f64]) -> Value {
    Value::List(n.iter().map(|x| Value::Num(*x)).collect())
}

/// The data of a Preferences in the key order `normalizePreferences` builds it in.
fn to_value(p: &Preferences) -> Value {
    let s = |x: &String| Value::Str(x.clone());
    let mut out = vec![
        (
            "channels".to_owned(),
            Value::object(vec![
                ("in".to_owned(), labels_value(&p.channels_in)),
                ("out".to_owned(), labels_value(&p.channels_out)),
            ]),
        ),
        ("inputPairs".to_owned(), numbers(&p.input_pairs)),
        (
            "editor".to_owned(),
            Value::object(vec![("vim".to_owned(), Value::Bool(p.editor_vim))]),
        ),
    ];
    if let Some(d) = &p.audio_device {
        let mut dev = vec![("name".to_owned(), s(&d.name))];
        if let Some(t) = &d.kind {
            dev.push(("type".to_owned(), s(t)));
        }
        out.push(("audioDevice".to_owned(), Value::object(dev)));
    }
    if let Some(x) = &p.stems_chain {
        out.push(("stemsChain".to_owned(), s(x)));
    }
    if let Some(x) = &p.instrumental_chain {
        out.push(("instrumentalChain".to_owned(), s(x)));
    }
    match &p.cue_output {
        Some(CueOutput::Label(x)) => out.push(("cueOutput".to_owned(), s(x))),
        Some(CueOutput::Channels(c)) => out.push(("cueOutput".to_owned(), numbers(c))),
        None => {}
    }
    Value::object(out)
}

pub fn parse_strict(text: &str) -> Option<Preferences> {
    match parse(text) {
        Ok(v @ Value::Map(_)) => Some(normalize_preferences(&v)),
        _ => None,
    }
}

/// The k7x text's preferences; a text that does not read is the defaults.
#[must_use]
pub fn parse_preferences(text: &str) -> Preferences {
    parse_strict(text).unwrap_or_default()
}

/// The text written to `preferences.k7x`: normalized, so no read ever meets a stray field.
///
/// # Errors
/// A value the printer refuses.
pub fn stringify_preferences(p: &Preferences) -> Res<String> {
    Ok(stringify(
        &to_value(&normalize_preferences(&to_value(p))),
        &StringifyOptions::default(),
    )?)
}

/// "Focusrite USB (ASIO)", or "(system default)" when nothing is configured.
#[must_use]
pub fn format_audio_device(d: Option<&AudioDevicePref>) -> String {
    match d {
        None => "(system default)".to_owned(),
        Some(d) => d
            .kind
            .as_ref()
            .map_or_else(|| d.name.clone(), |t| format!("{} ({t})", d.name)),
    }
}

/// A channel spec like "3+4" or "1" as channel numbers.
#[must_use]
pub fn parse_channels(spec: &str) -> Vec<f64> {
    spec.split('+')
        .map(|s| {
            let t = js_trim(s);
            let (sign, digits) = match t.as_bytes().first() {
                Some(b'-') => (-1.0, &t[1..]),
                Some(b'+') => (1.0, &t[1..]),
                _ => (1.0, t),
            };
            let run: String = digits.chars().take_while(char::is_ascii_digit).collect();
            run.parse::<f64>().map_or(f64::NAN, |n| sign * n)
        })
        .filter(|n| is_positive_integer(*n))
        .collect()
}

/// "3+4 synth" for a group, "1 guitar" for a single channel.
#[must_use]
pub fn format_channel(e: &ChannelLabel) -> String {
    format!(
        "{} {}",
        e.ch.iter().map(|n| js_number(*n)).collect::<Vec<_>>().join("+"),
        e.label
    )
}

/// A one-line summary per direction: (in, out).
#[must_use]
pub fn format_channels(p: &Preferences) -> (String, String) {
    let fmt = |l: &[ChannelLabel]| l.iter().map(format_channel).collect::<Vec<_>>().join(" · ");
    (fmt(&p.channels_in), fmt(&p.channels_out))
}

/// What a spec resolved to: the physical channels, or the label that names none.
#[derive(Clone, Debug, PartialEq)]
pub struct Resolved {
    pub channels: Vec<f64>,
    pub unknown_label: Option<String>,
}

fn resolve_channels(dir: ChannelDir, spec: Option<&CueOutput>, p: &Preferences) -> Resolved {
    let none = |c: Vec<f64>| Resolved {
        channels: c,
        unknown_label: None,
    };
    match spec {
        None => none(vec![]),
        Some(CueOutput::Channels(c)) => none(c.iter().copied().filter(|n| is_positive_integer(*n)).collect()),
        Some(CueOutput::Label(l)) if l.is_empty() => none(vec![]),
        Some(CueOutput::Label(l)) => match p.list(dir).iter().find(|c| &c.label == l) {
            Some(hit) => none(hit.ch.clone()),
            None => Resolved {
                channels: vec![],
                unknown_label: Some(l.clone()),
            },
        },
    }
}

/// A track's `input` as physical channels; an unresolvable LABEL reports itself.
#[must_use]
pub fn resolve_input(input: Option<&CueOutput>, p: &Preferences) -> Resolved {
    resolve_channels(ChannelDir::In, input, p)
}

/// The `CUE` bus's pair as physical channels, against the OUT labels.
#[must_use]
pub fn resolve_cue_output(p: &Preferences) -> Resolved {
    resolve_channels(ChannelDir::Out, p.cue_output.as_ref(), p)
}
