// host.rs — what a verb acts through: the files, the compiled project, its two channels of lines, its effects and the front's routes.
// A reader would get wrong that `call` waits: a call with no answer yet ends the run as `pending`, and the caller runs the line again with it.

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

/// A route that answered with an error: its sentence, the code a verb may word (`no-engine`, `no-ffmpeg`, `off`…) and its status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failed {
    pub error: String,
    pub code: Option<String>,
    pub status: u16,
}

impl Failed {
    /// `{"error", "code"?, "status"?}` is a failure, anything else the route's answer.
    fn of(value: Json) -> Result<Json, Failed> {
        let Some(error) = value.get("error").and_then(Json::as_str) else {
            return Ok(value);
        };
        Err(Failed {
            error: error.to_owned(),
            code: value.get("code").and_then(Json::as_str).map(str::to_owned),
            status: value
                .get("status")
                .and_then(Json::as_u64)
                .and_then(|s| u16::try_from(s).ok())
                .unwrap_or(500),
        })
    }
}

pub struct Host<'a> {
    pub fs: &'a mut dyn Fs,
    pub env: Env,
    /// The project folder, absolute in the `Fs`'s spelling.
    pub song_path: String,
    pub log: Vec<String>,
    pub refused: Vec<String>,
    pub effects: Vec<Effect>,
    /// Routes the caller asks after landing the writes, in order: `{"route": "POST /verb-host/engine", "args": {...}}`.
    pub asks: Vec<Json>,
    /// The answers of the routes this line called on its earlier runs, in call order: `{route, args, value}`.
    pub answers: Vec<Json>,
    /// How many of `answers` this run has used.
    used: usize,
    /// The first call this run had no answer for: the run stops there and lands nothing.
    pub pending: Option<Json>,
    /// The decoded files `project()` reuses while their stamps hold: a verb's second compile re-reads only what it wrote.
    pub cache: Cache,
    pub new_id: &'a mut dyn FnMut() -> String,
    /// The line's `YYMMDDTHHMMSS`, the caller's clock read once per line; None: one is made from a fresh id.
    pub stamp: Option<String>,
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
            answers: Vec::new(),
            used: 0,
            pending: None,
            cache: Cache::new(),
            new_id,
            stamp: None,
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

    /// Asks the caller to call the front's `route` with `args` once the writes landed; the answer's `lines` are said, nothing else returns.
    pub fn ask(&mut self, route: &str, args: Json) {
        self.asks.push(serde_json::json!({ "route": route, "args": args }));
    }

    /// The answer of the front's `route` (`"POST /verb-host/render"`) to `args`; with none yet the run is pending (docs/verbs-port.md).
    ///
    /// # Errors
    /// `Thrown` when the answer is not there yet: the verb returns with `?`, and the caller runs the line again with it.
    pub fn call(&mut self, route: &str, args: Json) -> Result<Result<Json, Failed>, Thrown> {
        if self.pending.is_none()
            && let Some(a) = self.answers.get(self.used)
            && a.get("route").and_then(Json::as_str) == Some(route)
            && a.get("args") == Some(&args)
        {
            self.used += 1;
            let value = a.get("value").cloned().unwrap_or(Json::Null);
            return Ok(Failed::of(value));
        }
        if self.pending.is_none() {
            self.pending = Some(serde_json::json!({ "route": route, "args": args }));
        }
        Err(Thrown(format!("{route}: waiting for the answer")))
    }

    /// The stamp a render's name carries, the same for the whole run; with no clock given, digits drawn from a fresh id.
    pub fn file_stamp(&mut self) -> String {
        if let Some(s) = &self.stamp {
            return s.clone();
        }
        let id = (self.new_id)();
        let mut digits = id
            .bytes()
            .filter(u8::is_ascii_hexdigit)
            .map(|b| char::from(b'0' + b % 10))
            .cycle();
        let mut part = |n: usize| (0..n).filter_map(|_| digits.next()).collect::<String>();
        let made = format!("{}T{}", part(6), part(6));
        self.stamp = Some(made.clone());
        made
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
