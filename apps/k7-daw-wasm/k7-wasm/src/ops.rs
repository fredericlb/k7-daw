// ops.rs — the request enum and the dispatch: JSON bytes in, JSON bytes out, no FFI and no global state.
// A reader would get wrong that an answer is the operation's value itself: only an error has a wrapper, and the status says which.

use crate::curve_ops::{
    CloseArgs, CurveTable, FormatArgs, OpenArgs, ParseArgs, PreviewArgs, curve_preview, timecode_format, timecode_parse,
};
use crate::error::WasmError;
use serde::{Deserialize, Serialize};

/// The status of a call that answered its value.
pub const OK: u32 = 0;
/// The status of a call that answered a `WasmError`.
pub const ERROR: u32 = 1;

/// One operation and its arguments: `{"op": "curve.preview", "args": {…}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum Request {
    #[serde(rename = "curve.preview")]
    CurvePreview(PreviewArgs),
    #[serde(rename = "curve.open")]
    CurveOpen(OpenArgs),
    #[serde(rename = "curve.close")]
    CurveClose(CloseArgs),
    #[serde(rename = "timecode.parse")]
    TimecodeParse(ParseArgs),
    #[serde(rename = "timecode.format")]
    TimecodeFormat(FormatArgs),
    #[cfg(feature = "song")]
    #[serde(rename = "song.open")]
    SongOpen(crate::song_ops::OpenArgs),
    #[cfg(feature = "song")]
    #[serde(rename = "song.update")]
    SongUpdate(crate::song_ops::UpdateArgs),
    #[cfg(feature = "song")]
    #[serde(rename = "song.compile")]
    SongCompile(crate::song_ops::CompileArgs),
    #[cfg(feature = "song")]
    #[serde(rename = "song.close")]
    SongClose(crate::song_ops::CloseArgs),
    #[cfg(feature = "song")]
    #[serde(rename = "edit.call")]
    EditCall(crate::edit_ops::CallArgs),
    #[cfg(feature = "song")]
    #[serde(rename = "edit.batch")]
    EditBatch(crate::edit_ops::BatchArgs),
    #[cfg(feature = "verbs")]
    #[serde(rename = "verb.run")]
    VerbRun(verbs::VerbRequest),
    #[cfg(feature = "verbs")]
    #[serde(rename = "verb.complete")]
    VerbComplete(crate::verb_ops::CompleteArgs),
    #[cfg(feature = "verbs")]
    #[serde(rename = "verb.catalogue")]
    VerbCatalogue,
    #[serde(rename = "gesture.plan_group_move")]
    GesturePlanGroupMove(crate::gesture_ops::PlanArgs),
    #[serde(rename = "gesture.warp_line")]
    GestureWarpLine(crate::gesture_ops::WarpArgs),
    #[serde(rename = "gesture.tool_line")]
    GestureToolLine(crate::gesture_ops::ToolArgs),
    #[serde(rename = "gesture.picture_tool_line")]
    GesturePictureToolLine(crate::gesture_ops::PictureToolArgs),
    #[serde(rename = "gesture.crossfade_pair")]
    GestureCrossfadePair(crate::gesture_ops::CrossfadeArgs),
    #[serde(rename = "gesture.ruler_ticks")]
    GestureRulerTicks(gesture::ruler::TickSpec),
    #[serde(rename = "text.hash")]
    TextHash(crate::engine_ops::HashArgs),
    #[serde(rename = "engine.compat")]
    EngineCompat(crate::engine_ops::CompatArgs),
    #[serde(rename = "engine.latency")]
    EngineLatency(crate::engine_ops::LatencyArgs),
    #[serde(rename = "engine.warning")]
    EngineWarning(crate::engine_ops::WarningArgs),
    #[serde(rename = "engine.choose")]
    EngineChoose(crate::engine_ops::ChooseArgs),
    #[serde(rename = "engine.best")]
    EngineBest(crate::engine_ops::ChooseArgs),
    /// The hash of the Rust sources this module was built from (decision 756).
    #[serde(rename = "version")]
    Version,
}

/// What `version` answers.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct VersionAnswer {
    pub sources: String,
}

