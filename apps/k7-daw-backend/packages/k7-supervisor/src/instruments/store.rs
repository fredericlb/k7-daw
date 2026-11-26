//! store.rs — an instrument's folder on disk: the marker that says which archive it came from, the unpack of a
//! zip into place, and the removal that takes what a cut install left. Blocking: callers run it on a worker.
//! What a reader gets wrong: the marker is written LAST by the install and deleted FIRST by a removal, so
//! a cut either way reads as not installed, never as a whole instrument.

use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

use super::Instrument;

/// Written last into an instrument's folder: the archive hash it came from, which `installed` compares.
pub(super) const MARKER: &str = ".k7-archive";

fn marker(root: &Path, i: &Instrument) -> String {
    fs::read_to_string(root.join(&i.name).join(MARKER))
        .ok()
        .and_then(|s| s.lines().next().map(str::to_string))
        .unwrap_or_default()
}

/// Whether `i` is unpacked, AND from the archive this list pins: an older one is fetched again.
pub fn installed(root: &Path, i: &Instrument) -> bool {
    marker(root, i) == i.sha256
}

/// Whether ANY archive of `i` was installed here, the pinned one or an older one: what a first boot keeps.
pub fn present(root: &Path, i: &Instrument) -> bool {
    !marker(root, i).is_empty()
}

fn beside(root: &Path, i: &Instrument, suffix: &str) -> PathBuf {
    root.join(format!("{}{suffix}", i.name))
}

/// Delete `i`'s folder and whatever a cut-short install left beside it; Err names what stays.
pub fn remove(root: &Path, i: &Instrument) -> Result<(), String> {
    let folder = root.join(&i.name);
    let _ = fs::remove_file(folder.join(MARKER));
    for p in [
        folder,
        beside(root, i, ".unpack"),
        beside(root, i, ".zip.part"),
    ] {
        let gone = if p.is_dir() {
            fs::remove_dir_all(&p)
        } else {
            fs::remove_file(&p)
        };
        if let Err(e) = gone
            && e.kind() != std::io::ErrorKind::NotFound
        {
            return Err(format!("cannot delete {}: {e}", p.display()));
        }
    }
    Ok(())
}

/// The hex SHA-256 of a file, empty when it cannot be read.
pub fn sha256_of(file: &Path) -> String {
    let Ok(mut f) = fs::File::open(file) else {
        return String::new();
    };
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        match f.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => hash.update(&buf[..n]),
            Err(_) => return String::new(),
        }
    }
    hash.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// A name that would land outside the folder: absolute, or climbing out with `..`.
fn escapes(name: &str) -> bool {
    let rel = Path::new(&name.replace('\\', "/")).to_path_buf();
    rel.is_absolute()
        || rel.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::Prefix(_) | Component::RootDir
            )
        })
}

/// Unpack `zip` into `folder`: a sibling first, renamed when whole; one top folder dropped. Err on any refusal.
pub fn unpack_zip(zip: &Path, folder: &Path) -> Result<(), String> {
    let name = folder.file_name().map(|n| n.to_string_lossy().into_owned());
    let into = folder.with_file_name(format!("{}.unpack", name.unwrap_or_default()));
    let _ = fs::remove_dir_all(&into);
    fs::create_dir_all(&into).map_err(|e| e.to_string())?;
    let done = extract(zip, &into).and_then(|()| put_in_place(&into, folder));
    let _ = fs::remove_dir_all(&into);
    done
}

fn extract(zip: &Path, into: &Path) -> Result<(), String> {
    let file = fs::File::open(zip).map_err(|_| format!("cannot open {}", zip.display()))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|_| format!("not a zip archive: {}", zip.display()))?;
    for n in 0..archive.len() {
        let mut entry = archive
            .by_index(n)
            .map_err(|_| "unreadable entry in the archive".to_string())?;
        let name = entry.name().to_string();
        if escapes(&name) {
            return Err(format!("the archive names a file outside it: {name}"));
        }
        let at = into.join(name.replace('\\', "/"));
        if entry.is_dir() {
            fs::create_dir_all(&at).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = at.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = fs::File::create(&at).map_err(|_| format!("could not unpack {name}"))?;
        std::io::copy(&mut entry, &mut out).map_err(|_| format!("could not unpack {name}"))?;
    }
    Ok(())
}

/// One folder and nothing else at the top, what GitHub serves for a repository, is the instrument itself.
fn put_in_place(into: &Path, folder: &Path) -> Result<(), String> {
    let top: Vec<PathBuf> = fs::read_dir(into)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    let source = match top.as_slice() {
        [one] if one.is_dir() => one.as_path(),
        _ => into,
    };
    let _ = fs::remove_dir_all(folder);
    if let Some(parent) = folder.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::rename(source, folder).map_err(|e| format!("could not move the instrument into place: {e}"))
}
