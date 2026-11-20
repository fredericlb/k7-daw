//! pack.rs — the archive of the tracks export: audio stored as it is, the small files deflated, written to a file the route streams.
//! What a reader gets wrong: a WAV is moved into the archive and its source removed as it goes, so the folder never holds the audio twice.

use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

/// What an entry holds.
pub enum Body {
    Text(Vec<u8>),
    /// A rendered WAV: stored, and deleted once copied.
    Wav(PathBuf),
}

pub struct Entry {
    pub name: String,
    pub body: Body,
}

/// `entries` written to `out` in order; blocking.
pub fn write_zip(out: &Path, entries: Vec<Entry>) -> Result<(), String> {
    let file = File::create(out).map_err(|e| e.to_string())?;
    let mut zip = ZipWriter::new(io::BufWriter::new(file));
    for e in entries {
        let stored = matches!(e.body, Body::Wav(_));
        let method = if stored {
            CompressionMethod::Stored
        } else {
            CompressionMethod::Deflated
        };
        let opts = SimpleFileOptions::default()
            .compression_method(method)
            .large_file(stored);
        zip.start_file(e.name.as_str(), opts)
            .map_err(|e| e.to_string())?;
        match e.body {
            Body::Text(bytes) => zip.write_all(&bytes).map_err(|e| e.to_string())?,
            Body::Wav(path) => {
                let mut f = File::open(&path).map_err(|e| e.to_string())?;
                io::copy(&mut f, &mut zip).map_err(|e| e.to_string())?;
                drop(f);
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    let mut inner = zip.finish().map_err(|e| e.to_string())?;
    inner.flush().map_err(|e| e.to_string())
}
