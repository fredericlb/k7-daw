// ops/panel.rs — the panel.* operations: the module builder's rules over a panel as loose JSON (geometry, tabs, clipboard, kinds, auto layout).
// A reader would get wrong that a panel here is not validated: these rules read what the grid holds mid-edit; the loader's reader is `validate.panel`.

use crate::error::WasmError;
use crate::ops::write;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};
use song::modules::ui::ai_answer::read_ai_answer;
use song::modules::ui::ai_check::{self, AiContext, AiParam, AiRequest, AiResult};
use song::modules::ui::auto::{self, AutoParam};
use song::modules::ui::edit::{self, Clip};
use song::modules::ui::geometry::{self, Cell};
use song::modules::ui::loose::Obj;
use song::modules::ui::rows::{self, RowSources};
use song::modules::ui::{ai_prompt, check, write};

/// A page ref: absent for "nothing chosen", null for the panel's own widgets, a number for a tab.
fn page_ref<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Option<usize>>, D::Error> {
    Ok(Some(Option::<usize>::deserialize(d)?))
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TwoWidgets {
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").WebWidget"))]
    pub a: Value,
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").WebWidget"))]
    pub b: Value,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FramedArgs {
    #[cfg_attr(
        feature = "ts",
        ts(type = "Record<string, import(\"@studio/graph-model\").WebWidget>")
    )]
    pub widgets: Obj,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional, type = "import(\"@studio/graph-model\").WebWidget"))]
    pub group: Option<Value>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct GridArgs {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub cols: f64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub rows: f64,
    #[serde(rename = "where")]
    pub place: String,
    #[cfg_attr(
        feature = "ts",
        ts(type = "Array<[string, import(\"@studio/graph-model\").WebWidget]>")
    )]
    pub widgets: Vec<(String, Value)>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UiArgs {
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").WebModuleUI"))]
    pub ui: Value,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FreeArgs {
    #[cfg_attr(
        feature = "ts",
        ts(type = "Record<string, import(\"@studio/graph-model\").WebWidget>")
    )]
    pub widgets: Obj,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub col: f64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub row: f64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub cols: f64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub rows: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CornerArgs {
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").WebWidget"))]
    pub w: Value,
    pub corner: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CellsArgs {
    pub a: Cell,
    pub b: Cell,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WidgetsArgs {
    #[cfg_attr(
        feature = "ts",
        ts(type = "Record<string, import(\"@studio/graph-model\").WebWidget>")
    )]
    pub widgets: Obj,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TakenArgs {
    pub taken: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PageArgs {
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").WebModuleUI"))]
    pub ui: Value,
    #[serde(default, deserialize_with = "page_ref")]
    #[cfg_attr(feature = "ts", ts(optional, type = "number | null"))]
    pub at: Option<Option<usize>>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WriteAtArgs {
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").WebModuleUI"))]
    pub ui: Value,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub at: Option<usize>,
    #[cfg_attr(
        feature = "ts",
        ts(type = "Record<string, import(\"@studio/graph-model\").WebWidget>")
    )]
    pub next: Value,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TabArgs {
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").WebModuleUI"))]
    pub ui: Value,
    pub at: usize,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub name: Option<String>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional, type = "number"))]
    pub by: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct IdArgs {
    #[cfg_attr(
        feature = "ts",
        ts(type = "Record<string, import(\"@studio/graph-model\").WebWidget>")
    )]
    pub widgets: Obj,
    pub id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PasteArgs {
    #[cfg_attr(
        feature = "ts",
        ts(type = "Record<string, import(\"@studio/graph-model\").WebWidget>")
    )]
    pub widgets: Obj,
    pub clip: Clip,
    pub at: Cell,
    #[cfg_attr(feature = "ts", ts(type = "{ cols: number, rows: number }"))]
    pub grid: GridSize,
    pub taken: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct GridSize {
    pub cols: f64,
    pub rows: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RetypeArgs {
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").WebWidget"))]
    pub w: Value,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NameArgs {
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ParamsArgs {
    pub params: Vec<AutoParam>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TextArgs {
    pub text: String,
    /// A whole `ui.k7x` rather than the `ui(…)` block of a module file.
    #[serde(default)]
    pub file: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WriteUiArgs {
    pub text: String,
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").WebModuleUI"))]
    pub ui: Value,
    #[serde(default)]
    pub file: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ContentArgs {
    pub content: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct StageArgs {
    pub path: String,
    pub io: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RowsArgs {
    pub sources: RowSources,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct LayoutArgs {
    pub text: String,
    pub committed: Option<String>,
    pub drawn: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AiPromptArgs {
    pub req: AiRequest,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AiBindArgs {
    pub params: Vec<AiParam>,
    pub module_text: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AiCheckArgs {
    pub result: AiResult,
    pub bind_names: Vec<String>,
    #[serde(default)]
    pub ctx: AiContext,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct UiTextArgs {
    pub ui_text: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AnswerArgs {
    pub text: String,
}

/// What reading a buffer's panel finds.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PanelRead {
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").WebModuleUI | null"))]
    pub ui: Option<Value>,
    pub malformed: bool,
}

/// A panel and the tab a gesture leaves selected.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Tabbed {
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").WebModuleUI"))]
    pub ui: Value,
    pub at: usize,
}

/// A paste: the widgets after it, and the block's head to select.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Pasted {
    #[cfg_attr(
        feature = "ts",
        ts(type = "Record<string, import(\"@studio/graph-model\").WebWidget>")
    )]
    pub widgets: Value,
    pub first: Option<String>,
}

