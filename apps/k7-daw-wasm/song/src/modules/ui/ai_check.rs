// ai_check.rs — the WIRE and the CHECK of AI organize: what the builder sends, the per-module message the model reads, and the test a proposed panel must pass.
// A reader would get wrong that the issues are sentences for the MODEL: each says what to change, and a round of them is handed back once.
// Choices: apps/k7-daw-frontend/packages/ui-module-builder/docs/aiOrganize.md.

use super::ai_script::script_syntax_error;
use super::geometry::{Pairs, framed_by, js_num, twin_issues};
use super::loose::{bind, entries, kind, num, obj, span, text};
use super::rx::Rx;
use crate::doc::{INPUT_VOLUME_PARAM, LevelSide, MIX_PARAM, OUTPUT_VOLUME_PARAM, own_level_side};
use crate::files::validate_file;
use k7x::edit::js_number;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::sync::LazyLock;

/// The bind prefix a rack pad carries: `@rack:BD` fires the sound `BD`. The `chains` crate holds the same two, pinned by a test in `k7-wasm`.
pub const RACK_BIND: &str = "@rack:";
/// The bind of a rack's last tap, which adds a sound rather than playing one.
pub const RACK_ADD_BIND: &str = "@rack+";

/// The symbol a rack-pad bind names, or None for any other bind.
pub fn rack_bind_symbol(bind: Option<&str>) -> Option<&str> {
    bind?.strip_prefix(RACK_BIND)
}

/// One state of a param that lists named settings.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AiState {
    pub label: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub value: serde_json::Number,
}

/// A param's range as the model sees it.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct AiMeta {
    pub min: f64,
    pub max: f64,
    pub def: Option<f64>,
    pub states: Option<Vec<AiState>>,
}

/// One param as the model sees it: its exact name, its range and its current value.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct AiParam {
    pub name: String,
    pub meta: Option<AiMeta>,
    pub value: Option<String>,
}

/// The two things AI organize can be asked to do with a module that already has a panel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum AiMode {
    Update,
    #[default]
    Reset,
}

/// What the builder hands its host: everything the model needs, nothing it could not use.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AiRequest {
    /// The module's name, for the header comment and the model's sense of what it is.
    pub module: String,
    /// The module.k7x text: its plugin, its declared params, its label.
    pub module_text: String,
    pub params: Vec<AiParam>,
    /// The panel (`ui.k7x`) and the screens as they are now: sent to the model only on an update.
    pub ui_text: String,
    pub widgets_js: String,
    pub mode: AiMode,
    /// What the author asked for in their own words.
    pub direction: String,
}

/// What a proposal is judged by beside its own text: what the author asked, and the screens there were.
#[derive(Clone, Debug, Default, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AiContext {
    #[serde(default)]
    pub direction: String,
    #[serde(default)]
    pub widgets_js: String,
    #[serde(default)]
    pub mode: AiMode,
}

/// The most params a request may carry.
const MAX_PARAMS: usize = 2000;
/// The longest direction, in UTF-16 units.
const MAX_DIRECTION: usize = 2000;

/// The request checked: a typed request, or why it is refused.
pub fn parse_request(body: &Value) -> Result<AiRequest, String> {
    let req: AiRequest = serde_json::from_value(body.clone()).map_err(|e| e.to_string())?;
    if req.module.is_empty() {
        return Err("module: expected a non-empty string".into());
    }
    if req.params.len() > MAX_PARAMS {
        return Err(format!("params: expected at most {MAX_PARAMS} params"));
    }
    if req.params.iter().any(|p| p.name.is_empty()) {
        return Err("params: every param has a name".into());
    }
    if req.direction.encode_utf16().count() > MAX_DIRECTION {
        return Err(format!("direction: expected at most {MAX_DIRECTION} characters"));
    }
    // An update with nothing to change would pay a minute of the account's Claude for the panel it has.
    if req.mode == AiMode::Update && req.direction.trim_matches(crate::files::scalar::js_space).is_empty() {
        return Err("direction: an update says what should change".into());
    }
    Ok(req)
}

/// Whether a panel file lays anything out: the question that decides whether there is a layout to update.
pub fn has_layout(ui_text: &str) -> bool {
    // A panel that does not even parse is nothing to update: a reset is what it needs.
    k7x::edit::parse(ui_text)
        .ok()
        .and_then(|v| match v.get("widgets") {
            Some(k7x::edit::Value::Map(m)) => Some(!m.is_empty()),
            _ => None,
        })
        .unwrap_or(false)
}

