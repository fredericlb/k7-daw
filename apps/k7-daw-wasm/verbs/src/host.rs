// host.rs — what a verb acts through: the files, the compiled project, its two channels of lines and the effects it asks.
// A reader would get wrong that `project()` is held: it compiles the files as they are NOW, its cache keyed by each file's stamp.

use serde_json::Value as Json;
use song::Cache;
use song::doc::{ProjectDoc, Request, Vsts, compile};
use song::lanes::rig::Rig;
use song_edit::Fs;
use song_edit::confine::Env;
use song_edit::loader::roots_for;

/// A throw that leaves the verb, as a TypeScript handler's rejected promise: the message only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Thrown(pub String);

impl From<song_edit::Error> for Thrown {
    fn from(e: song_edit::Error) -> Thrown {
        Thrown(e.to_string())
    }
}

/// An effect only the caller can perform: `reload` (read the files again) or `applyProject` (push the new mix).
pub type Effect = &'static str;

pub struct Host<'a> {
    pub fs: &'a mut dyn Fs,
    pub env: Env,
    /// The project folder, absolute in the `Fs`'s spelling.
    pub song_path: String,
    pub log: Vec<String>,
    pub refused: Vec<String>,
    pub effects: Vec<Effect>,
    /// Host members the caller performs after landing the writes, in order: `{"member": "transport", "args": ["play"]}`.
    pub asks: Vec<Json>,
    /// The decoded files `project()` reuses while their stamps hold: a verb's second compile re-reads only what it wrote.
    pub cache: Cache,
    pub new_id: &'a mut dyn FnMut() -> String,
}

impl<'a> Host<'a> {
    pub fn new(fs: &'a mut dyn Fs, env: Env, song_path: &str, new_id: &'a mut dyn FnMut() -> String) -> Host<'a> {
        Host {
            fs,
            env,
            song_path: song_path.to_owned(),
            log: Vec::new(),
            refused: Vec::new(),
            effects: Vec::new(),
            asks: Vec::new(),
            cache: Cache::new(),
            new_id,
        }
    }

    pub fn log(&mut self, lines: impl IntoIterator<Item = String>) {
        self.log.extend(lines);
    }

    /// Says NO, and why; the verb returns right after.
    pub fn refuse(&mut self, lines: impl IntoIterator<Item = String>) {
        self.refused.extend(lines);
    }

    pub fn reload(&mut self) {
        self.effects.push("reload");
    }

    pub fn apply_project(&mut self) {
        self.effects.push("applyProject");
    }

    /// Asks the caller to call host member `member` with `args`, as the TypeScript verb awaits it; its answer never comes back.
    pub fn ask(&mut self, member: &str, args: Json) {
        self.asks.push(serde_json::json!({ "member": member, "args": args }));
    }

    /// A wav's length in seconds, read from its header; None when it cannot be read.
    pub fn seconds(&self, file: &str) -> Option<f64> {
        let st = self.fs.stat(file).ok()?;
        let head = self.fs.read(file, song::Span::All).ok()?;
        song::views::wav::wav_seconds(&head, st.len)
    }

    /// The document the engine is sent for the project as its files say now; a compile that fails throws its first finding.
    pub fn project(&mut self) -> Result<ProjectDoc, Thrown> {
        let roots = roots_for(&*self.fs, &self.env, &self.song_path);
        let req = Request {
            roots,
            rig: Rig::default(),
            vsts: Vsts::Scan,
            thawed: Vec::new(),
        };
        let compiled = compile(&*self.fs, &mut self.cache, &req).map_err(|e| Thrown(e.to_string()))?;
        compiled.engine.map_err(|f| {
            Thrown(
                f.diagnostics
                    .first()
                    .map_or_else(|| "the project does not compile".to_owned(), |d| d.message.clone()),
            )
        })
    }
}