/// A retype: the fields to set beside the new type, and the fields it clears.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Retyped {
    #[cfg_attr(feature = "ts", ts(type = "Partial<import(\"@studio/graph-model\").WebWidget>"))]
    pub set: Map<String, Value>,
    pub clear: Vec<String>,
}

/// The panel family's requests: `{"op": "panel.twin_issues", "args": {"ui": {…}}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum PanelRequest {
    #[serde(rename = "panel.clash")]
    Clash(TwoWidgets),
    #[serde(rename = "panel.framed_by")]
    FramedBy(FramedArgs),
    #[serde(rename = "panel.group_of")]
    GroupOf(FramedArgs),
    #[serde(rename = "panel.grid_issues")]
    GridIssues(GridArgs),
    #[serde(rename = "panel.twin_issues")]
    TwinIssues(UiArgs),
    #[serde(rename = "panel.reflow_narrow")]
    ReflowNarrow(UiArgs),
    #[serde(rename = "panel.align_tabs")]
    AlignTabs(UiArgs),
    #[serde(rename = "panel.first_free")]
    FirstFree(FreeArgs),
    #[serde(rename = "panel.corner_anchor")]
    CornerAnchor(CornerArgs),
    #[serde(rename = "panel.box_between")]
    BoxBetween(CellsArgs),
    #[serde(rename = "panel.groups_first")]
    GroupsFirst(WidgetsArgs),
    #[serde(rename = "panel.next_id")]
    NextId(TakenArgs),
    #[serde(rename = "panel.page_in")]
    PageIn(PageArgs),
    #[serde(rename = "panel.widgets_at")]
    WidgetsAt(PageArgs),
    #[serde(rename = "panel.write_widgets_at")]
    WriteWidgetsAt(WriteAtArgs),
    #[serde(rename = "panel.panel_ids")]
    PanelIds(UiArgs),
    #[serde(rename = "panel.add_tab")]
    AddTab(UiArgs),
    #[serde(rename = "panel.remove_tab")]
    RemoveTab(TabArgs),
    #[serde(rename = "panel.rename_tab")]
    RenameTab(TabArgs),
    #[serde(rename = "panel.move_tab")]
    MoveTab(TabArgs),
    #[serde(rename = "panel.clip_ids")]
    ClipIds(IdArgs),
    #[serde(rename = "panel.copy_widgets")]
    CopyWidgets(IdArgs),
    #[serde(rename = "panel.paste_widgets")]
    PasteWidgets(PasteArgs),
    #[serde(rename = "panel.retype")]
    Retype(RetypeArgs),
    #[serde(rename = "panel.family_of")]
    FamilyOf(NameArgs),
    #[serde(rename = "panel.is_plumbing")]
    IsPlumbing(NameArgs),
    #[serde(rename = "panel.is_blend")]
    IsBlend(NameArgs),
    #[serde(rename = "panel.group_params")]
    GroupParams(ParamsArgs),
    #[serde(rename = "panel.auto_organize")]
    AutoOrganize(ParamsArgs),
    #[serde(rename = "panel.read_ui")]
    ReadUi(TextArgs),
    #[serde(rename = "panel.write_ui")]
    WriteUi(WriteUiArgs),
    #[serde(rename = "panel.read_params")]
    ReadParams(ContentArgs),
    #[serde(rename = "panel.param_values")]
    ParamValues(ContentArgs),
    #[serde(rename = "panel.param_rows")]
    ParamRows(RowsArgs),
    #[serde(rename = "panel.stage_for")]
    StageFor(StageArgs),
    #[serde(rename = "panel.check_layout")]
    CheckLayout(LayoutArgs),
    #[serde(rename = "panel.ai_user_prompt")]
    AiUserPrompt(AiPromptArgs),
    #[serde(rename = "panel.ai_brief")]
    AiBrief,
    #[serde(rename = "panel.has_layout")]
    HasLayout(UiTextArgs),
    #[serde(rename = "panel.bindable_names")]
    BindableNames(AiBindArgs),
    #[serde(rename = "panel.ai_check")]
    AiCheck(AiCheckArgs),
    #[serde(rename = "panel.ai_read_answer")]
    AiReadAnswer(AnswerArgs),
}

fn pairs(list: &[(String, Value)]) -> geometry::Pairs<'_> {
    list.iter().map(|(id, w)| (id, w)).collect()
}

