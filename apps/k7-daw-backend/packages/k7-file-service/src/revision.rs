//! revision.rs — the inputs of the change token: the narrow walk of a project's sources, the files outside it
//! that a derived view reads, and the token's spelling (1310 to 1314). `projectWalk.ts`' `walkProject(dir, true)`.
//! What a reader gets wrong: a source is stat'ed through the listing only on Unix; NTFS's listing can hold stale figures.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::filter::{is_binary, skip};
use crate::libraries::Home;
use crate::paths::{join, resolve_all};
use crate::tree::Limits;

/// What the token reads of a project's own files: how many sources, the newest mtime, and the names (1311).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Summary {
    pub count: u64,
    /// In node's `mtimeMs`.
    pub newest: f64,
    /// The wrapping sum of FNV-1a 64 over each source's project-relative path.
    pub names: u64,
}

/// Everything a workspace's token depends on; the same inputs answer the same token (1310).
#[derive(Clone, Debug, PartialEq)]
pub struct Inputs {
    pub dir: Option<String>,
    pub mutations: u64,
    pub summary: Option<Summary>,
    pub outside: f64,
    pub unknowable: bool,
}

/// `REVISION_FILE`: `.k7x` and `.vstpreset`, without case.
pub fn is_source(name: &str) -> bool {
    let n = name.as_bytes();
    let ends = |ext: &str| {
        n.len() >= ext.len() && n[n.len() - ext.len()..].eq_ignore_ascii_case(ext.as_bytes())
    };
    ends(".k7x") || ends(".vstpreset")
}

fn fnv1a(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// `host/fs.rs`' `ms`: seconds times 1000 plus nanoseconds over 1e6, 0 when the time cannot be read.
pub fn ms(t: std::io::Result<SystemTime>) -> f64 {
    t.ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0.0, |d| {
            d.as_secs() as f64 * 1e3 + f64::from(d.subsec_nanos()) / 1e6
        })
}

/// One file the walk found, from a single stat (`WalkedFile`).
#[derive(Clone, Debug, PartialEq)]
pub struct Walked {
    pub abs: String,
    /// Below the walked folder, `/`-spelled.
    pub rel: String,
    pub size: u64,
    /// In node's `mtimeMs`.
    pub mtime_ms: f64,
    pub binary: bool,
    pub source: bool,
}

/// What `walk_project` met: the files, the sources' summary, and whether a limit cut it short (1314).
#[derive(Clone, Debug)]
pub struct Walk {
    pub files: Vec<Walked>,
    pub summary: Summary,
    pub reached: bool,
    /// Stats taken by the joined path: links, and every file off Unix; a count for the test of 1797.
    pub by_path: u64,
}

struct Walking<'a> {
    limits: &'a Limits,
    sources_only: bool,
    seen: u64,
    out: Walk,
}

/// A listed name, its type, and the metadata taken from the listing where that is trusted.
type Entry = (String, std::fs::FileType, Option<std::fs::Metadata>);

/// A folder being walked: its path, spelling below the root, depth, and the names still to visit.
struct Frame {
    abs: String,
    rel: String,
    depth: u32,
    names: std::vec::IntoIter<Entry>,
}

enum Listed {
    Names(Vec<Entry>),
    Unreadable,
    /// A limit stopped the walk.
    Stop,
}