/// The sources hash `buildWasm.mjs` sets in the build's environment, or `dev` for a build made by hand.
const SOURCES: &str = match option_env!("K7_WASM_SOURCES") {
    Some(s) => s,
    None => "dev",
};

/// What outlives a call: the table of open curves.
#[derive(Default)]
pub struct Session {
    pub curves: CurveTable,
    #[cfg(feature = "song")]
    pub songs: crate::song_ops::SongTable,
}

fn write<T: Serialize + ?Sized>(out: &mut Vec<u8>, value: &T) -> Result<(), WasmError> {
    serde_json::to_writer(out, value).map_err(|e| WasmError::bad_request(e.to_string()))
}

impl Session {
    pub fn new() -> Session {
        Session::default()
    }

    /// Runs one request, leaving the answer's JSON in `out`.
    pub fn call(&mut self, input: &[u8], out: &mut Vec<u8>) -> Result<(), WasmError> {
        out.clear();
        let request: Request = serde_json::from_slice(input).map_err(|e| WasmError::bad_request(e.to_string()))?;
        match request {
            Request::CurvePreview(a) => write(out, &curve_preview(a)?),
            Request::CurveOpen(a) => write(out, &self.curves.open(a)?),
            Request::CurveClose(a) => write(out, &self.curves.close(a)?),
            Request::TimecodeParse(a) => write(out, &timecode_parse(a)?),
            Request::TimecodeFormat(a) => write(out, &timecode_format(a)?),
            #[cfg(feature = "song")]
            Request::SongOpen(a) => write(out, &self.songs.open(a)?),
            #[cfg(feature = "song")]
            Request::SongUpdate(a) => write(out, &self.songs.update(a)?),
            #[cfg(feature = "song")]
            Request::SongCompile(a) => self.songs.compile(a, out),
            #[cfg(feature = "song")]
            Request::SongClose(a) => write(out, &self.songs.close(a)?),
            #[cfg(feature = "song")]
            Request::EditCall(a) => write(out, &crate::edit_ops::call(a)?),
            #[cfg(feature = "song")]
            Request::EditBatch(a) => write(out, &crate::edit_ops::batch(a)?),
            #[cfg(feature = "verbs")]
            Request::VerbRun(a) => write(out, &crate::verb_ops::run(a)?),
            #[cfg(feature = "verbs")]
            Request::VerbComplete(a) => write(out, &crate::verb_ops::complete(a)?),
            #[cfg(feature = "verbs")]
            Request::VerbCatalogue => write(out, verbs::palette::catalogue()),
            Request::GesturePlanGroupMove(a) => write(out, &crate::gesture_ops::plan_move(a)),
            Request::GestureWarpLine(a) => write(out, &crate::gesture_ops::warp(a)),
            Request::GestureToolLine(a) => write(out, &crate::gesture_ops::clip_tool(a)),
            Request::GesturePictureToolLine(a) => write(out, &crate::gesture_ops::shot_tool(a)),
            Request::GestureCrossfadePair(a) => write(out, &crate::gesture_ops::crossfade(a)),
            Request::GestureRulerTicks(a) => write(out, &crate::gesture_ops::ticks(a)),
            Request::TextHash(a) => write(out, &crate::engine_ops::text_hash(a)),
            Request::EngineCompat(a) => write(out, &crate::engine_ops::compat(a)),
            Request::EngineLatency(a) => write(out, &crate::engine_ops::latency(a)),
            Request::EngineWarning(a) => write(out, &crate::engine_ops::warning(a)),
            Request::EngineChoose(a) => write(out, &crate::engine_ops::choose(a)),
            Request::EngineBest(a) => write(out, &crate::engine_ops::best(a)),
            Request::Version => write(
                out,
                &VersionAnswer {
                    sources: SOURCES.to_owned(),
                },
            ),
        }
    }

    /// Runs one request and answers its status: the value's JSON in `out` for OK, a `WasmError`'s for ERROR.
    pub fn handle(&mut self, input: &[u8], out: &mut Vec<u8>) -> u32 {
        match self.call(input, out) {
            Ok(()) => OK,
            Err(e) => {
                out.clear();
                // A WasmError is two plain fields: serializing it cannot fail, and an empty answer is the fallback.
                let _ = serde_json::to_writer(&mut *out, &e);
                ERROR
            }
        }
    }
}