/// The sounds module.k7x's own `rack(…)` declares, by symbol; none when it has no rack or does not parse.
pub fn rack_symbols(module_text: &str) -> Vec<String> {
    let Ok(doc) = k7x::edit::parse(module_text) else {
        return Vec::new();
    };
    match doc.get("rack").and_then(|r| r.get("symbols")) {
        Some(k7x::edit::Value::Map(m)) => m.iter().map(|(k, _)| k.clone()).collect(),
        _ => Vec::new(),
    }
}

/// Every bind a proposed panel may carry: the params, and on a rack module its taps.
pub fn bindable_names(params: &[AiParam], module_text: &str) -> Vec<String> {
    let rack = rack_symbols(module_text);
    let mut out: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
    out.extend(rack.iter().map(|s| format!("{RACK_BIND}{s}")));
    if !rack.is_empty() {
        out.push(RACK_ADD_BIND.to_owned());
    }
    out
}

fn json_text(v: &impl Serialize) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

/// A param's one line: name, range or settings, and the value, which says where it usually sits.
fn param_line(p: &AiParam) -> String {
    let range = match &p.meta {
        Some(m) if m.states.as_ref().is_some_and(|s| !s.is_empty()) => format!("states: {}", json_text(&m.states)),
        Some(m) => {
            let def = m.def.map(|d| format!(", default {}", js_number(d))).unwrap_or_default();
            format!("range {}..{}{def}", js_number(m.min), js_number(m.max))
        }
        None => "range unknown (use 0..1)".to_owned(),
    };
    let value = p.value.as_ref().map(|v| format!(", value {v}")).unwrap_or_default();
    format!("- {}: {range}{value}", json_text(&p.name))
}

fn or_empty<'a>(s: &'a str, fallback: &'a str) -> &'a str {
    let t = s.trim_matches(crate::files::scalar::js_space);
    if t.is_empty() { fallback } else { t }
}

/// The per-module half of the prompt: everything that changes between two requests.
pub fn ai_user_prompt(req: &AiRequest) -> String {
    let direction = req.direction.trim_matches(crate::files::scalar::js_space);
    let rack = rack_symbols(&req.module_text);
    let mut out: Vec<String> = vec![
        format!("Module: {}", req.module),
        String::new(),
        "## module.k7x".into(),
        "```k7x".into(),
        or_empty(&req.module_text, "// (empty)").to_owned(),
        "```".into(),
        String::new(),
        format!("## Parameters ({}) — bind names exactly as quoted", req.params.len()),
    ];
    out.extend(req.params.iter().map(param_line));
    out.push(String::new());
    if !rack.is_empty() {
        out.push(format!(
            "## Rack sounds ({}) — one `tap` each, bound `{RACK_BIND}<symbol>`, plus the `{RACK_ADD_BIND}` tap",
            rack.len()
        ));
        out.extend(
            rack.iter()
                .map(|s| format!("- {}", json_text(&format!("{RACK_BIND}{s}")))),
        );
        out.push(String::new());
    }
    // An UPDATE shows the panel there is; a RESET leaves it out, so nothing of it can leak back in.
    if req.mode == AiMode::Update {
        out.extend([
            "## Current ui.k7x".to_owned(),
            "```k7x".into(),
            or_empty(&req.ui_text, "// (none yet)").to_owned(),
            "```".into(),
            String::new(),
            "## Current widgets.js".into(),
            "```js".into(),
            or_empty(&req.widgets_js, "// (none yet)").to_owned(),
            "```".into(),
            String::new(),
            "## Update this panel".into(),
            "Change what the author asks for below and keep everything else as it is — the same widget ids, positions, sizes, binds and screens. Return the whole panel, changed.".into(),
            String::new(),
            format!("## What to change\n{direction}"),
        ]);
    } else {
        out.push("## Design a new panel".into());
        out.push("There is no panel to keep: lay this module out from its parameters alone.".into());
        if !direction.is_empty() {
            out.push(String::new());
            out.push(format!("## What the author asks for\n{direction}"));
        }
    }
    out.join("\n")
}

