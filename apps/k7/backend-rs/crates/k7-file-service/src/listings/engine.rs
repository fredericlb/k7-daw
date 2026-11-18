//! engine.rs — `@engine` as a library row: synthesised from the engine's plugin scan, not a folder (1424).
//! The song crate's own port of `engineModules` reads the scan, so the two cannot drift.
//! What a reader gets wrong: a leaf opens nothing, and a scan that cannot say lists the built-ins, so a row is nearly always there.

use std::collections::BTreeMap;

use indexmap::IndexMap;
use song::DiskSource;
use song::modules::{LoadedModule, SCAN_CAP, Scan, engine_modules};

use super::quoted;
use crate::libraries::Home;
use crate::paths::join;
use crate::space::Declined;

/// The engine's scan cache, under the studio home (the C++ engine writes the same file).
const EXTS_FILE: &str = "exts.json";
/// The fewest bytes a scanned plugin is written in, `{"name":1,"format":1},`: a smaller file cannot pass the cap.
const MIN_PLUGIN_BYTES: u64 = 20;

/// The engine's plugin scan as `exts.json` holds it; declined where the song crate's reader says less than the TypeScript (1424).
pub fn engine_scan(home: &Home) -> Result<Scan, Declined> {
    let exts = join(&[&home.studio, EXTS_FILE]);
    let on_disk = std::fs::symlink_metadata(&exts).ok();
    if on_disk.as_ref().is_some_and(std::fs::Metadata::is_symlink) {
        return Err(Declined::NotRegular);
    }
    let scan = DiskSource::with_files(&[] as &[&str], &[exts.as_str()])
        .ok()
        .and_then(|source| Scan::read(&source, Some(&exts)).ok())
        .unwrap_or_default();
    let cap_bytes = (SCAN_CAP as u64).saturating_mul(MIN_PLUGIN_BYTES);
    if scan.is_empty() && on_disk.is_some_and(|m| m.is_file() && m.len() >= cap_bytes) {
        return Err(Declined::TooMany);
    }
    Ok(scan)
}

/// Every module the engine offers, by ref: the scan, or the built-ins when it cannot say.
pub fn engine_catalogue(home: &Home) -> Result<IndexMap<String, LoadedModule>, Declined> {
    engine_scan(home).map(|scan| engine_modules(&scan))
}

/// The `@engine` row as JSON, `None` with no module.
pub fn engine_row(home: &Home) -> Result<Option<String>, Declined> {
    let scan = engine_scan(home)?;
    // Groups and leaves sort in code point order, which a `BTreeMap` of `String`s keeps.
    let mut by_type: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for (r, m) in engine_modules(&scan) {
        let category = m.category.clone().unwrap_or_else(|| "audio".into());
        by_type
            .entry(category)
            .or_default()
            .push((m.file.clone(), r));
    }
    if by_type.is_empty() {
        return Ok(None);
    }
    let mut out = br#"{"ns":"engine","path":"","exists":true,"readOnly":true,"nodes":["#.to_vec();
    for (i, (kind, mut leaves)) in by_type.into_iter().enumerate() {
        if i > 0 {
            out.push(b',');
        }
        leaves.sort();
        out.extend_from_slice(b"{\"name\":");
        quoted(&mut out, &kind);
        out.extend_from_slice(b",\"path\":");
        quoted(&mut out, &format!("@engine/{kind}"));
        out.extend_from_slice(b",\"dir\":true,\"children\":[");
        for (j, (file, r)) in leaves.iter().enumerate() {
            if j > 0 {
                out.push(b',');
            }
            out.extend_from_slice(b"{\"name\":");
            quoted(&mut out, file);
            out.extend_from_slice(b",\"path\":");
            quoted(&mut out, r);
            out.extend_from_slice(b",\"dir\":false,\"kind\":\"text\"}");
        }
        out.extend_from_slice(b"]}");
    }
    out.extend_from_slice(b"]}");
    Ok(String::from_utf8(out).ok())
}
