//! preview.rs — `resolvePreview` of `previewSource.ts`: a play button's request turned into a file and a window (2345).
//! What a reader gets wrong: the compiled document is asked for LAZILY (the TypeScript compiled on every audition), and
//! the TypeScript's quirks are copied (2345b): an absolute audio path is never stat'ed, a folder named `x.wav` plays.
//! It departs in one place (DEPARTS 2345c, pending the user): every file is also confined through its links.

use crate::confine::{Disk, Probe, confine_to};
use crate::libraries::{Home, library_of};
use crate::paths::{is_within, relative, resolve, resolve_all, slashed};
use song::PathRule;
use song::clips::ClipKind;
use song::clips::audio_def::{AudioClipDef, ClipGrid, audio_clip_def, material_ref_of};
use song::doc::ProjectDoc;

/// What an audition asks (`PreviewReq`).
#[derive(Clone, Debug, PartialEq)]
pub enum PreviewReq {
    Clip { clip: String },
    Source { source: String },
    Slice { source: String, from: f64, to: f64 },
}

/// The file and the window of it that plays, in seconds.
#[derive(Clone, Debug, PartialEq)]
pub struct Window {
    pub file: String,
    pub start: f64,
    pub length: f64,
}

/// A refusal: the TypeScript's sentence, and its `code` when a read failed (`ENOENT` is a 404, the rest a 400).
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewError {
    pub message: String,
    pub code: Option<String>,
}

impl PreviewError {
    fn said(message: &str) -> PreviewError {
        PreviewError {
            message: message.to_owned(),
            code: None,
        }
    }

    pub fn status(&self) -> u16 {
        if self.code.as_deref() == Some("ENOENT") {
            404
        } else {
            400
        }
    }
}

/// A clip of the compiled document, as an audition reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DocClip<'a> {
    pub audio: bool,
    pub source: &'a str,
    pub source_from: Option<f64>,
    pub source_to: Option<f64>,
}

/// The document's clips and its grid; a recorded table in a test, the compiled `ProjectDoc` in the front.
pub trait ClipTable {
    fn find(&self, id: &str) -> Option<DocClip<'_>>;
    fn grid(&self) -> ClipGrid;
}

impl ClipTable for ProjectDoc {
    fn find(&self, id: &str) -> Option<DocClip<'_>> {
        let c = self
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .find(|c| c.id == id)?;
        Some(DocClip {
            audio: c.kind == ClipKind::Audio,
            source: &c.source,
            source_from: c.source_from,
            source_to: c.source_to,
        })
    }

    fn grid(&self) -> ClipGrid {
        ClipGrid {
            tempo: self.tempo,
            num: f64::from(self.time_sig_numerator),
            den: f64::from(self.time_sig_denominator),
        }
    }
}

/// The active project's document, produced only when an audition needs it (at most once per request).
pub trait DocSource {
    fn table(&self) -> Result<&dyn ClipTable, PreviewError>;
}

/// `loadAudioClipDoc`: the project, the id and the grid in, the clip or the TypeScript's thrown sentence out.
pub type DefsFn<'a> =
    dyn Fn(&str, &str, Option<ClipGrid>) -> Result<Option<AudioClipDef>, String> + 'a;

/// What an audition reads of the world; every member is the workspace's own.
pub struct PreviewCtx<'a> {
    pub active: Option<&'a str>,
    pub workspace_root: &'a str,
    pub home: &'a Home,
    /// What a path a project's files name may reach (`project_roots`); the workspace's own root is added to it.
    pub roots: &'a [String],
    pub probe: &'a dyn Probe,
    pub doc: &'a dyn DocSource,
    /// `loadAudioClipDoc` over the project's files.
    pub defs: &'a DefsFn<'a>,
    /// `existsSync`: every error is "no".
    pub exists: &'a dyn Fn(&str) -> bool,
}

/// `AUDIO_EXT`: a file the engine's audio readers take, by its extension in any case.
fn audio_ext(spec: &str) -> bool {
    let Some(dot) = spec.rfind('.') else {
        return false;
    };
    ["wav", "mp3", "ogg", "flac", "aif", "aiff", "m4a"]
        .iter()
        .any(|e| spec[dot + 1..].eq_ignore_ascii_case(e))
}

/// The shim's `isAbsolute`: a root of `/` or of `C:/`, with `\` read as `/`.
fn is_absolute(p: &str) -> bool {
    let s = slashed(p);
    let b = s.as_bytes();
    s.starts_with('/')
        || (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'/')
}