/// Every way a set of widgets drawn together can break the grid: out of bounds, or on top of another.
fn layout_issues(ui: &Value, place: &str, ws: &Pairs) -> Vec<String> {
    let (cols, rows) = (num(ui, "cols"), num(ui, "rows"));
    let mut out = Vec::new();
    for (id, w) in ws {
        if num(w, "col") + span(w, "colSpan") > cols || num(w, "row") + span(w, "rowSpan") > rows {
            out.push(format!(
                "{place}: `{id}` runs outside the {}×{} grid",
                js_num(cols),
                js_num(rows)
            ));
        }
    }
    for i in 0..ws.len() {
        for j in i + 1..ws.len() {
            if !super::geometry::clash(ws[i].1, ws[j].1) {
                continue;
            }
            let (a, b) = (ws[i].0, ws[j].0);
            out.push(if kind(ws[i].1) == "group" || kind(ws[j].1) == "group" {
                format!("{place}: `{a}` and `{b}` cross — a widget lies wholly inside a group or wholly outside it, and groups never overlap")
            } else {
                format!("{place}: `{a}` overlaps `{b}`")
            });
        }
    }
    out
}

static DRY: LazyLock<Rx> = LazyLock::new(|| Rx::new(r"\bdry\b", true));
static WET: LazyLock<Rx> = LazyLock::new(|| Rx::new(r"\bwet\b", true));
static MIX: LazyLock<Rx> = LazyLock::new(|| Rx::new(r"\bmix\b", true));
static EQ_BAND: LazyLock<Rx> = LazyLock::new(|| {
    Rx::new(
        r"^(low|lo|mid|mids|high|hi|bass|treble|presence|presnce|pres)(\s*-?\s*(shelf|peak|band))?(\s+(gain|level|eq))?(\s*\d+)?$",
        true,
    )
});
static POINTER: LazyLock<Rx> = LazyLock::new(|| Rx::new(r"\bpointer\s*(\(|:)", false));
static INTERACTION: LazyLock<Rx> = LazyLock::new(|| {
    Rx::new(
        r"interacti|\bdrag|\bclick|\btouch|\bpointer|\bmouse|\bgrab|editable|glisse|clique|souris|déplac|éditable",
        true,
    )
});

/// A module with a dry and a wet level and no mix of its own blends through ONE crossfade knob.
pub fn crossfade_issues(widgets: &Pairs, bind_names: &[String]) -> Vec<String> {
    let own: Vec<&String> = bind_names.iter().filter(|n| !n.starts_with('@')).collect();
    // "Dry/Wet" names both sides at once: that is a mix param, and the module needs no crossfade.
    if own.iter().any(|n| MIX.test(n) || (DRY.test(n) && WET.test(n))) {
        return Vec::new();
    }
    let (Some(dry), Some(wet)) = (own.iter().find(|n| DRY.test(n)), own.iter().find(|n| WET.test(n))) else {
        return Vec::new();
    };
    let fix = format!("one crossfade knob — `bind: {wet}, pair: {dry}`");
    fn bound(w: &Value) -> Option<&str> {
        bind(w)
    }
    fn pair(w: &Value) -> Option<&str> {
        text(w, "pair")
    }
    let dry_on = widgets.iter().find(|(_, w)| bound(w) == Some(dry.as_str()));
    let wet_on = widgets
        .iter()
        .find(|(_, w)| bound(w) == Some(wet.as_str()) && pair(w) != Some(dry.as_str()));
    let mut out = Vec::new();
    if let (Some(d), Some(w)) = (dry_on, wet_on) {
        out.push(format!(
            "`{}` and `{}` set `{dry}` and `{wet}` apart: blend them with {fix}",
            d.0, w.0
        ));
    }
    for (id, w) in widgets {
        if bound(w) == Some(dry.as_str()) && pair(w) == Some(wet.as_str()) {
            out.push(format!("`{id}` is a crossfade knob the wrong way round: {fix}"));
        }
    }
    out
}

/// Where the engine offers `@mix`, the blend is THAT knob and the plugin's own mix, dry and wet stay off.
pub fn blend_issues(widgets: &Pairs) -> Vec<String> {
    let own = |n: &str| !n.starts_with('@') && (MIX.test(n) || DRY.test(n) || WET.test(n));
    let mut out = Vec::new();
    for (id, w) in widgets {
        let pair = text(w, "pair").filter(|p| !p.is_empty());
        if pair.is_some() || bind(w).is_some_and(own) {
            let what = if pair.is_some() {
                "dry and wet".to_owned()
            } else {
                format!("`{}`", bind(w).unwrap_or_default())
            };
            out.push(format!(
                "`{id}` blends with the plugin's own {what}: the blend is the engine's `{MIX_PARAM}`, drawn in the card's footer — leave it off the panel"
            ));
        }
    }
    out
}