impl Walking<'_> {
    /// One folder's entries, the hidden ones dropped, in the order the shim lists them.
    fn list(&mut self, abs: &str) -> Listed {
        // An unreadable folder is empty.
        let Ok(read) = std::fs::read_dir(abs) else {
            return Listed::Unreadable;
        };
        let mut names: Vec<Entry> = Vec::new();
        for e in read {
            let Ok(e) = e else { break };
            self.seen += 1;
            if self.seen > self.limits.max_seen {
                self.out.reached = true;
                return Listed::Stop;
            }
            let name = e.file_name().to_string_lossy().into_owned();
            if skip(&name) {
                continue;
            }
            let Ok(kind) = e.file_type() else { continue };
            // A non-link entry is stat'ed from its listing on Unix (1797); a link, and NTFS, by the joined path.
            // A name the shim spells otherwise (a `\`, bytes that are no UTF-8) names no file there: joined path.
            let wanted = !self.sources_only || is_source(&name);
            let plain = !name.contains('\\') && e.file_name().to_str().is_some();
            let meta = (cfg!(unix) && wanted && plain && kind.is_file())
                .then(|| e.metadata().ok())
                .flatten()
                .filter(|m| m.is_file());
            names.push((name, kind, meta));
        }
        // The shim sorts a listing by bytes on Unix only (`host/fs.rs`).
        #[cfg(unix)]
        names.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        Listed::Names(names)
    }

    /// Depth first with an explicit stack, so a deep tree cannot overflow (1051).
    fn run(&mut self, dir: &str) {
        let Listed::Names(names) = self.list(dir) else {
            return;
        };
        let mut stack = vec![Frame {
            abs: dir.to_string(),
            rel: String::new(),
            depth: 0,
            names: names.into_iter(),
        }];
        while let Some(top) = stack.last_mut() {
            let Some((name, kind, listed)) = top.names.next() else {
                stack.pop();
                continue;
            };
            let path = join(&[&top.abs, &name]);
            let at = if top.rel.is_empty() {
                name.clone()
            } else {
                format!("{}/{name}", top.rel)
            };
            // A link, a junction included, is never entered; a link to a folder is a file to the walk.
            if kind.is_dir() && !kind.is_symlink() {
                let depth = top.depth + 1;
                if depth > self.limits.max_depth {
                    self.out.reached = true;
                    return;
                }
                match self.list(&path) {
                    Listed::Stop => return,
                    Listed::Unreadable => {}
                    Listed::Names(n) => stack.push(Frame {
                        abs: path,
                        rel: at,
                        depth,
                        names: n.into_iter(),
                    }),
                }
                continue;
            }
            let source = is_source(&name);
            if self.sources_only && !source {
                continue;
            }
            // Raced away between the listing and the stat: the next walk sees it.
            let by_path = listed.is_none();
            let Some(meta) = listed.or_else(|| std::fs::metadata(&path).ok()) else {
                continue;
            };
            self.out.by_path += u64::from(by_path);
            let mtime_ms = ms(meta.modified());
            if source {
                let sum = &mut self.out.summary;
                sum.count += 1;
                sum.newest = sum.newest.max(mtime_ms);
                sum.names = sum.names.wrapping_add(fnv1a(&at));
            }
            self.out.files.push(Walked {
                abs: path,
                rel: at,
                size: meta.len(),
                mtime_ms,
                binary: is_binary(&name),
                source,
            });
        }
    }
}

/// `walkProject(dir, sourcesOnly)`: every file the workspace shows under `dir`, or only its sources (1731).
pub fn walk_project(dir: &str, limits: &Limits, sources_only: bool) -> Walk {
    let summary = Summary {
        count: 0,
        newest: 0.0,
        names: 0,
    };
    let mut w = Walking {
        limits,
        sources_only,
        seen: 0,
        out: Walk {
            files: Vec::new(),
            summary,
            reached: false,
            by_path: 0,
        },
    };
    w.run(dir);
    w.out
}

/// `walkProject(dir, true)`: the sources under `dir`; the bool is true when a limit was reached (1314).
pub fn walk_sources(dir: &str, limits: &Limits) -> (Summary, bool) {
    let w = walk_project(dir, limits, true);
    (w.summary, w.reached)
}

fn mtime_of(path: &str) -> f64 {
    std::fs::metadata(path).map_or(0.0, |m| ms(m.modified()))
}

/// The newest of the files outside the project a derived view reads (1313): never kept, three stats.
pub fn outside_newest(home: &Home) -> f64 {
    let models = match std::env::var("STUDIO_MODELS") {
        Ok(v) if !v.is_empty() => resolve_all(&[&v]),
        _ => join(&[&home.studio, "models"]),
    };
    [
        join(&[&home.studio, "exts.json"]),
        join(&[&home.studio, "preferences.yaml"]),
        models,
    ]
    .iter()
    .map(|p| mtime_of(p))
    .fold(0.0, f64::max)
}

/// The token: `<boot>.<epoch>`, or `none:<boot>.<epoch>` with no active project (1310).
pub fn spell(boot: &str, epoch: u64, active: bool) -> String {
    if active {
        format!("{boot}.{epoch}")
    } else {
        format!("none:{boot}.{epoch}")
    }
}
