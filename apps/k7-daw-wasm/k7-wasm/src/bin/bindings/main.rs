// bindings — writes the TypeScript type of everything that crosses the ABI into a folder, plus its index.ts, and the constants file.
// A reader would get wrong that it deletes the folder first: a type that no longer exists must not linger there.

mod consts;

use gesture::roll_drag::GroupMove;
use gesture::ruler::RulerTick;
use gesture::tools::{FadePair, ToolAct};
use k7_wasm::WasmError;
use k7_wasm::ops::VersionAnswer;
use k7_wasm::ops::curve::{CurveRequest, OpenAnswer};
use k7_wasm::ops::engine::{BestAnswer, EngineRequest, WarningAnswer};
use k7_wasm::ops::eyes::{CubeAnswer, EyesRequest};
use k7_wasm::ops::fields::FieldsRequest;
use k7_wasm::ops::file::FileRequest;
use k7_wasm::ops::gesture::GestureRequest;
use k7_wasm::ops::k7x::{K7xRequest, ReadAnswer};
use k7_wasm::ops::k7x_edit::EditAnswer;
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

fn write_types(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if dir.exists() {
        fs::remove_dir_all(dir)?;
    }
    fs::create_dir_all(dir)?;
    // Each export walks the types its own type depends on, so these roots reach every file.
    CurveRequest::export_all_to(dir)?;
    TimecodeRequest::export_all_to(dir)?;
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
    TextRequest::export_all_to(dir)?;
    k7_wasm::ops::text::TextAnswer::export_all_to(dir)?;
    k7_wasm::ops::text_lane::VideoPlacement::export_all_to(dir)?;
    k7_wasm::ops::text_desk::MixCues::export_all_to(dir)?;
    k7_wasm::ops::text_desk::MixViewRead::export_all_to(dir)?;
    song::views::surfaces::ViewBinding::export_all_to(dir)?;
    k7_wasm::ops::text_desk::AssemblyRead::export_all_to(dir)?;
    k7_wasm::ops::text_desk::ChainRead::export_all_to(dir)?;
    k7_wasm::ops::text_desk::LibraryPins::export_all_to(dir)?;
    ModuleRequest::export_all_to(dir)?;
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
    EyesRequest::export_all_to(dir)?;
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
    // Biome's import order ignores case, so the index is sorted the same way.
    names.sort_by_key(|n| (n.to_lowercase(), n.clone()));
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
