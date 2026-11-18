// cache.rs — decoded files kept by path and settled stamp, an Arc each, so a compile re-reads only what moved.
// A reader would get wrong what is keyed: only a SETTLED stamp, since a write of the same size in the same tick would reproduce it.

use crate::assemblies::{AssemblyFile, AssemblyTemplate, decode_assembly, decode_assembly_template};
use crate::curves::CurveFile;
use crate::files::{Clip, Finding, RootFile, Sequence, Settings, Severity, Timeline};
use crate::modules::panels::{ControllerFile, UiFile, decode_controller, decode_ui_file};
use crate::modules::{Module, Vocabulary, build_vocabulary};
use crate::readset::{ReadSet, Watch};
use crate::roots::Roots;
use crate::routing::{ChainFile, Layout, Segments, decode_chain, decode_graph, decode_layout};
use crate::source::path::PathRule;
use crate::source::{Kind, Source, SourceError, Span, Stamp, Stat};
use crate::views::surfaces::{SurfaceFile, decode_surface};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

/// How many vocabularies are held, the least recently used dropped.
const MAX_VOCABULARIES: usize = 4;

/// How many measured lengths are held before the map is dropped whole.
const MAX_LENGTHS: usize = 100_000;

/// The parse cache is dropped whole once the texts it holds pass this many bytes.
const MAX_BYTES: usize = 64 * 1024 * 1024;

/// A decoded file, or its finding.
pub type Read<T> = Result<Arc<T>, Finding>;

/// A kind of file the cache can decode and hold.
pub trait FileValue: Sized + Send + Sync + 'static {
    /// The folder of this kind under the project, or "" for a root file.
    const DIR: &'static str;

    /// `rule` is the path rule of the roots the cache serves, which a chain's lane names are checked by.
    fn decode(file: &str, text: &str, rule: PathRule) -> Result<Self, Finding>;
    fn slot(cache: &mut Cache) -> &mut Slot<Self>;
}

/// One held decode: its stamp, its size, its value, and its text so a Recorder can be handed it on a hit.
type Held<T> = (Stamp, usize, Read<T>, Option<Arc<str>>);

pub struct Slot<T> {
    held: HashMap<String, Held<T>>,
}

impl<T> Default for Slot<T> {
    fn default() -> Self {
        Slot { held: HashMap::new() }
    }
}

pub struct Cache {
    rule: PathRule,
    curves: Slot<CurveFile>,
    clips: Slot<Clip>,
    sequences: Slot<Sequence>,
    timelines: Slot<Timeline>,
    roots: Slot<RootFile>,
    settings: Slot<Settings>,
    modules: Slot<Module>,
    panels: Slot<UiFile>,
    maps: Slot<ControllerFile>,
    graphs: Slot<Segments>,
    chains: Slot<ChainFile>,
    layouts: Slot<Layout>,
    surfaces: Slot<SurfaceFile>,
    lengths: HashMap<String, (Stamp, Option<f64>)>,
    digests: HashMap<String, Digested>,
    assemblies: Slot<AssemblyFile>,
    templates: Slot<AssemblyTemplate>,
    vocabularies: VecDeque<VocabSlot>,
    hits: usize,
    bytes: usize,
}

impl Default for Cache {
    fn default() -> Self {
        Cache::with_paths(PathRule::HOST)
    }
}

/// A digested file: its stamp, the bytes it counts, its 16 hex and, when it was UTF-8, its text for a Recorder.
struct Digested {
    stamp: Stamp,
    size: usize,
    hex: String,
    text: Option<Arc<str>>,
}

/// A vocabulary and what it was read from, under the roots it was made for.
struct VocabSlot {
    key: String,
    vocabulary: Arc<Vocabulary>,
    set: ReadSet,
}

macro_rules! file_value {
    ($ty:ty, $dir:expr, $decode:path, $slot:ident) => {
        impl FileValue for $ty {
            const DIR: &'static str = $dir;

            fn decode(file: &str, text: &str, _rule: PathRule) -> Result<Self, Finding> {
                $decode(file, text)
            }

            fn slot(cache: &mut Cache) -> &mut Slot<Self> {
                &mut cache.$slot
            }
        }
    };
}

file_value!(CurveFile, "curves", crate::curves::decode_curve, curves);
file_value!(Clip, "clips", crate::files::decode_clip, clips);
file_value!(Sequence, "sequences", crate::files::decode::<Sequence>, sequences);
file_value!(Timeline, "timelines", crate::files::decode::<Timeline>, timelines);
file_value!(RootFile, "", crate::files::decode_root, roots);
file_value!(Settings, "", crate::files::decode::<Settings>, settings);
file_value!(Module, "nodes", crate::modules::decode_module, modules);
file_value!(UiFile, "", decode_ui_file, panels);
file_value!(ControllerFile, "", decode_controller, maps);
file_value!(Segments, "", decode_graph, graphs);

