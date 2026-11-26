// support — a translation landed in memory beside k7's own meta modules and compiled, as the page would compile what an import wrote.
// What a reader gets wrong: the library is only the two stand-in instruments an import can name, not the whole seed.

#![allow(dead_code)]

use std::io::Write;
use std::path::{Path, PathBuf};

use k7_import::Written;
use k7x::edit::Value;
use song::doc::ProjectDoc;
use song_edit::MemFs;
use song_edit::confine::Env;

pub const WS: &str = "/w/ws";
const META: &str = "/k7/meta";
const HOME: &str = "/home-none";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../..")
}

fn walk(root: &Path, rel: &str, out: &mut Vec<(String, Vec<u8>)>) {
    let Ok(entries) = std::fs::read_dir(root.join(rel)) else { return };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let sub = if rel.is_empty() { name } else { format!("{rel}/{name}") };
        if e.path().is_dir() {
            walk(root, &sub, out);
        } else if let Ok(bytes) = std::fs::read(e.path()) {
            out.push((sub, bytes));
        }
    }
}

/// What an import wrote, in a workspace with k7's meta modules and the library modules a stand-in names.
pub struct Landed {
    pub fs: MemFs,
    pub project: String,
}

pub fn land(project: &str, written: &Written) -> Landed {
    let r = repo();
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut add = |from: PathBuf, to: &str| {
        let mut found = Vec::new();
        walk(&from, "", &mut found);
        files.extend(found.into_iter().map(|(rel, b)| (format!("{to}/{rel}"), b)));
    };
    add(r.join("apps/k7-daw-frontend/packages/song/meta"), META);
    let lib = format!("{HOME}/libraries/k7-core");
    add(r.join("k7-core/modules/instruments/k7-rack"), &format!("{lib}/modules/instruments/k7-rack"));
    add(r.join("k7-core/modules/instruments/synth/surge-xt"), &format!("{lib}/modules/instruments/synth/surge-xt"));
    files.push((format!("{lib}/library.k7x"), std::fs::read(r.join("k7-core/library.k7x")).unwrap_or_default()));
    for (p, t) in &written.texts {
        files.push((format!("{WS}/{p}"), t.clone().into_bytes()));
    }
    for (p, b) in &written.binaries {
        files.push((format!("{WS}/{p}"), b.to_vec()));
    }
    Landed { fs: MemFs::from_tree("", files), project: format!("{WS}/{project}") }
}

impl Landed {
    /// What the engine would be sent for the project, or the first finding that stops it.
    pub fn compile(&mut self) -> Result<ProjectDoc, String> {
        let env = Env::at(HOME, META);
        let mut n = 0;
        let mut ids = || {
            n += 1;
            format!("id{n}")
        };
        verbs::Host::new(&mut self.fs, env, &self.project, &mut ids)
            .project()
            .map_err(|t| t.0)
    }

    pub fn text(&self, rel: &str) -> String {
        self.fs
            .text(&format!("{}/{rel}", self.project))
            .unwrap_or_else(|| panic!("no file {rel}"))
    }

    pub fn has(&self, rel: &str) -> bool {
        self.fs.text(&format!("{}/{rel}", self.project)).is_some()
    }

    /// A k7x file of the project read back as data.
    pub fn data(&self, rel: &str) -> Value {
        song_edit::workspace_doc::ws_data(&self.fs, &format!("{}/{rel}", self.project)).expect("a k7x file that reads")
    }
}

/// A zip of the entries, as a `.dawproject` or a zipped Ableton folder is.
pub fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default();
        for (name, bytes) in entries {
            z.start_file(*name, opts).unwrap();
            z.write_all(bytes).unwrap();
        }
        z.finish().unwrap();
    }
    buf.into_inner()
}

pub fn gz(xml: &str) -> Vec<u8> {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    e.write_all(xml.as_bytes()).unwrap();
    e.finish().unwrap()
}

/// A 16-bit mono wav of `seconds` of silence at 8 kHz, with a header the song crate reads the length from.
pub fn wav(seconds: f64) -> Vec<u8> {
    let rate: u32 = 8000;
    let data = (f64::from(rate) * seconds) as u32 * 2;
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    b.resize(b.len() + data as usize, 0);
    b
}
