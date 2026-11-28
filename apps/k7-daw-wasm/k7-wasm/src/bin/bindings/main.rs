// bindings — writes the TypeScript type of everything that crosses the ABI into a folder, plus its index.ts, and the constants file.
// A reader would get wrong that it deletes the folder first: a type that no longer exists must not linger there.

mod consts;
mod desk_consts;

use gesture::roll_drag::GroupMove;
use gesture::ruler::RulerTick;
use gesture::tools::{FadePair, ToolAct};
use k7_wasm::WasmError;
use k7_wasm::ops::VersionAnswer;
use k7_wasm::ops::address::{AddressRequest, BenchRef};
use k7_wasm::ops::chains::{ChainsRequest, Kept};
use k7_wasm::ops::curve::{CurveRequest, OpenAnswer};
use k7_wasm::ops::desk::DeskRequest;
use k7_wasm::ops::engine::{BestAnswer, EngineRequest, WarningAnswer};
use k7_wasm::ops::eyes::{CubeAnswer, EyesRequest};
use k7_wasm::ops::fields::FieldsRequest;
use k7_wasm::ops::file::FileRequest;
use k7_wasm::ops::gesture::GestureRequest;
use k7_wasm::ops::git::GitRequest;
use k7_wasm::ops::k7x::{K7xRequest, ReadAnswer};
use k7_wasm::ops::k7x_edit::EditAnswer;
use k7_wasm::ops::media::MediaRequest;
use k7_wasm::ops::module::ModuleRequest;
use k7_wasm::ops::picture::{Aspect, PictureRequest};
use k7_wasm::ops::records::{RecordsRequest, ResolvedRecord};
use k7_wasm::ops::rules::{RulesRequest, SeqWindowDto, WindowSpan};
use k7_wasm::ops::schema::{BuiltinPluginDto, EngineModule, PlaylistDto, SchemaRequest, TaskDto, TimelineLane};
use k7_wasm::ops::stems::{StemLane, StemsRequest};
use k7_wasm::ops::text::TextRequest;
use k7_wasm::ops::timecode::TimecodeRequest;
use k7_wasm::ops::track::{TrackKind, TrackRequest};
use song::files::Validity;
use song::views::fields::FileKind;
use song_schema::engine::{Choice, Compatibility, EngineLatency};
use song_schema::{CurvePreview, CurveSpec, ExportFormat, ExportQuality, ResolvedVideo, Stem, VideoSettings};
use std::path::Path;
use std::process::ExitCode;
use std::{fs, io};
use ts_rs::TS;

/// Biome's order for two names: by letter, a capital before its lowercase where only the case differs.
fn biome_order(a: &str, b: &str) -> std::cmp::Ordering {
    for (x, y) in a.chars().zip(b.chars()) {
        if x == y {
            continue;
        }
        let (lx, ly) = (
            x.to_lowercase().next().unwrap_or(x),
            y.to_lowercase().next().unwrap_or(y),
        );
        return if lx == ly {
            x.is_lowercase().cmp(&y.is_lowercase())
        } else {
            lx.cmp(&ly)
        };
    }
    a.len().cmp(&b.len())
}