impl FileValue for ChainFile {
    const DIR: &'static str = "chains";

    fn decode(file: &str, text: &str, rule: PathRule) -> Result<Self, Finding> {
        decode_chain(file, text, rule)
    }

    fn slot(cache: &mut Cache) -> &mut Slot<Self> {
        &mut cache.chains
    }
}

file_value!(Layout, "", decode_layout, layouts);
file_value!(SurfaceFile, "surfaces", decode_surface, surfaces);
file_value!(AssemblyFile, "assemblies", decode_assembly, assemblies);
file_value!(AssemblyTemplate, "", decode_assembly_template, templates);

impl Cache {
    pub fn new() -> Cache {
        Cache::default()
    }

    /// A cache whose decoders read names by `rule`, the one of the roots it is asked for.
    pub fn with_paths(rule: PathRule) -> Cache {
        Cache {
            rule,
            curves: Slot::default(),
            clips: Slot::default(),
            sequences: Slot::default(),
            timelines: Slot::default(),
            roots: Slot::default(),
            settings: Slot::default(),
            modules: Slot::default(),
            panels: Slot::default(),
            maps: Slot::default(),
            graphs: Slot::default(),
            chains: Slot::default(),
            layouts: Slot::default(),
            surfaces: Slot::default(),
            lengths: HashMap::new(),
            digests: HashMap::new(),
            assemblies: Slot::default(),
            templates: Slot::default(),
            vocabularies: VecDeque::new(),
            hits: 0,
            bytes: 0,
        }
    }

    /// The rule the decoders read by; a cache asked under another rule forgets what it decoded under this one.
    pub fn paths(&self) -> PathRule {
        self.rule
    }

    pub fn set_paths(&mut self, rule: PathRule) {
        if rule != self.rule {
            self.forget();
            self.rule = rule;
        }
    }

    /// The bytes the cache holds, texts and vocabularies together.
    pub fn bytes(&self) -> usize {
        self.bytes + self.vocabulary_bytes()
    }

    /// The file's decoded value, from the cache while its stamp is settled and unchanged.
    pub fn read<T: FileValue>(&mut self, source: &dyn Source, path: &str) -> Result<Read<T>, SourceError> {
        let st = source.stat(path)?;
        if st.kind != Kind::File {
            return Err(SourceError::Io(format!("{path}: not a file")));
        }
        if st.stamp.settled
            && let Some((stamp, _, held, text)) = T::slot(self).held.get(path)
            && *stamp == st.stamp
        {
            match text {
                Some(text) => source.note_text(path, text),
                // A file that was not UTF-8 holds no text: a Recorder learns it again by reading it.
                None => drop(source.read(path, Span::All)),
            }
            return Ok(held.clone());
        }
        let bytes = source.read(path, Span::All)?;
        let size = bytes.len();
        let mut kept: Option<Arc<str>> = None;
        let read: Read<T> = match String::from_utf8(bytes) {
            Ok(text) => {
                let decoded = T::decode(path, &text, self.rule).map(Arc::new);
                let shared: Arc<str> = Arc::from(text);
                // One copy serves the cache and a read set: the source swaps its own for this.
                source.note_text(path, &shared);
                kept = Some(shared);
                decoded
            }
            Err(_) => Err(Finding {
                file: path.to_owned(),
                line: 1,
                col: 1,
                message: "the file is not valid UTF-8".to_owned(),
                severity: Severity::Error,
            }),
        };
        if st.stamp.settled {
            if let Some((_, old, _, _)) = T::slot(self)
                .held
                .insert(path.to_owned(), (st.stamp, size, read.clone(), kept))
            {
                self.bytes = self.bytes.saturating_sub(old);
            }
            self.bytes += size;
            if self.bytes + self.vocabulary_bytes() > MAX_BYTES {
                self.clear();
            }
        }
        Ok(read)
    }

    /// A held audio length for `path`, while its stamp is settled and equal; the inner None is a file with no length.
    pub fn length_of(&self, path: &str, stamp: Stamp) -> Option<Option<f64>> {
        match self.lengths.get(path) {
            Some((held, value)) if stamp.settled && *held == stamp => Some(*value),
            _ => None,
        }
    }

    /// Holds a measured length by its settled stamp; an unsettled one holds nothing, and the map is dropped past its limit.
    pub fn hold_length(&mut self, path: &str, stamp: Stamp, value: Option<f64>) {
        if !stamp.settled {
            return;
        }
        if self.lengths.len() >= MAX_LENGTHS {
            self.lengths.clear();
        }
        self.lengths.insert(path.to_owned(), (stamp, value));
    }