impl PreviewCtx<'_> {
    fn within_root(&self, abs: &str) -> bool {
        is_within(&resolve_all(&[self.workspace_root]), abs)
    }

    /// Is `abs` inside an account library's `samples/`, `irs/` or `wavetables/`, and nowhere else of theirs?
    fn in_library_sounds(&self, abs: &str) -> bool {
        let Some(lib) = library_of(self.home, abs) else {
            return false;
        };
        let rel = relative(&lib.root, abs);
        matches!(
            rel.split('/').next(),
            Some("samples" | "irs" | "wavetables")
        )
    }

    /// The project's roots and the workspace's own, which a project outside `<studio>/workspace` does not list.
    fn allowed(&self) -> Vec<&str> {
        let mut roots: Vec<&str> = self.roots.iter().map(String::as_str).collect();
        roots.push(self.workspace_root);
        roots
    }

    /// A path a song file names, inside the project's roots or nothing, links followed.
    fn confined(&self, source: &str, dir: &str) -> Option<String> {
        if source.is_empty() {
            return None;
        }
        confine_to(&self.allowed(), &resolve(dir, source), self.probe)
    }

    fn confined_source(&self, source: &str, dir: &str) -> Result<String, PreviewError> {
        self.confined(source, dir)
            .ok_or_else(|| PreviewError::said("audio path is outside the project"))
    }

    /// DEPARTS 2345c: a path inside the workspace or a library is also followed through its links.
    fn through_links(&self, abs: &str, relative: bool) -> Result<String, PreviewError> {
        // Nothing there (a name with a NUL, a file not made) has no link to follow; the sentence is the TypeScript's.
        if !self.probe.present(abs) {
            return Ok(abs.to_owned());
        }
        match confine_to(&self.allowed(), abs, self.probe) {
            Some(real) => Ok(real),
            // A relative spelling of a dangling link answers as the TypeScript does (`existsSync` is false); an absolute one is refused.
            None if relative && !(self.exists)(abs) => Ok(abs.to_owned()),
            None => Err(PreviewError::said("audio path is outside the workspace")),
        }
    }

    fn resolve_source(&self, spec: &str, dir: &str) -> Result<String, PreviewError> {
        if spec.is_empty() {
            return Err(PreviewError::said("preview: empty source"));
        }
        if is_absolute(spec) && audio_ext(spec) {
            let abs = resolve_all(&[spec]);
            // A library module's own audio is the other place a preview may play from.
            if !self.within_root(&abs) && library_of(self.home, &abs).is_none() {
                return Err(PreviewError::said("audio path is outside the workspace"));
            }
            return self.through_links(&abs, false);
        }
        // Not a clip id, or a document that does not compile: try it as a plain path below.
        let clip = match self.doc.table() {
            Err(_) => None,
            Ok(t) => match t.find(spec) {
                Some(c) => Some((c.audio, c.source.to_owned())),
                None => (self.defs)(dir, spec, None)
                    .ok()
                    .flatten()
                    .map(|d| (true, d.source)),
            },
        };
        if let Some((true, source)) = clip {
            return self.confined_source(&source, dir);
        }
        let abs = resolve(dir, spec);
        if !self.within_root(&abs) && !self.in_library_sounds(&abs) {
            return Err(PreviewError::said("audio path is outside the workspace"));
        }
        let abs = self.through_links(&abs, true)?;
        if !(self.exists)(&abs) {
            return Err(PreviewError::said(&format!("no audio for '{spec}'")));
        }
        Ok(abs)
    }

    fn clip_window(&self, id: &str, dir: &str) -> Result<Window, PreviewError> {
        let table = self.doc.table()?;
        let found = table
            .find(id)
            .map(|c| (c.audio, c.source.to_owned(), c.source_from, c.source_to));
        let (audio, source, from, to) = match found {
            Some(c) => c,
            None => {
                // A placement id names the material, whose definition is keyed without the `@`.
                match (self.defs)(dir, material_ref_of(id), Some(table.grid())) {
                    Err(why) => {
                        return Err(PreviewError {
                            message: why,
                            code: None,
                        });
                    }
                    Ok(None) => return Err(PreviewError::said(&format!("no clip '{id}'"))),
                    Ok(Some(d)) => (true, d.source, d.source_from, d.source_to),
                }
            }
        };
        if !audio {
            return Err(PreviewError::said(&format!("'{id}' is not an audio clip")));
        }
        let start = from.unwrap_or(0.0).max(0.0);
        let to = to.unwrap_or(0.0);
        Ok(Window {
            file: self.confined_source(&source, dir)?,
            start,
            length: if to > start { to - start } else { 0.0 },
        })
    }
}

/// The file and window a request plays, or why not.
pub fn resolve_preview(ctx: &PreviewCtx<'_>, req: &PreviewReq) -> Result<Window, PreviewError> {
    let Some(dir) = ctx.active else {
        return Err(PreviewError::said(
            "no active project — select a project in the workspace first",
        ));
    };
    match req {
        PreviewReq::Clip { clip } => ctx.clip_window(clip, dir),
        PreviewReq::Slice { source, from, to } => {
            let from = from.max(0.0);
            Ok(Window {
                file: ctx.resolve_source(source, dir)?,
                start: from,
                length: (to - from).max(0.0),
            })
        }
        PreviewReq::Source { source } => Ok(Window {
            file: ctx.resolve_source(source, dir)?,
            start: 0.0,
            length: 0.0,
        }),
    }
}

/// `loadAudioClipDoc` over the folders `roots` mount, as the front calls it.
pub fn disk_defs(
    roots: &[String],
    rule: PathRule,
) -> impl Fn(&str, &str, Option<ClipGrid>) -> Result<Option<AudioClipDef>, String> {
    let roots = roots.to_vec();
    move |project, id, grid| {
        let source = song::DiskSource::new(&roots).map_err(|e| e.to_string())?;
        audio_clip_def(&source, project, id, grid, rule)
    }
}

/// `existsSync` on the disk.
pub fn disk_exists(path: &str) -> bool {
    std::fs::metadata(path).is_ok()
}

/// The real disk's probe for the confinement.
pub const DISK: Disk = Disk;
