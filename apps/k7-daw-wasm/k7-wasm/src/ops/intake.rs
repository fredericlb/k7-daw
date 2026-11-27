// ops/intake.rs — the intake.* and kit.* operations: the names bytes get when they enter a project, and the rack builder's rules.
// A reader would get wrong that an accent is folded by the page: `folded` is the name after `normalize("NFD")` with its combining marks taken off.

use crate::error::{ErrorCode, WasmError};
use crate::ops::write;
use serde::Deserialize;
use song_edit::intake::{
    DropLane, candidate_ids, clip_decl, declared_source, dropped_clip, dropped_kind, ext_for_mime, file_stamp, place_line, plan_drop,
    take_id,
};
use song_edit::kit::{
    KIT_GRID, KitDestination, RackPad, build_kit_node, cells_in_note_order, default_symbol, kit_sample_ref, kit_upload_dir, pad_note,
    rack_issues, rack_module_ref,
};

/// A dropped file's name beside the same name with its accents folded.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DroppedName {
    pub name: String,
    pub folded: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct IntakeNameArgs {
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct IntakePlanArgs {
    pub names: Vec<DroppedName>,
    pub lanes: Vec<DropLane>,
    pub at: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct IntakeUploadArgs {
    pub upload: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct IntakeTextArgs {
    pub text: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct IntakeCandidatesArgs {
    pub id: String,
    pub n: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct IntakeMimeArgs {
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub mime: Option<String>,
}

/// The wall clock a stamp is made of: the month counts from 0, as `Date` does.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct IntakeStampArgs {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct IntakeTakeIdArgs {
    pub lane: String,
    pub stamp: String,
}

/// A number JSON can carry, or the word for the three it cannot: NaN and the two infinities.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(untagged)]
pub enum JsNumber {
    Value(f64),
    #[cfg_attr(feature = "ts", ts(type = "\"NaN\" | \"Infinity\" | \"-Infinity\""))]
    Word(String),
}

impl JsNumber {
    fn get(&self) -> f64 {
        match self {
            JsNumber::Value(n) => *n,
            JsNumber::Word(w) => match w.as_str() {
                "Infinity" => f64::INFINITY,
                "-Infinity" => f64::NEG_INFINITY,
                _ => f64::NAN,
            },
        }
    }
}

/// A take's place and length in seconds.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct IntakePlaceArgs {
    pub lane: String,
    pub id: String,
    pub at_sec: JsNumber,
    pub length_sec: JsNumber,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct KitFileNameArgs {
    pub file_name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct KitBaseModulesArgs {
    pub base_modules: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct KitUploadDirArgs {
    pub dest: KitDestination,
    pub kit: String,
    pub project: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct KitSampleRefArgs {
    pub dest: KitDestination,
    pub kit: String,
    pub file: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct KitIssuesArgs {
    pub name: String,
    pub pads: Vec<RackPad>,
    pub taken: Vec<String>,
    pub has_rack: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct KitNodeArgs {
    pub name: String,
    pub pads: Vec<RackPad>,
    pub dest: KitDestination,
    pub rack_ref: String,
}

/// A cell of the kit's grid, with the note its pad fires.
#[derive(Clone, Debug, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct KitCell {
    pub row: usize,
    pub col: usize,
    pub note: f64,
}

/// The grid's side and its cells in NOTE order.
#[derive(Clone, Debug, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct KitGrid {
    pub size: usize,
    pub cells: Vec<KitCell>,
}

/// The intake family's requests: `{"op": "intake.plan", "args": {…}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum IntakeRequest {
    #[serde(rename = "intake.kind")]
    Kind(IntakeNameArgs),
    #[serde(rename = "intake.clip")]
    Clip(DroppedName),
    #[serde(rename = "intake.plan")]
    Plan(IntakePlanArgs),
    #[serde(rename = "intake.decl")]
    Decl(IntakeUploadArgs),
    #[serde(rename = "intake.declared_source")]
    DeclaredSource(IntakeTextArgs),
    #[serde(rename = "intake.candidates")]
    Candidates(IntakeCandidatesArgs),
    #[serde(rename = "intake.ext_for_mime")]
    ExtForMime(IntakeMimeArgs),
    #[serde(rename = "intake.stamp")]
    Stamp(IntakeStampArgs),
    #[serde(rename = "intake.take_id")]
    TakeId(IntakeTakeIdArgs),
    #[serde(rename = "intake.place_line")]
    PlaceLine(IntakePlaceArgs),
    #[serde(rename = "kit.grid")]
    Grid {},
    #[serde(rename = "kit.default_symbol")]
    DefaultSymbol(KitFileNameArgs),
    #[serde(rename = "kit.module_ref")]
    ModuleRef(KitBaseModulesArgs),
    #[serde(rename = "kit.upload_dir")]
    UploadDir(KitUploadDirArgs),
    #[serde(rename = "kit.sample_ref")]
    SampleRef(KitSampleRefArgs),
    #[serde(rename = "kit.issues")]
    Issues(KitIssuesArgs),
    #[serde(rename = "kit.node")]
    Node(KitNodeArgs),
}

pub fn dispatch(request: IntakeRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    use IntakeRequest as R;
    match request {
        R::Kind(a) => write(out, &dropped_kind(&a.name)),
        R::Clip(a) => write(out, &dropped_clip(&a.name, &a.folded)),
        R::Plan(a) => {
            let names: Vec<(String, String)> = a.names.into_iter().map(|n| (n.name, n.folded)).collect();
            write(out, &plan_drop(&names, &a.lanes, &a.at))
        }
        R::Decl(a) => write(out, &clip_decl(&a.upload).map_err(|e| WasmError::new(ErrorCode::BadK7x, e.to_string()))?),
        R::DeclaredSource(a) => write(out, &declared_source(&a.text)),
        R::Candidates(a) => write(out, &candidate_ids(&a.id, a.n)),
        R::ExtForMime(a) => write(out, ext_for_mime(a.mime.as_deref())),
        R::Stamp(a) => write(out, &file_stamp(a.year, a.month, a.day, a.hour, a.minute, a.second)),
        R::TakeId(a) => write(out, &take_id(&a.lane, &a.stamp)),
        R::PlaceLine(a) => write(
            out,
            &place_line(&a.lane, &a.id, a.at_sec.get(), a.length_sec.get()),
        ),
        R::Grid {} => write(
            out,
            &KitGrid {
                size: KIT_GRID,
                cells: cells_in_note_order()
                    .into_iter()
                    .map(|(row, col)| KitCell {
                        row,
                        col,
                        note: pad_note(row, col),
                    })
                    .collect(),
            },
        ),
        R::DefaultSymbol(a) => write(out, &default_symbol(&a.file_name)),
        R::ModuleRef(a) => write(out, &rack_module_ref(&a.base_modules)),
        R::UploadDir(a) => write(out, &kit_upload_dir(&a.dest, &a.kit, &a.project)),
        R::SampleRef(a) => write(out, &kit_sample_ref(&a.dest, &a.kit, &a.file)),
        R::Issues(a) => write(out, &rack_issues(&a.name, &a.pads, &a.taken, a.has_rack)),
        R::Node(a) => write(
            out,
            &build_kit_node(&a.name, &a.pads, &a.dest, &a.rack_ref).map_err(|e| WasmError::new(ErrorCode::BadK7x, e.to_string()))?,
        ),
    }
}