    /// The first 16 hex of the SHA-256 of a file's bytes, kept while its settled stamp holds (decision 345).
    pub fn digest(&mut self, source: &dyn Source, path: &str, stat: &Stat) -> Result<String, SourceError> {
        if stat.stamp.settled
            && let Some(d) = self.digests.get(path)
            && d.stamp == stat.stamp
        {
            if let Some(text) = &d.text {
                source.note_text(path, text);
            }
            source.note_digest(path, &d.hex);
            return Ok(d.hex.clone());
        }
        // A replay holds a binary by its length: the digest its bundle recorded stands for the bytes.
        if let Some(hex) = source.known_digest(path) {
            source.note_digest(path, &hex);
            return Ok(hex);
        }
        let bytes = source.read(path, Span::All)?;
        let hex = hex16(&bytes);
        source.note_digest(path, &hex);
        let text = std::str::from_utf8(&bytes).ok().map(Arc::<str>::from);
        // Only the kept text is held; a binary keeps its 16 hex and weighs nothing here.
        let size = text.as_ref().map_or(0, |t| t.len());
        if let Some(text) = &text {
            source.note_text(path, text);
        }
        if stat.stamp.settled {
            let held = Digested {
                stamp: stat.stamp,
                size,
                hex: hex.clone(),
                text,
            };
            if let Some(old) = self.digests.insert(path.to_owned(), held) {
                self.bytes = self.bytes.saturating_sub(old.size);
            }
            self.bytes += size;
            if self.bytes + self.vocabulary_bytes() > MAX_BYTES {
                self.clear();
            }
        }
        Ok(hex)
    }

    /// Forgets every decoded file.
    pub fn clear(&mut self) {
        self.curves.held.clear();
        self.clips.held.clear();
        self.sequences.held.clear();
        self.timelines.held.clear();
        self.roots.held.clear();
        self.settings.held.clear();
        self.modules.held.clear();
        self.panels.held.clear();
        self.maps.held.clear();
        self.graphs.held.clear();
        self.chains.held.clear();
        self.layouts.held.clear();
        self.surfaces.held.clear();
        self.lengths.clear();
        self.digests.clear();
        self.assemblies.held.clear();
        self.templates.held.clear();
        self.bytes = 0;
    }

    /// Forgets everything: the decoded files and every vocabulary.
    pub fn forget(&mut self) {
        self.clear();
        self.vocabularies.clear();
    }

    /// Lets go of all but the vocabulary used last, as a host short of memory does.
    pub fn shed(&mut self) {
        self.clear();
        self.vocabularies.truncate(1);
    }

    /// The bytes the held vocabularies keep of what they read.
    fn vocabulary_bytes(&self) -> usize {
        self.vocabularies.iter().map(|v| v.set.text_bytes()).sum()
    }

    /// How many times a vocabulary was reused.
    pub fn vocabulary_hits(&self) -> usize {
        self.hits
    }

    /// How many vocabularies are held.
    pub fn vocabularies(&self) -> usize {
        self.vocabularies.len()
    }

    /// The module vocabulary under `roots`, reused while what it read has not moved, and rebuilt otherwise.
    pub fn vocabulary(&mut self, source: &dyn Source, roots: &Roots) -> Result<Arc<Vocabulary>, SourceError> {
        let key = roots.vocabulary_key();
        if let Some(at) = self.vocabularies.iter().position(|v| v.key == key)
            && self.vocabularies[at].set.valid(source)
            && let Some(slot) = self.vocabularies.remove(at)
        {
            self.hits += 1;
            slot.set.replay(source);
            let held = slot.vocabulary.clone();
            self.vocabularies.push_front(slot);
            return Ok(held);
        }
        let watch = Watch::new(source);
        let vocabulary = Arc::new(build_vocabulary(&watch, self, roots)?);
        let set = watch.into_set();
        // A set holding an unsettled stamp is never a key: a write of the same size in the same tick would reproduce it.
        self.vocabularies.retain(|v| v.key != key);
        if set.settled() {
            self.vocabularies.push_front(VocabSlot {
                key,
                vocabulary: vocabulary.clone(),
                set,
            });
            self.vocabularies.truncate(MAX_VOCABULARIES);
            // The vocabularies count against the same limit as the decoded files: the oldest go first.
            while self.vocabularies.len() > 1 && self.bytes + self.vocabulary_bytes() > MAX_BYTES {
                self.vocabularies.pop_back();
            }
        }
        Ok(vocabulary)
    }

    /// How many decoded files are held.
    pub fn len(&self) -> usize {
        self.clips.held.len()
            + self.curves.held.len()
            + self.sequences.held.len()
            + self.timelines.held.len()
            + self.roots.held.len()
            + self.settings.held.len()
            + self.modules.held.len()
            + self.panels.held.len()
            + self.maps.held.len()
            + self.graphs.held.len()
            + self.chains.held.len()
            + self.layouts.held.len()
            + self.surfaces.held.len()
            + self.assemblies.held.len()
            + self.templates.held.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The first 16 hex digits of the SHA-256 of `bytes`.
fn hex16(bytes: &[u8]) -> String {
    let sum = Sha256::digest(bytes);
    sum.iter().take(8).map(|b| format!("{b:02x}")).collect()
}
