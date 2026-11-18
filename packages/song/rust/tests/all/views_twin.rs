// views_twin.rs — writes the declarations ts-rs derives for the view types, for scripts/viewsTwinCheck.ts to hold against the hand-written ones.
//
//   K7_SONG_TWIN=<dir> cargo test --features ts --test all -- --ignored views_twin::      then      npx tsx scripts/viewsTwinCheck.ts <dir>
//
// What a reader gets wrong: the check runs from the generated type to the hand-written one (what the route gives is what the frontend
// reads), and the run-time proof of the same shapes is the gate's `compare.rs` and its `no_null` (decision 644).

use song::files::Severity;
use song::views::Diagnostic;
use song::views::catalogue::{BaseModuleRef, CatalogueModule, Panel};
use song::views::errors::{FormatTooNew, ProjectErrors};
use song::views::hints::{Hints, ParamRange, RackHint, RackSymbol};
use song::views::io_ports::{IoDir, IoPort};
use song::views::surfaces::{
    BindTarget, ProjectSurface, ProjectSurfaces, SurfaceFile, SurfaceLayout, SurfaceType, ViewBinding,
};
use song::views::web::{
    WebClip, WebGroup, WebMarker, WebMixer, WebNote, WebPlugin, WebProject, WebTimeline, WebTimelineRef, WebTrack,
};
use ts_rs::TS;

#[test]
#[ignore = "writes the declarations for scripts/viewsTwinCheck.ts: K7_SONG_TWIN"]
fn the_declarations_are_written_for_the_type_check() {
    let dir = std::path::PathBuf::from(std::env::var("K7_SONG_TWIN").expect("K7_SONG_TWIN"));
    std::fs::create_dir_all(&dir).expect("the folder");
    let decls = [
        Severity::decl(),
        Diagnostic::decl(),
        FormatTooNew::decl(),
        ProjectErrors::decl(),
        WebNote::decl(),
        WebTimelineRef::decl(),
        WebClip::decl(),
        WebMixer::decl(),
        WebPlugin::decl(),
        WebTrack::decl(),
        WebGroup::decl(),
        WebMarker::decl(),
        WebTimeline::decl(),
        WebProject::decl(),
        SurfaceType::decl(),
        SurfaceLayout::decl(),
        BindTarget::decl(),
        ViewBinding::decl(),
        SurfaceFile::decl(),
        ProjectSurface::decl(),
        ProjectSurfaces::decl(),
        Panel::decl(),
        CatalogueModule::decl(),
        BaseModuleRef::decl(),
        IoDir::decl(),
        IoPort::decl(),
        ParamRange::decl(),
        RackSymbol::decl(),
        RackHint::decl(),
        Hints::decl(),
    ];
    let text: Vec<String> = decls.iter().map(|d| format!("export {d}")).collect();
    std::fs::write(dir.join("twin.ts"), text.join("\n")).expect("the declarations");
}