/// Where the engine offers a level, the plugin's own input or output level stays off the panel.
pub fn level_issues(widgets: &Pairs, bind_names: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for (id, w) in widgets {
        let Some(b) = bind(w).filter(|b| !b.starts_with('@')) else {
            continue;
        };
        let (side, engine) = match own_level_side(b) {
            Some(LevelSide::Input) => ("in", INPUT_VOLUME_PARAM),
            Some(LevelSide::Output) => ("out", OUTPUT_VOLUME_PARAM),
            None => continue,
        };
        if bind_names.iter().any(|n| n == engine) {
            out.push(format!(
                "`{id}` binds the plugin's own `{b}`: the level {side} is the engine's `{engine}`, drawn in the card's footer — leave it off the panel"
            ));
        }
    }
    out
}

/// An EQ band is a fader, never a knob: side by side, the faders draw the curve.
pub fn eq_band_issues(widgets: &Pairs) -> Vec<String> {
    widgets
        .iter()
        .filter(|(_, w)| {
            kind(w) == "knob" && bind(w).is_some_and(|b| EQ_BAND.test(b.trim_matches(crate::files::scalar::js_space)))
        })
        .map(|(id, w)| {
            format!(
                "`{id}` is a knob on `{}`, an EQ band: make it a fader (1 column × 3+ rows) beside the other bands",
                bind(w).unwrap_or_default()
            )
        })
        .collect()
}

/// No control on an `@` param: k7 draws the engine's stage in the card's footer, a lane's on its strip.
pub fn at_control_issues(widgets: &Pairs) -> Vec<String> {
    widgets
        .iter()
        .filter(|(_, w)| {
            matches!(kind(w), "knob" | "slider" | "fader" | "number")
                && bind(w)
                    .is_some_and(|b| b.starts_with('@') && b != RACK_ADD_BIND && rack_bind_symbol(Some(b)).is_none())
        })
        .map(|(id, w)| {
            format!(
                "`{id}` puts a {} on `{}`: k7 draws that in the card's footer — leave `@` params off the panel",
                kind(w),
                bind(w).unwrap_or_default()
            )
        })
        .collect()
}

/// A group frames a SECTION: one around a single widget is a tint, not a grouping.
pub fn lone_group_issues(widgets: &Pairs) -> Vec<String> {
    let map: super::loose::Obj = widgets.iter().map(|(id, w)| ((*id).clone(), (*w).clone())).collect();
    let mut out = Vec::new();
    for (id, w) in widgets {
        if kind(w) != "group" {
            continue;
        }
        let n = framed_by(&map, w).len();
        if n < 2 {
            out.push(format!(
                "group `{id}` frames {}: a group holds two or more — drop it{}",
                if n == 0 { "nothing" } else { "a single widget" },
                if n > 0 { " and leave the widget bare" } else { "" }
            ));
        }
    }
    out
}

/// A draw function for `id` in widgets.js: `id(lcd) {`, `id: (lcd) =>`, `"id": function…`.
pub fn draws_for(script: &str, id: &str) -> bool {
    let mut escaped = String::new();
    for c in id.chars() {
        if ".*+?^${}()|[]\\".contains(c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    Rx::new(&format!(r#"["']?\b{escaped}\b["']?\s*(\(|:)"#), false).test(script)
}

/// Does the author's direction ask for a screen you can touch? Words, in English or French.
pub fn asks_for_interaction(direction: &str) -> bool {
    INTERACTION.test(direction)
}

/// A screen takes the pointer only when the author asked, or an update keeps one that already did.
pub fn interactive_issues(script: &str, ctx: &AiContext) -> Vec<String> {
    if !POINTER.test(script) || asks_for_interaction(&ctx.direction) {
        return Vec::new();
    }
    if ctx.mode == AiMode::Update && POINTER.test(&ctx.widgets_js) {
        return Vec::new();
    }
    vec!["widgets_js gives a screen a `pointer` handler, and the author did not ask for an interactive screen: make every screen a plain draw function".into()]
}

/// What a proposal is: both files as text, and what turning the model's answer into them could not keep.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct AiResult {
    pub ui_text: String,
    pub widgets_js: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub issues: Vec<String>,
}

/// A proposal judged: the panel read when it can be, and why it must not land.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Checked {
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").WebModuleUI | null"))]
    pub ui: Option<Value>,
    pub issues: Vec<String>,
}

