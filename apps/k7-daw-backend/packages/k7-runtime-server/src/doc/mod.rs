//! doc — a `ProjectDoc` off the wire, checked: parsed with nlohmann's refusals, every number bounded, repeated ids
//! renamed, every path held to the engine's roots; the result is the JSON the C ABI's `apply` reads
//! (`engine/DocReader.cpp` in k7-runtime), which has no checks of its own. `docs/project.md` holds the bounds.
//! What a reader gets wrong: this is a trust boundary, since the engine binds every interface under the
//! supervisor, so a number the engine divides by or loops over is bounded here and nowhere after.

mod bounds;
mod plugin;
mod project;
mod read;
mod track;

pub use bounds::*;
pub use project::{Parsed, SLOT_SESSION, parse_project, parse_tracks};
pub use read::{Obj, Read};

use crate::engine::Engine;
use std::cell::RefCell;

type Warn<'a> = Box<dyn Fn(&str, &str) + 'a>;

/// What the parse asks of the engine: whether a path may be read or written, and where a warning goes.
pub struct Checks<'a> {
    readable: Box<dyn Fn(&str) -> bool + 'a>,
    writable: Box<dyn Fn(&str) -> bool + 'a>,
    warn: Warn<'a>,
    /// Every path dropped for lying outside the roots: what a `render` refuses whole.
    pub refused: RefCell<Vec<String>>,
}

impl<'a> Checks<'a> {
    /// `warn` is told the message and then the subject (empty for none).
    pub fn new(
        readable: impl Fn(&str) -> bool + 'a,
        writable: impl Fn(&str) -> bool + 'a,
        warn: impl Fn(&str, &str) + 'a,
    ) -> Self {
        Checks {
            readable: Box::new(readable),
            writable: Box::new(writable),
            warn: Box::new(warn),
            refused: RefCell::default(),
        }
    }

    /// The engine's own roots and warning sink.
    pub fn of(engine: &Engine) -> Checks<'static> {
        let (r, w, s) = (engine.clone(), engine.clone(), engine.clone());
        Checks::new(
            move |p| r.path_allowed(p, false),
            move |p| w.path_allowed(p, true),
            move |message, subject| s.warn("", subject, message),
        )
    }

    pub(crate) fn warn(&self, message: &str, subject: &str) {
        (self.warn)(message, subject);
    }

    fn note_refused(&self, path: &str) {
        self.refused.borrow_mut().push(path.to_owned());
    }

    /// A file the document names for READING, kept only when the engine may open it, else empty and a warning.
    pub(crate) fn readable(&self, path: String, what: &str) -> String {
        if path.is_empty() || (self.readable)(&path) {
            return path;
        }
        self.note_refused(&path);
        self.warn(
            &format!("{what} '{path}' is outside the folders this engine reads — ignored"),
            "",
        );
        String::new()
    }

    /// The `.vstpreset` under the write roots a plugin's state is saved to, nothing else.
    pub(crate) fn preset_target(&self, path: String) -> String {
        if path.is_empty() || (has_extension_ci(&path, ".vstpreset") && (self.writable)(&path)) {
            return path;
        }
        self.note_refused(&path);
        self.warn(
            &format!(
                "preset save path '{path}' is not a .vstpreset under the folders this engine writes — not saved"
            ),
            "",
        );
        String::new()
    }

    /// The folder takes are WRITTEN to, empty and a warning when outside the write roots.
    pub(crate) fn record_dir(&self, dir: String) -> String {
        if dir.is_empty() || (self.writable)(&dir) {
            return dir;
        }
        self.note_refused(&dir);
        self.warn(
            &format!(
                "record folder '{dir}' is outside the folders this engine writes — recording is off"
            ),
            "",
        );
        String::new()
    }

    /// An `@instruments/` reference resolves under the state directory, so no segment of it may be `..`.
    pub(crate) fn instrument_ref(&self, reference: String) -> String {
        const PREFIX: &str = "@instruments/";
        let Some(rest) = reference.strip_prefix(PREFIX) else {
            return self.readable(reference, "sfz instrument");
        };
        if rest.split(['/', '\\']).any(|segment| segment == "..") {
            self.note_refused(&reference);
            self.warn(
                &format!(
                    "sfz instrument '{reference}' climbs out of the instruments folder — ignored"
                ),
                "",
            );
            return String::new();
        }
        reference
    }
}

/// Whether `path` ends in `ext` (with its dot), ignoring ASCII case.
fn has_extension_ci(path: &str, ext: &str) -> bool {
    path.len() >= ext.len()
        && path.as_bytes()[path.len() - ext.len()..].eq_ignore_ascii_case(ext.as_bytes())
}
