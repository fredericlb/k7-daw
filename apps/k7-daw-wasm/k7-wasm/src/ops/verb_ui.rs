// ops/verb_ui.rs — the `ui.*` operations: the command bar's, the menus' and the edits' rules, over the `verbs` crate's `ui` module.
// A reader would get wrong that these read the project: every input arrives in the request, and an answer is an index or a small value.

use crate::error::WasmError;
use crate::ops::write;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use verbs::ui::fill::{self, CaretAim, EnterAction, FormArg, Offered};
use verbs::ui::fuzzy;
use verbs::ui::prefs::{self, Like, View};
use verbs::ui::rank::{self, ArgOption, Completion, EmptyBar, EmptyRow, RankCommands, RankOptions};
use verbs::ui::step::{self, Step, StepCommand};

/// A list of strings to score against one query.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FuzzyArgs {
    pub q: String,
    pub strings: Vec<String>,
}

/// Each string's score, in order; -1 is no match.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Scores {
    pub scores: Vec<f64>,
}

/// The positions of the best matches in the list asked about, best first.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Indices {
    pub indices: Vec<usize>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CompletionsArgs {
    pub full: Vec<Completion>,
    pub prefix: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FormArgs {
    pub args: Vec<FormArg>,
    #[serde(default)]
    pub values: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ValuesArgs {
    pub values: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NextArgs {
    pub args: Vec<FormArg>,
    pub from: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct OfferArgs {
    pub args: Vec<FormArg>,
    pub offered: HashMap<String, String>,
    pub values: Vec<String>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub active: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CaretArgs {
    pub canonical: String,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub caret: Option<CaretAim>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct KindArgs {
    pub args: Vec<FormArg>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub active: i64,
    pub kind: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct EnterArgs {
    pub args: Vec<FormArg>,
    pub values: Vec<String>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub active: i64,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub suggestion: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StepArgs {
    pub label: String,
    pub commands: Vec<StepCommand>,
    pub locale: String,
}

/// A catalogue of commands with the person's arrangement of it.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PrefsArgs {
    pub commands: Vec<Like>,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
    pub prefs: Map<String, Value>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub locale: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MoveArgs {
    pub commands: Vec<Like>,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
    pub prefs: Map<String, Value>,
    pub canonical: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub by: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SetPrefArgs {
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
    pub prefs: Map<String, Value>,
    pub canonical: String,
    /// A field set to null is taken back, as an undefined was.
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
    pub patch: Map<String, Value>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SetMenuArgs {
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
    pub prefs: Map<String, Value>,
    pub canonical: String,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub catalogue: Option<Vec<String>>,
    pub next: Vec<String>,
    pub all: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SetLabelArgs {
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
    pub prefs: Map<String, Value>,
    pub canonical: String,
    pub locale: String,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MenuChoiceArgs {
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>", optional))]
    pub pref: Option<Map<String, Value>>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub catalogue: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct InMenuArgs {
    pub canonical: String,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
    pub prefs: Map<String, Value>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub entity: Option<String>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub catalogue: Option<Vec<String>>,
}

/// What a menu choice says: the entities, or none meaning "wherever it fits".
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MenuChoiceAnswer {
    pub entities: Option<Vec<String>>,
}

/// The prefs after a change.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PrefsAnswer {
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
    pub prefs: Map<String, Value>,
}

/// The `ui.*` requests.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum UiRequest {
    #[serde(rename = "ui.fuzzy_scores")]
    FuzzyScores(FuzzyArgs),
    #[serde(rename = "ui.rank_commands")]
    RankCommands(RankCommands),
    #[serde(rename = "ui.rank_options")]
    RankOptions(RankOptions),
    #[serde(rename = "ui.options_from_completions")]
    OptionsFromCompletions(CompletionsArgs),
    #[serde(rename = "ui.empty_bar")]
    EmptyBar(EmptyBar),
    #[serde(rename = "ui.required_count")]
    RequiredCount(FormArgs),
    #[serde(rename = "ui.empty_values")]
    EmptyValues(FormArgs),
    #[serde(rename = "ui.can_run")]
    CanRun(FormArgs),
    #[serde(rename = "ui.line_args")]
    LineArgs(ValuesArgs),
    #[serde(rename = "ui.next_field")]
    NextField(NextArgs),
    #[serde(rename = "ui.first_missing")]
    FirstMissing(FormArgs),
    #[serde(rename = "ui.apply_offer")]
    ApplyOffer(OfferArgs),
    #[serde(rename = "ui.caret_answers")]
    CaretAnswers(CaretArgs),
    #[serde(rename = "ui.wants_kind")]
    WantsKind(KindArgs),
    #[serde(rename = "ui.enter_action")]
    EnterAction(EnterArgs),
    #[serde(rename = "ui.step_sentence")]
    StepSentence(StepArgs),
    #[serde(rename = "ui.prefs_view")]
    PrefsView(PrefsArgs),
    #[serde(rename = "ui.order_commands")]
    OrderCommands(PrefsArgs),
    #[serde(rename = "ui.move_command")]
    MoveCommand(MoveArgs),
    #[serde(rename = "ui.set_command_pref")]
    SetCommandPref(SetPrefArgs),
    #[serde(rename = "ui.set_menu_entities")]
    SetMenuEntities(SetMenuArgs),
    #[serde(rename = "ui.set_command_label")]
    SetCommandLabel(SetLabelArgs),
    #[serde(rename = "ui.menu_choice")]
    MenuChoice(MenuChoiceArgs),
    #[serde(rename = "ui.in_menu")]
    InMenu(InMenuArgs),
}

pub fn dispatch(request: UiRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    match request {
        UiRequest::FuzzyScores(a) => write(
            out,
            &Scores {
                scores: fuzzy::fuzzy_scores(&a.q, &a.strings),
            },
        ),
        UiRequest::RankCommands(a) => write(
            out,
            &Indices {
                indices: rank::rank_commands(&a),
            },
        ),
        UiRequest::RankOptions(a) => write(
            out,
            &Indices {
                indices: rank::rank_options(&a),
            },
        ),
        UiRequest::OptionsFromCompletions(a) => {
            let options: Vec<ArgOption> = rank::options_from_completions(&a.full, &a.prefix);
            write(out, &options)
        }
        UiRequest::EmptyBar(a) => {
            let rows: Vec<EmptyRow> = rank::empty_bar_rows(&a);
            write(out, &rows)
        }
        UiRequest::RequiredCount(a) => write(out, &fill::required_count(&a.args)),
        UiRequest::EmptyValues(a) => write(out, &fill::empty_values(&a.args)),
        UiRequest::CanRun(a) => write(out, &fill::can_run(&a.args, &a.values)),
        UiRequest::LineArgs(a) => write(out, &fill::line_args(&a.values)),
        UiRequest::NextField(a) => write(out, &fill::next_field(&a.args, a.from)),
        UiRequest::FirstMissing(a) => write(out, &fill::first_missing(&a.args, &a.values)),
        UiRequest::ApplyOffer(a) => {
            let got: Option<Offered> = fill::apply_offer(&a.args, &a.offered, &a.values, a.active);
            write(out, &got)
        }
        UiRequest::CaretAnswers(a) => write(out, &fill::caret_answers(&a.canonical, a.caret.as_ref())),
        UiRequest::WantsKind(a) => write(out, &fill::wants_kind(&a.args, a.active, &a.kind)),
        UiRequest::EnterAction(a) => {
            let act: EnterAction = fill::enter_action(&a.args, &a.values, a.active, a.suggestion.as_deref());
            write(out, &act)
        }
        UiRequest::StepSentence(a) => {
            let s: Step = step::step_sentence(&a.label, &a.commands, &a.locale);
            write(out, &s)
        }
        UiRequest::PrefsView(a) => {
            let v: View = prefs::view(&a.commands, &a.prefs, a.locale.as_deref().unwrap_or("en"));
            write(out, &v)
        }
        UiRequest::OrderCommands(a) => write(
            out,
            &Indices {
                indices: prefs::order_commands(&a.commands, &a.prefs),
            },
        ),
        UiRequest::MoveCommand(a) => write(
            out,
            &PrefsAnswer {
                prefs: prefs::move_command(&a.commands, &a.prefs, &a.canonical, a.by),
            },
        ),
        UiRequest::SetCommandPref(a) => write(
            out,
            &PrefsAnswer {
                prefs: prefs::set_command_pref(&a.prefs, &a.canonical, &a.patch),
            },
        ),
        UiRequest::SetMenuEntities(a) => write(
            out,
            &PrefsAnswer {
                prefs: prefs::set_menu_entities(&a.prefs, &a.canonical, a.catalogue.as_deref(), &a.next, &a.all),
            },
        ),
        UiRequest::SetCommandLabel(a) => write(
            out,
            &PrefsAnswer {
                prefs: prefs::set_command_label(&a.prefs, &a.canonical, &a.locale, &a.text),
            },
        ),
        UiRequest::InMenu(a) => write(
            out,
            &prefs::in_menu(&a.canonical, &a.prefs, a.entity.as_deref(), a.catalogue.as_deref()),
        ),
        UiRequest::MenuChoice(a) => write(
            out,
            &MenuChoiceAnswer {
                entities: prefs::menu_choice(a.pref.as_ref(), a.catalogue.as_deref()),
            },
        ),
    }
}
