// ops/song.rs — the song.* operations: a bundle opened into a handle, drafts laid over it, the views compiled from it.
// A reader would get wrong that a file the bundle lacks is an error naming it (`needs_files`), never an empty read.

use crate::error::{ErrorCode, WasmError};
use crate::ops::write;
use serde::Deserialize;
use serde_json::Value;
use song::doc::Vsts;
use song::graph::Folder;
use song::session::{Ask, Session, View};
use song::source::{Entry, Span, Stat};
use song::{MemSource, Source, SourceBundle, SourceError};
use std::cell::RefCell;
use std::collections::HashMap;

/// How many projects stay open at once.
const MAX_OPEN: usize = 4;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenArgs {
    pub bundle: SourceBundle,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Draft {
    pub path: String,
    /// The new text; absent removes the file.
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UpdateArgs {
    pub handle: u32,
    pub files: Vec<Draft>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileArgs {
    pub handle: u32,
    pub want: Vec<View>,
    #[serde(default)]
    pub rev: Option<String>,
    #[serde(default)]
    pub thawed: Option<Vec<String>>,
    /// The graph of a folder other than the open project.
    #[serde(default)]
    pub at: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CloseArgs {
    pub handle: u32,
}

/// The song family's requests: `{"op": "song.open", "args": {…}}`.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "op", content = "args")]
pub enum SongRequest {
    #[serde(rename = "song.open")]
    Open(Box<OpenArgs>),
    #[serde(rename = "song.update")]
    Update(UpdateArgs),
    #[serde(rename = "song.compile")]
    Compile(CompileArgs),
    #[serde(rename = "song.close")]
    Close(CloseArgs),
}

pub fn dispatch(songs: &mut SongTable, request: SongRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    match request {
        SongRequest::Open(a) => write(out, &songs.open(*a)?),
        SongRequest::Update(a) => write(out, &songs.update(a)?),
        SongRequest::Compile(a) => songs.compile(a, out),
        SongRequest::Close(a) => write(out, &songs.close(a)?),
    }
}

/// One open project: the bundle's memory, the compile's session and the request the bundle was made for.
struct Open {
    source: MemSource,
    session: Session,
    ask: Ask,
    tick: u64,
}

/// What outlives a call: the open projects.
#[derive(Default)]
pub struct SongTable {
    open: HashMap<u32, Open>,
    next: u32,
    tick: u64,
}

/// A source that remembers the paths it could not answer, so a failed compile names what to fetch.
struct Watching<'a> {
    inner: &'a MemSource,
    missing: RefCell<Vec<String>>,
}

impl Watching<'_> {
    fn note<T>(&self, path: &str, r: Result<T, SourceError>) -> Result<T, SourceError> {
        if matches!(r, Err(SourceError::Missing)) {
            let mut m = self.missing.borrow_mut();
            if !m.iter().any(|p| p == path) {
                m.push(path.to_owned());
            }
        }
        r
    }
}

impl Source for Watching<'_> {
    fn stat(&self, path: &str) -> Result<Stat, SourceError> {
        self.note(path, self.inner.stat(path))
    }

    fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError> {
        self.note(dir, self.inner.list(dir))
    }

    fn read(&self, path: &str, span: Span) -> Result<Vec<u8>, SourceError> {
        self.note(path, self.inner.read(path, span))
    }

    fn stamps_are_times(&self) -> bool {
        self.inner.stamps_are_times()
    }

    fn folder_id(&self, path: &str) -> Option<String> {
        self.inner.folder_id(path)
    }

    fn known_digest(&self, path: &str) -> Option<String> {
        self.inner.known_digest(path)
    }

    fn known_stale(&self, source: &str) -> Option<bool> {
        self.inner.known_stale(source)
    }

    fn known_seconds(&self, path: &str) -> Option<f64> {
        self.inner.known_seconds(path)
    }

    fn measured_by_table(&self) -> bool {
        self.inner.measured_by_table()
    }
}

fn refused(e: SourceError) -> WasmError {
    WasmError::bad_request(format!("the bundle is refused: {e}"))
}

impl SongTable {
    fn get(&mut self, handle: u32) -> Result<&mut Open, WasmError> {
        self.tick += 1;
        let tick = self.tick;
        let open = self
            .open
            .get_mut(&handle)
            .ok_or_else(|| WasmError::new(ErrorCode::UnknownHandle, format!("no open project {handle}")))?;
        open.tick = tick;
        Ok(open)
    }

    /// Opens a bundle; its own request names the roots and the rig, and a bundle with none is refused.
    pub fn open(&mut self, a: OpenArgs) -> Result<Value, WasmError> {
        let source = MemSource::from_bundle(a.bundle).map_err(refused)?;
        let request = source
            .request()
            .cloned()
            .ok_or_else(|| WasmError::bad_request("the bundle carries no request (roots and rig)"))?;
        let vsts = if request.vsts == "none" { Vsts::None } else { Vsts::Scan };
        let roots = request.roots.clone();
        let rule = roots.paths;
        let ask = Ask {
            roots,
            rig: request.rig.clone(),
            vsts,
            thawed: request.thawed.clone(),
            rev: None,
            folder: Folder::Open,
        };
        if self.open.len() >= MAX_OPEN
            && let Some(oldest) = self.open.iter().min_by_key(|(_, o)| o.tick).map(|(h, _)| *h)
        {
            self.open.remove(&oldest);
        }
        self.next = self.next.wrapping_add(1).max(1);
        self.tick += 1;
        self.open.insert(
            self.next,
            Open {
                source,
                session: Session::new(rule),
                ask,
                tick: self.tick,
            },
        );
        Ok(serde_json::json!({ "handle": self.next }))
    }

    /// Lays texts over the open project: a draft per file, or a removal.
    pub fn update(&mut self, a: UpdateArgs) -> Result<Value, WasmError> {
        let open = self.get(a.handle)?;
        for d in a.files {
            open.source.set(&d.path, d.text).map_err(refused)?;
        }
        Ok(serde_json::json!({}))
    }

    /// The views asked, as `song.compile` answers them on a disk: `{views, reused}`.
    pub fn compile(&mut self, a: CompileArgs, out: &mut Vec<u8>) -> Result<(), WasmError> {
        let open = self.get(a.handle)?;
        let mut ask = open.ask.clone();
        ask.rev = a.rev;
        if let Some(t) = a.thawed {
            ask.thawed = t;
        }
        ask.folder = if a.at { Folder::Other } else { Folder::Open };
        let watching = Watching {
            inner: &open.source,
            missing: RefCell::new(Vec::new()),
        };
        let reply = open.session.answer(&watching, &ask, &a.want);
        let missing = watching.missing.take();
        let reply = match reply {
            Ok(r) => r,
            Err(SourceError::Missing) => return Err(WasmError::needs_files(missing)),
            Err(e) => return Err(WasmError::bad_request(format!("the compile: {e}"))),
        };
        // A view's text is JSON already: it is joined, never parsed.
        let views: Vec<String> = reply
            .views
            .iter()
            .map(|(v, t)| format!("{}:{t}", serde_json::to_string(v).unwrap_or_default()))
            .collect();
        out.clear();
        out.extend_from_slice(format!("{{\"views\":{{{}}},\"reused\":{}}}", views.join(","), reply.reused).as_bytes());
        Ok(())
    }

    pub fn close(&mut self, a: CloseArgs) -> Result<Value, WasmError> {
        self.open
            .remove(&a.handle)
            .ok_or_else(|| WasmError::new(ErrorCode::UnknownHandle, format!("no open project {}", a.handle)))?;
        Ok(Value::Null)
    }
}