fn write_types(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if dir.exists() {
        fs::remove_dir_all(dir)?;
    }
    fs::create_dir_all(dir)?;
    // Each export walks the types its own type depends on, so these roots reach every file.
    CurveRequest::export_all_to(dir)?;
    TimecodeRequest::export_all_to(dir)?;
    AddressRequest::export_all_to(dir)?;
    BenchRef::export_all_to(dir)?;
    song_schema::address::Launch::export_all_to(dir)?;
    song_schema::address::LanSurface::export_all_to(dir)?;
    song_schema::address::LocalTarget::export_all_to(dir)?;
    EngineRequest::export_all_to(dir)?;
    SchemaRequest::export_all_to(dir)?;
    BuiltinPluginDto::export_all_to(dir)?;
    EngineModule::export_all_to(dir)?;
    PlaylistDto::export_all_to(dir)?;
    TaskDto::export_all_to(dir)?;
    TimelineLane::export_all_to(dir)?;
    K7xRequest::export_all_to(dir)?;
    ReadAnswer::export_all_to(dir)?;
    EditAnswer::export_all_to(dir)?;
    VersionAnswer::export_all_to(dir)?;
    FieldsRequest::export_all_to(dir)?;
    FileRequest::export_all_to(dir)?;
    GitRequest::export_all_to(dir)?;
    MediaRequest::export_all_to(dir)?;
    git_rules::GitState::export_all_to(dir)?;
    git_rules::ForgeAccount::export_all_to(dir)?;
    git_rules::ForgeStatus::export_all_to(dir)?;
    git_rules::ForgeProbe::export_all_to(dir)?;
    git_rules::ForgeStart::export_all_to(dir)?;
    git_rules::DeviceCode::export_all_to(dir)?;
    git_rules::PollAnswer::export_all_to(dir)?;
    git_rules::PullState::export_all_to(dir)?;
    git_rules::Entry::export_all_to(dir)?;
    git_rules::LogEntry::export_all_to(dir)?;
    git_rules::Conflict::export_all_to(dir)?;
    git_rules::BarView::export_all_to(dir)?;
    git_rules::MenuView::export_all_to(dir)?;
    git_rules::CommitView::export_all_to(dir)?;
    git_rules::SshHost::export_all_to(dir)?;
    git_rules::PortAnswer::export_all_to(dir)?;
    git_rules::Arranged::export_all_to(dir)?;
    git_rules::Ago::export_all_to(dir)?;
    git_rules::RoamAnswer::export_all_to(dir)?;
    git_rules::RoamState::export_all_to(dir)?;
    TextRequest::export_all_to(dir)?;
    k7_wasm::ops::text::TextAnswer::export_all_to(dir)?;
    k7_wasm::ops::text_lane::VideoPlacement::export_all_to(dir)?;
    k7_wasm::ops::text_desk::MixCues::export_all_to(dir)?;
    k7_wasm::ops::text_desk::MixViewRead::export_all_to(dir)?;
    song::views::surfaces::ViewBinding::export_all_to(dir)?;
    song::views::surfaces::ProjectSurfaces::export_all_to(dir)?;
    song::wire::ProjectShape::export_all_to(dir)?;
    song::wire::NodePresets::export_all_to(dir)?;
    song::wire::ReferenceTrack::export_all_to(dir)?;
    k7_wasm::ops::text_desk::AssemblyRead::export_all_to(dir)?;
    k7_wasm::ops::text_desk::ChainRead::export_all_to(dir)?;
    k7_wasm::ops::text_desk::LibraryPins::export_all_to(dir)?;
    ModuleRequest::export_all_to(dir)?;
    k7_wasm::ops::library::LibraryRequest::export_all_to(dir)?;
    k7_wasm::ops::nodes::NodesRequest::export_all_to(dir)?;
    k7_wasm::ops::surfaces::SurfacesRequest::export_all_to(dir)?;
    chains::surface_rows::RowGroup::export_all_to(dir)?;
    chains::pane_in_use::InUse::export_all_to(dir)?;
    chains::history_marks::HistoryMark::export_all_to(dir)?;
    chains::master_controls::MasterControl::export_all_to(dir)?;
    chains::anchor_right::SplitMixer::export_all_to(dir)?;
    k7_wasm::ops::panel::PanelRequest::export_all_to(dir)?;
    k7_wasm::ops::panel::Tabbed::export_all_to(dir)?;
    k7_wasm::ops::panel::Pasted::export_all_to(dir)?;
    k7_wasm::ops::panel::Retyped::export_all_to(dir)?;
    k7_wasm::ops::panel::PanelRead::export_all_to(dir)?;
    song::modules::ui::rows::ParamRowOut::export_all_to(dir)?;
    song::modules::ui::ai_check::Checked::export_all_to(dir)?;
    song::modules::ui::ai_check::AiRequest::export_all_to(dir)?;
    song::modules::ui::ai_check::AiResult::export_all_to(dir)?;
    song::modules::ui::ai_check::AiContext::export_all_to(dir)?;
    song::modules::ui::ai_answer::AiEvent::export_all_to(dir)?;
    song::modules::ui::auto::Page::export_all_to(dir)?;
    song::modules::ui::edit::Clip::export_all_to(dir)?;
    song::modules::ui::geometry::Cell::export_all_to(dir)?;
    PictureRequest::export_all_to(dir)?;
    Aspect::export_all_to(dir)?;
    RecordsRequest::export_all_to(dir)?;
    ResolvedRecord::export_all_to(dir)?;
    RulesRequest::export_all_to(dir)?;
    SeqWindowDto::export_all_to(dir)?;
    WindowSpan::export_all_to(dir)?;
    StemsRequest::export_all_to(dir)?;
    StemLane::export_all_to(dir)?;
    TrackRequest::export_all_to(dir)?;
    TrackKind::export_all_to(dir)?;
    VideoSettings::export_all_to(dir)?;
    ResolvedVideo::export_all_to(dir)?;
    ExportFormat::export_all_to(dir)?;
    ExportQuality::export_all_to(dir)?;
    Stem::export_all_to(dir)?;
    GestureRequest::export_all_to(dir)?;
    FileKind::export_all_to(dir)?;
    Validity::export_all_to(dir)?;
    WasmError::export_all_to(dir)?;
    OpenAnswer::export_all_to(dir)?;
    CurvePreview::export_all_to(dir)?;
    CurveSpec::export_all_to(dir)?;
    BestAnswer::export_all_to(dir)?;
    Choice::export_all_to(dir)?;
    Compatibility::export_all_to(dir)?;
    EngineLatency::export_all_to(dir)?;
    WarningAnswer::export_all_to(dir)?;
    ChainsRequest::export_all_to(dir)?;
    Kept::export_all_to(dir)?;
    k7_wasm::ops::chains::Built::export_all_to(dir)?;
    k7_wasm::ops::chains::Size::export_all_to(dir)?;
    k7_wasm::ops::chains::Placement::export_all_to(dir)?;
    chains::row_seams::SeamsAnswer::export_all_to(dir)?;
    chains::stack_layout::StackAnswer::export_all_to(dir)?;
    chains::module_row::Spaced::export_all_to(dir)?;
    chains::node::EdgeInsert::export_all_to(dir)?;
    chains::bindings::ResolvedMapping::export_all_to(dir)?;
    chains::subgraph::KeptChain::export_all_to(dir)?;
    chains::curve_placement::Moved::export_all_to(dir)?;
    chains::flow::NodeFlow::export_all_to(dir)?;
    #[cfg(feature = "verbs")]
    export_ui(dir)?;
    #[cfg(feature = "verbs")]
    verbs::picture::Doc::export_all_to(dir)?;
    song::doc::ProjectDoc::export_all_to(dir)?;
    song::views::web::WebProject::export_all_to(dir)?;
    song::reads::Blanks::export_all_to(dir)?;
    song::reads::ShellView::export_all_to(dir)?;
    song::reads::LaneMix::export_all_to(dir)?;
    song::reads::ClipLoop::export_all_to(dir)?;
    song::reads::Instances::export_all_to(dir)?;
    song::reads::TakesEvent::export_all_to(dir)?;
    song::reads::LayoutRow::export_all_to(dir)?;
    song::reads::LaneNotes::export_all_to(dir)?;
    song::reads::LaneSpan::export_all_to(dir)?;
    song::reads::Hit::export_all_to(dir)?;
    song::reads::LaneInput::export_all_to(dir)?;
    song::reads::PlayedTarget::export_all_to(dir)?;
    song::reads::ClipNameParts::export_all_to(dir)?;
    song::reads::TapRun::export_all_to(dir)?;
    song::reads::FolderName::export_all_to(dir)?;
    DeskRequest::export_all_to(dir)?;
    turntables::beats::BeatGrid::export_all_to(dir)?;
    turntables::loops::LoopSpan::export_all_to(dir)?;
    turntables::deck::LevelDb::export_all_to(dir)?;
    turntables::bindings::DeskBindings::export_all_to(dir)?;
    turntables::bindings::Published::export_all_to(dir)?;
    turntables::set_list::SetList::export_all_to(dir)?;
    EyesRequest::export_all_to(dir)?;
    export_buffer(dir)?;
    CubeAnswer::export_all_to(dir)?;
    eyes::scene::VideoScene::export_all_to(dir)?;
    eyes::scene::Transition::export_all_to(dir)?;
    eyes::cameras::Match::export_all_to(dir)?;
    GroupMove::export_all_to(dir)?;
    ToolAct::export_all_to(dir)?;
    FadePair::export_all_to(dir)?;
    RulerTick::export_all_to(dir)?;
    let mut names = fs::read_dir(dir)?
        .map(|e| e.map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect::<Result<Vec<_>, io::Error>>()?;
    // Biome's export order: letters compare ignoring case, but the first place two names differ only by case puts the capital first.
    names.sort_by(|a, b| biome_order(a, b));
    let index: String = names
        .iter()
        .filter_map(|n| n.strip_suffix(".ts"))
        .filter(|n| *n != "index")
        .map(|n| format!("export type * from \"./{n}\";\n"))
        .collect();
    fs::write(
        dir.join("index.ts"),
        format!("// index.ts — GENERATED by bindings/main.rs. Do not edit.\n\n{index}"),
    )?;
    Ok(())
}

/// The editor's buffer, paths, intake, kit, clipboard and curve-draft ops, and the types their answers carry.
fn export_buffer(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use k7_wasm::ops::buffer::{
        BufferRequest, ClipWindow, K7xCursor, KeyToken, PreviewReq, PreviewTrigger, SliceSym, ValueToken,
    };
    use k7_wasm::ops::clipboard::ClipboardRequest;
    use k7_wasm::ops::draft::DraftRequest;
    use k7_wasm::ops::intake::{IntakeRequest, KitGrid};
    use k7_wasm::ops::paths::{PathsRequest, TreeEntry};
    use k7_wasm::ops::text_buffer::SequenceRead;
    use song_edit::buffer_text::{BufferReads, WindowBand};
    use song_edit::clipboard::{Clipboard, PastedEvent, PastedNote};
    use song_edit::curve_draft::{CurveDraft, DraftShape};
    use song_edit::intake::DropPlan;
    use song_edit::kit::RackIssue;
    BufferRequest::export_all_to(dir)?;
    ClipboardRequest::export_all_to(dir)?;
    DraftRequest::export_all_to(dir)?;
    IntakeRequest::export_all_to(dir)?;
    PathsRequest::export_all_to(dir)?;
    for r in [
        ClipWindow::export_all_to(dir),
        K7xCursor::export_all_to(dir),
        KeyToken::export_all_to(dir),
        PreviewReq::export_all_to(dir),
        PreviewTrigger::export_all_to(dir),
        SliceSym::export_all_to(dir),
        ValueToken::export_all_to(dir),
        KitGrid::export_all_to(dir),
        TreeEntry::export_all_to(dir),
        SequenceRead::export_all_to(dir),
        BufferReads::export_all_to(dir),
        WindowBand::export_all_to(dir),
        Clipboard::export_all_to(dir),
        PastedEvent::export_all_to(dir),
        PastedNote::export_all_to(dir),
        CurveDraft::export_all_to(dir),
        DraftShape::export_all_to(dir),
        DropPlan::export_all_to(dir),
        RackIssue::export_all_to(dir),
        k7_wasm::ops::text_buffer::GridNote::export_all_to(dir),
    ] {
        r?;
    }
    Ok(())
}

/// The `ui.*` requests and answers, and the verbs types their answers carry.
#[cfg(feature = "verbs")]
fn export_ui(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use k7_wasm::ops::verb_ui::{Indices, MenuChoiceAnswer, PrefsAnswer, Scores, UiRequest};
    use verbs::ui::fill::{EnterAction, Offered};
    use verbs::ui::prefs::PrefsView;
    use verbs::ui::rank::{ArgOption, EmptyRow};
    use verbs::ui::step::Step;
    UiRequest::export_all_to(dir)?;
    Scores::export_all_to(dir)?;
    Indices::export_all_to(dir)?;
    MenuChoiceAnswer::export_all_to(dir)?;
    PrefsAnswer::export_all_to(dir)?;
    Offered::export_all_to(dir)?;
    EnterAction::export_all_to(dir)?;
    ArgOption::export_all_to(dir)?;
    EmptyRow::export_all_to(dir)?;
    PrefsView::export_all_to(dir)?;
    Step::export_all_to(dir)?;
    export_edit(dir)?;
    export_menu(dir)
}

/// The `menu.*` requests and answers, and the catalogue's own types.
#[cfg(feature = "verbs")]
fn export_menu(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use k7_wasm::ops::verb_menu::{Danger, Focus, MenuRequest, Named, Position};
    use verbs::palette::CommandInfo;
    use verbs::ui::home::{HomeRow, Parsed, Resolved};
    use verbs::ui::menu::{Item, MenuTarget, TabGroup};
    use verbs::ui::palette::{ArgSource, PaletteRow};
    use verbs::ui::wire::{ArgClass, ArgProvider, MenuEntity};
    MenuRequest::export_all_to(dir)?;
    for r in [
        Danger::export_all_to(dir),
        Focus::export_all_to(dir),
        Named::export_all_to(dir),
        Position::export_all_to(dir),
        CommandInfo::export_all_to(dir),
        HomeRow::export_all_to(dir),
        Parsed::export_all_to(dir),
        Resolved::export_all_to(dir),
        Item::export_all_to(dir),
        MenuTarget::export_all_to(dir),
        TabGroup::export_all_to(dir),
        ArgSource::export_all_to(dir),
        PaletteRow::export_all_to(dir),
        ArgClass::export_all_to(dir),
        ArgProvider::export_all_to(dir),
        MenuEntity::export_all_to(dir),
    ] {
        r?;
    }
    Ok(())
}

/// The `edit.*` requests and answers, and the verbs types their answers carry.
#[cfg(feature = "verbs")]
fn export_edit(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use k7_wasm::ops::verb_edit::{ClipPatches, EditRequest, MaybeLine};
    use verbs::ui::export::ExportAsk;
    use verbs::ui::pending::PendingEdit;
    use verbs::ui::writes::VerbClass;
    EditRequest::export_all_to(dir)?;
    ClipPatches::export_all_to(dir)?;
    MaybeLine::export_all_to(dir)?;
    ExportAsk::export_all_to(dir)?;
    PendingEdit::export_all_to(dir)?;
    VerbClass::export_all_to(dir)?;
    Ok(())
}

fn main() -> ExitCode {
    let (Some(dir), Some(consts)) = (std::env::args().nth(1), std::env::args().nth(2)) else {
        eprintln!("usage: bindings <types dir> <consts file>");
        return ExitCode::from(2);
    };
    let done = write_types(Path::new(&dir)).and_then(|()| Ok(fs::write(&consts, consts::text())?));
    match done {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("bindings: {e}");
            ExitCode::FAILURE
        }
    }
}
