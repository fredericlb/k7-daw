//! jobs_preview_fixes.rs — the K1 review's audition fixes 2, 3 and 4, each with the case that fails without it (2345, 2345c).
//! A reader gets wrong that these build their own worlds: the recorded scenarios of jobs_preview.rs put the project in the workspace's folder.

use crate::support::jobs::Tmp;
use k7_file_service::confine::{Disk, project_roots};
use k7_file_service::jobs::preview::{
    ClipTable, DocClip, DocSource, PreviewCtx, PreviewError, PreviewReq, disk_defs, disk_exists,
    resolve_preview,
};
use k7_file_service::libraries::Home;
use song::PathRule;
use song::clips::audio_def::ClipGrid;

/// A document that compiles to no clips, or fails.
struct Doc(bool);

struct NoClips;

impl ClipTable for NoClips {
    fn find(&self, _: &str) -> Option<DocClip<'_>> {
        None
    }

    fn grid(&self) -> ClipGrid {
        ClipGrid {
            tempo: 120.0,
            num: 4.0,
            den: 4.0,
        }
    }
}

impl DocSource for Doc {
    fn table(&self) -> Result<&dyn ClipTable, PreviewError> {
        if self.0 {
            Ok(&NoClips)
        } else {
            Err(PreviewError {
                message: "the project does not compile".into(),
                code: None,
            })
        }
    }
}

const PROJECT: &str = "project({ name: \"p\", tempo: 110, sampleRate: 48000, timeSignature: \"4/4\", video: false, layers: [] })\nmarkers({})\n";

/// Asks `req` of the project at `project` (under `ws`), the workspace's root being `ws` alone.
fn ask(tmp: &Tmp, project: &str, compiles: bool, req: PreviewReq) -> Result<String, String> {
    let home = Home {
        studio: tmp.at("studio"),
        libraries: tmp.at("libraries"),
    };
    let project = tmp.at(project);
    let roots = project_roots(&home, &project, &tmp.at("no-meta"), &tmp.at("no-models"));
    let defs = disk_defs(&roots, PathRule::HOST);
    let ctx = PreviewCtx {
        active: Some(&project),
        workspace_root: &tmp.at("ws"),
        home: &home,
        roots: &roots,
        probe: &Disk,
        doc: &Doc(compiles),
        defs: &defs,
        exists: &disk_exists,
    };
    resolve_preview(&ctx, &req)
        .map(|w| w.file)
        .map_err(|e| e.message)
}

fn source(s: String) -> PreviewReq {
    PreviewReq::Source { source: s }
}

#[test]
fn a_document_that_fails_leaves_the_clip_files_unread_and_the_name_a_path() {
    let tmp = Tmp::new("jobspreviewfix2");
    tmp.write("ws/p.k7/project.k7x", PROJECT);
    tmp.write("ws/p.k7/clips/kick.k7x", "source(\"clips/kick.wav\")\n");
    tmp.write("ws/p.k7/clips/kick.wav", "RIFF");
    // The same name with a document that compiles is the clip's source, from its file.
    assert_eq!(
        ask(&tmp, "ws/p.k7", true, source("kick".into())),
        Ok(tmp.at("ws/p.k7/clips/kick.wav"))
    );
    // The TypeScript's one `try` skips the clip files when the document threw: the name is read as a path.
    assert_eq!(
        ask(&tmp, "ws/p.k7", false, source("kick".into())),
        Err("no audio for 'kick'".into())
    );
}

#[test]
fn the_workspaces_own_root_is_allowed_for_a_project_that_lists_another() {
    let tmp = Tmp::new("jobspreviewfix4");
    tmp.write("ws/deep/p.k7/project.k7x", PROJECT);
    tmp.write("ws/outside.wav", "RIFF");
    // `project_roots` gives the project's parent here, `ws/deep`: the file sits in `ws`, which only the context's root names.
    assert_eq!(
        ask(
            &tmp,
            "ws/deep/p.k7",
            true,
            source("../../outside.wav".into())
        ),
        Ok(tmp.at("ws/outside.wav"))
    );
    assert_eq!(
        ask(&tmp, "ws/deep/p.k7", true, source(tmp.at("ws/outside.wav"))),
        Ok(tmp.at("ws/outside.wav"))
    );
}

#[cfg(unix)]
#[test]
fn a_dangling_link_is_no_audio_when_spelled_relative_and_outside_when_absolute_on_unix_only() {
    let tmp = Tmp::new("jobspreviewfix3");
    tmp.write("ws/p.k7/project.k7x", PROJECT);
    tmp.mkdir("ws/p.k7/clips");
    std::os::unix::fs::symlink(
        tmp.at("ws/p.k7/clips/missing.wav"),
        tmp.at("ws/p.k7/clips/dangling.wav"),
    )
    .unwrap();
    assert_eq!(
        ask(&tmp, "ws/p.k7", true, source("clips/dangling.wav".into())),
        Err("no audio for 'clips/dangling.wav'".into())
    );
    // DEPARTS 2345c: the absolute spelling is never stat'ed by the TypeScript, and a link it cannot follow is refused here.
    assert_eq!(
        ask(
            &tmp,
            "ws/p.k7",
            true,
            source(tmp.at("ws/p.k7/clips/dangling.wav"))
        ),
        Err("audio path is outside the workspace".into())
    );
}

// The workspace root may be spelled the host's way (backslashes); a path inside it is still inside.
#[test]
fn a_backslash_workspace_root_holds_an_absolute_audio_path() {
    let tmp = Tmp::new("jobspreviewback");
    let home = Home {
        studio: tmp.at("studio"),
        libraries: tmp.at("libraries"),
    };
    let project = tmp.at("ws/p.k7");
    let roots = project_roots(&home, &project, &tmp.at("no-meta"), &tmp.at("no-models"));
    let defs = disk_defs(&roots, PathRule::HOST);
    let ctx = PreviewCtx {
        active: Some(&project),
        workspace_root: "C:\\ws",
        home: &home,
        roots: &roots,
        probe: &Disk,
        doc: &Doc(true),
        defs: &defs,
        exists: &disk_exists,
    };
    let got = resolve_preview(&ctx, &source("C:/ws/kick.wav".into()));
    assert_eq!(got.map(|w| w.file), Ok("C:/ws/kick.wav".to_string()));
}
