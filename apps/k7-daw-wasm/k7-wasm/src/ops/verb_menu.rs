// ops/verb_menu.rs — the `menu.*` operations: a right-click's rows, tabs and title, the palette's catalogue and vocabularies, and the home page's commands.
// A reader would get wrong who writes the words: a row carries a message key where it is named by hand, and the page resolves it in its language.

use crate::error::WasmError;
use crate::ops::write;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use verbs::palette::CommandInfo;
use verbs::ui::home::{self, HomeContext, HomeRow, Parsed, Resolved, WorkspaceRef};
use verbs::ui::menu::{self, Item, MenuCtx, MenuTarget, NodeMenuTarget, TabGroup, TabRow, Text};
use verbs::ui::palette::{self, ArgSource, LocalCommand, PaletteRow, ShellLists};

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ContextMenuArgs {
    pub ctx: MenuCtx,
    pub commands: Vec<CommandInfo>,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
    pub prefs: Map<String, Value>,
    pub locale: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CtxArgs {
    pub ctx: MenuCtx,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RowsArgs {
    pub rows: Vec<TabRow>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FocusArgs {
    pub key: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub at: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub n: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NodeMenuArgs {
    pub target: NodeMenuTarget,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BuildArgs {
    pub backend: Vec<CommandInfo>,
    pub locals: Vec<LocalCommand>,
    pub locale: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NameOfArgs {
    pub line: String,
    pub commands: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ArgSourceArgs {
    pub canonical: String,
    pub prior: Vec<String>,
    pub lists: ShellLists,
    pub entry_arg_is_file: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LocaleArgs {
    pub locale: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LineArgs {
    pub line: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct HomeCtxArgs {
    pub ctx: HomeContext,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct HomeValuesArgs {
    pub canonical: String,
    pub arg_index: usize,
    pub ctx: HomeContext,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FindArgs {
    pub workspace: WorkspaceRef,
    pub name: String,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Danger {
    pub safe: Vec<usize>,
    pub danger: Vec<usize>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Position {
    pub index: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Focus {
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub at: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Named {
    pub canonical: Option<String>,
}

/// The `menu.*` requests.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum MenuRequest {
    #[serde(rename = "menu.context_menu")]
    ContextMenu(ContextMenuArgs),
    #[serde(rename = "menu.with_ambient")]
    WithAmbient(CtxArgs),
    #[serde(rename = "menu.target")]
    Target(CtxArgs),
    #[serde(rename = "menu.tabs")]
    Tabs(RowsArgs),
    #[serde(rename = "menu.danger_split")]
    DangerSplit(RowsArgs),
    #[serde(rename = "menu.move_focus")]
    MoveFocus(FocusArgs),
    #[serde(rename = "menu.node_menu")]
    NodeMenu(NodeMenuArgs),
    #[serde(rename = "menu.build_commands")]
    BuildCommands(BuildArgs),
    #[serde(rename = "menu.command_name_of")]
    CommandNameOf(NameOfArgs),
    #[serde(rename = "menu.arg_source")]
    ArgSource(ArgSourceArgs),
    #[serde(rename = "menu.home_catalogue")]
    HomeCatalogue(LocaleArgs),
    #[serde(rename = "menu.home_parse")]
    HomeParse(LineArgs),
    #[serde(rename = "menu.home_resolve")]
    HomeResolve(HomeCtxArgs),
    #[serde(rename = "menu.home_find_project")]
    HomeFindProject(FindArgs),
    #[serde(rename = "menu.home_gripe")]
    HomeGripe(FindArgs),
    #[serde(rename = "menu.home_arg_values")]
    HomeArgValues(HomeValuesArgs),
}

pub fn dispatch(request: MenuRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    match request {
        MenuRequest::ContextMenu(a) => {
            let items: Vec<Item> = menu::context_menu(&a.ctx, &a.commands, &a.prefs, &a.locale);
            write(out, &items)
        }
        MenuRequest::WithAmbient(a) => write(out, &menu::with_ambient(a.ctx)),
        MenuRequest::Target(a) => {
            let t: Option<MenuTarget> = menu::menu_target(&a.ctx);
            write(out, &t)
        }
        MenuRequest::Tabs(a) => {
            let tabs: Vec<TabGroup> = menu::menu_tabs(&a.rows);
            write(out, &tabs)
        }
        MenuRequest::DangerSplit(a) => {
            let (safe, danger) = menu::danger_split(&a.rows);
            write(out, &Danger { safe, danger })
        }
        MenuRequest::MoveFocus(a) => write(
            out,
            &Focus {
                at: menu::move_focus(&a.key, a.at, a.n),
            },
        ),
        MenuRequest::NodeMenu(a) => {
            let items: Vec<Item> = menu::node_menu(&a.target);
            write(out, &items)
        }
        MenuRequest::BuildCommands(a) => {
            let rows: Vec<PaletteRow> = palette::build_commands(&a.backend, &a.locals, &a.locale);
            write(out, &rows)
        }
        MenuRequest::CommandNameOf(a) => write(
            out,
            &Named {
                canonical: palette::command_name_of(&a.line, &a.commands),
            },
        ),
        MenuRequest::ArgSource(a) => {
            let s: ArgSource = palette::arg_source(&a.canonical, &a.prior, &a.lists, a.entry_arg_is_file);
            write(out, &s)
        }
        MenuRequest::HomeCatalogue(a) => {
            let rows: Vec<HomeRow> = home::catalogue(&a.locale);
            write(out, &rows)
        }
        MenuRequest::HomeParse(a) => {
            let p: Parsed = home::parse_line(&a.line);
            write(out, &p)
        }
        MenuRequest::HomeResolve(a) => {
            let r: Resolved = home::resolve_workspace(&a.ctx);
            write(out, &r)
        }
        MenuRequest::HomeFindProject(a) => write(
            out,
            &Position {
                index: home::find_project(&a.workspace, &a.name),
            },
        ),
        MenuRequest::HomeGripe(a) => {
            let t: Text = home::project_gripe(&a.workspace, &a.name);
            write(out, &t)
        }
        MenuRequest::HomeArgValues(a) => write(out, &home::arg_values(&a.canonical, a.arg_index, &a.ctx)),
    }
}