pub fn dispatch(request: PanelRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    use PanelRequest as R;
    match request {
        R::Clash(a) => write(out, &geometry::clash(&a.a, &a.b)),
        R::FramedBy(a) => write(out, &geometry::framed_by(&a.widgets, &a.group.unwrap_or(Value::Null))),
        R::GroupOf(a) => write(out, &geometry::group_of(&a.widgets, &a.id.unwrap_or_default())),
        R::GridIssues(a) => write(
            out,
            &geometry::grid_issues(a.cols, a.rows, &a.place, &pairs(&a.widgets)),
        ),
        R::TwinIssues(a) => write(out, &geometry::twin_issues(&a.ui)),
        R::ReflowNarrow(a) => write(out, &geometry::reflow_narrow(&a.ui)),
        R::AlignTabs(a) => write(out, &geometry::align_tabs(&a.ui)),
        R::FirstFree(a) => write(out, &geometry::first_free(&a.widgets, a.col, a.row, a.cols, a.rows)),
        R::CornerAnchor(a) => write(out, &geometry::corner_anchor(&a.w, &a.corner)),
        R::BoxBetween(a) => write(out, &geometry::box_between(a.a, a.b)),
        R::GroupsFirst(a) => write(out, &geometry::groups_first(&a.widgets)),
        R::NextId(a) => write(out, &geometry::next_id(&a.taken)),
        R::PageIn(a) => write(out, &edit::page_in(&a.ui, a.at)),
        R::WidgetsAt(a) => write(out, &edit::widgets_at(&a.ui, a.at.flatten())),
        R::WriteWidgetsAt(a) => write(out, &edit::write_widgets_at(&a.ui, a.at, a.next)),
        R::PanelIds(a) => write(out, &edit::panel_ids(&a.ui)),
        R::AddTab(a) => {
            let (ui, at) = edit::add_tab(&a.ui);
            write(out, &Tabbed { ui, at })
        }
        R::RemoveTab(a) => write(out, &edit::remove_tab(&a.ui, a.at)),
        R::RenameTab(a) => write(out, &edit::rename_tab(&a.ui, a.at, &a.name.unwrap_or_default())),
        R::MoveTab(a) => {
            let (ui, at) = edit::move_tab(&a.ui, a.at, a.by.unwrap_or(0));
            write(out, &Tabbed { ui, at })
        }
        R::ClipIds(a) => write(out, &edit::clip_ids(&a.widgets, &a.id)),
        R::CopyWidgets(a) => write(out, &edit::copy_widgets(&a.widgets, &a.id)),
        R::PasteWidgets(a) => {
            let (widgets, first) = edit::paste_widgets(
                &a.widgets,
                &a.clip,
                (a.at.col, a.at.row),
                (a.grid.cols, a.grid.rows),
                &a.taken,
            );
            write(out, &Pasted { widgets, first })
        }
        R::Retype(a) => {
            let (set, clear) = edit::retype(&a.w, &a.kind);
            let clear = clear.into_iter().map(str::to_owned).collect();
            write(out, &Retyped { set, clear })
        }
        R::FamilyOf(a) => write(out, auto::family_of(&a.name)),
        R::IsPlumbing(a) => write(out, &auto::is_plumbing(&a.name)),
        R::IsBlend(a) => write(out, &auto::is_blend(&a.name)),
        R::GroupParams(a) => write(out, &auto::group_params(&a.params)),
        R::AutoOrganize(a) => write(out, &auto::auto_organize(&a.params)),
        R::ReadUi(a) => {
            let r = write::read_ui(&a.text, a.file);
            write(
                out,
                &PanelRead {
                    ui: r.ui,
                    malformed: r.malformed,
                },
            )
        }
        R::WriteUi(a) => match write::write_ui(&a.text, &a.ui, a.file) {
            Ok(text) => write(out, &text),
            Err(e) => Err(WasmError::bad_request(e)),
        },
        R::ReadParams(a) => write(out, &write::read_params(&a.content)),
        R::ParamValues(a) => write(out, &rows::param_values(&a.content)),
        R::ParamRows(a) => write(out, &rows::param_rows(&a.sources)),
        R::StageFor(a) => write(out, &rows::stage_for(&a.path, a.io.as_deref())),
        R::CheckLayout(a) => write(out, &check::check_layout(&a.text, a.committed.as_deref(), &a.drawn)),
        R::AiUserPrompt(a) => write(out, &ai_check::ai_user_prompt(&a.req)),
        R::AiBrief => write(out, &ai_prompt::ai_design_brief()),
        R::HasLayout(a) => write(out, &ai_check::has_layout(&a.ui_text)),
        R::BindableNames(a) => write(out, &ai_check::bindable_names(&a.params, &a.module_text)),
        R::AiReadAnswer(a) => write(out, &read_ai_answer(&a.text)),
        R::AiCheck(a) => write(out, &ai_check::check_ai_result(&a.result, &a.bind_names, &a.ctx)),
    }
}