fn pairs_of(m: Option<&super::loose::Obj>) -> Pairs<'_> {
    entries(m)
}

/// Why a proposal must not land, as sentences the MODEL can act on; empty means it may.
pub fn check_ai_result(res: &AiResult, bind_names: &[String], ctx: &AiContext) -> Checked {
    let raw = match k7x::edit::parse(&res.ui_text) {
        Ok(v) => super::write::from_k7(&v),
        Err(e) => {
            return Checked {
                ui: None,
                issues: vec![format!("the panel does not read as k7x: {e}")],
            };
        }
    };
    let verdict = validate_file("modules/audio/x/ui.k7x", &res.ui_text);
    if !verdict.ok {
        let issues = verdict
            .issues
            .iter()
            .map(|i| {
                format!(
                    "panel {}: {}",
                    if i.path.is_empty() { "(root)" } else { &i.path },
                    i.message
                )
            })
            .collect();
        return Checked { ui: None, issues };
    }
    let ui = raw;
    let mut issues: Vec<String> = res.issues.clone();
    let known: HashSet<&str> = bind_names.iter().map(String::as_str).collect();
    let top = pairs_of(obj(&ui, "widgets"));
    let tabs: Vec<&Value> = ui.get("tabs").and_then(Value::as_array).into_iter().flatten().collect();
    // The panel's own widgets are drawn on EVERY tab, so each tab is checked together with them.
    let mut all: Pairs = top.clone();
    issues.extend(layout_issues(&ui, "panel", &top));
    for t in &tabs {
        let in_tab = pairs_of(obj(t, "widgets"));
        all.extend(in_tab.iter().copied());
        let mut together = top.clone();
        together.extend(in_tab);
        let name = text(t, "name").unwrap_or("undefined");
        issues.extend(layout_issues(&ui, &format!("tab {name}"), &together));
    }
    let mut seen: HashSet<&str> = HashSet::new();
    for (id, w) in &all {
        if !seen.insert(id.as_str()) {
            issues.push(format!("widget id `{id}` is used twice"));
        }
        let pad = bind(w) == Some(RACK_ADD_BIND) || rack_bind_symbol(bind(w)).is_some();
        // A pad is a tap's and a tap is a pad: either one alone draws a card that plays nothing.
        if kind(w) == "tap" && !pad {
            issues.push(format!("tap `{id}` must bind a rack sound or `{RACK_ADD_BIND}`"));
        } else if pad && kind(w) != "tap" {
            issues.push(format!("`{id}` binds a rack sound, which only a tap may"));
        } else if let Some(b) = bind(w).filter(|b| !known.contains(b)) {
            issues.push(if pad {
                format!("tap `{id}` binds `{b}`, which is not a sound of this module's rack")
            } else {
                format!("`{id}` binds `{b}`, which is not a parameter")
            });
        }
        if let Some(p) = text(w, "pair").filter(|p| !p.is_empty() && !known.contains(p)) {
            issues.push(format!("`{id}` pairs `{p}`, which is not a parameter"));
        }
        if kind(w) == "lcdscreen" && !draws_for(&res.widgets_js, id) {
            issues.push(format!("lcdscreen `{id}` has no draw function in widgets_js"));
        }
    }
    issues.extend(if bind_names.iter().any(|n| n == MIX_PARAM) {
        blend_issues(&all)
    } else {
        crossfade_issues(&all, bind_names)
    });
    // The panel's two shapes: sizes, the vertical one's geometry, the same controls on every page.
    issues.extend(twin_issues(&ui));
    issues.extend(lone_group_issues(&top));
    issues.extend(at_control_issues(&all));
    issues.extend(eq_band_issues(&all));
    issues.extend(level_issues(&all, bind_names));
    for t in &tabs {
        issues.extend(lone_group_issues(&pairs_of(obj(t, "widgets"))));
    }
    if !res.widgets_js.trim_matches(crate::files::scalar::js_space).is_empty()
        && let Some(err) = script_syntax_error(&res.widgets_js)
    {
        issues.push(format!("widgets_js does not compile: {err}"));
    }
    issues.extend(interactive_issues(&res.widgets_js, ctx));
    Checked { ui: Some(ui), issues }
}

#[cfg(test)]
#[path = "ai_check_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "ai_cases_tests.rs"]
mod cases_tests;

#[cfg(test)]
#[path = "ai_rules_tests.rs"]
mod rules_tests;
