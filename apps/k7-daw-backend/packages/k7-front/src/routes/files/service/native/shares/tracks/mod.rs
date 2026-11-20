//! tracks — `/export/tracks`: each track of a project as audio without effects, a MIDI file and a page of its chain, in one zip.
//! What a reader gets wrong: the audio is the engine's own stem render of a stripped document; the chain the page lists is the project's, not the audio's.

pub mod describe;
pub mod midi;
pub mod notes;
pub mod pack;
pub mod signal;
pub mod strip;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_engine;
#[cfg(test)]
mod tests_route;

use std::collections::HashMap;
use std::path::Path;

use k7_file_service::shares::names;
use serde_json::Value;

use super::disk::Folder;
use super::export::Mixed;
use super::{Account, Ports};
use describe::Files;
use pack::{Body, Entry};

/// The zip's mime type.
const ZIP: &str = "application/zip";

/// A track's name as a file name: what a file system refuses becomes `_`.
pub fn file_part(name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    // A file name over 255 bytes is refused when unzipping, so the name is cut well short of it.
    let cut: String = clean.chars().take(60).collect();
    let trimmed = cut.trim_matches(|c: char| c == ' ' || c == '.');
    if trimmed.is_empty() {
        "track".into()
    } else {
        trimmed.to_owned()
    }
}

/// The base of every file of track `index` (0-based) out of `count`: its number, then its name.
pub fn base_name(index: usize, count: usize, name: &str) -> String {
    let width = count.to_string().len().max(2);
    format!("{:0width$}-{}", index + 1, file_part(name))
}

/// Render the tracks of the project at `dir`, or of one `timeline` of it, and pack them in a folder of its own.
pub async fn export_tracks(
    acct: &Account,
    ports: &Ports,
    dir: &Path,
    timeline: Option<&str>,
) -> Result<Mixed, String> {
    if !ports.mixdown.available() {
        return Err(k7_file_service::shares::no_engine("tracks"));
    }
    let mixdoc = ports.views.mix_doc(dir, timeline).await?;
    let folder = Folder::make(&acct.tmp, "tbtracks-").map_err(|e| e.to_string())?;
    let stripped = strip::strip(&mixdoc.doc);
    let mut wavs: HashMap<usize, std::path::PathBuf> = HashMap::new();
    if !stripped.kept.is_empty() {
        let made = match ports
            .mixdown
            .render(&stripped.doc, folder.path(), true)
            .await
        {
            Ok(m) => Some(m),
            // A render with nothing audible leaves the stems out; the notes and pages still go.
            Err(e) if e.contains("produced no audio") => None,
            Err(e) => return Err(e),
        };
        if let Some(made) = &made {
            let _ = tokio::fs::remove_file(folder.path().join(&made.mix)).await;
        }
        for s in made.iter().flat_map(|m| &m.stems) {
            let index = s
                .name
                .strip_prefix('t')
                .and_then(|n| n.parse::<usize>().ok());
            if let Some(i) = index.filter(|i| stripped.kept.contains(i)) {
                wavs.insert(i, folder.path().join(&s.file));
            }
        }
    }
    let origins = {
        let (views, dir) = (ports.views.clone(), dir.to_owned());
        // Best effort: a project whose files will not read has its params listed with no origin.
        tokio::task::spawn_blocking(move || views.origins(&dir))
            .await
            .unwrap_or_default()
    };
    let entries = entries(&mixdoc.doc, &mut wavs, &origins)?;
    let out = folder.path().join("tracks.zip");
    let target = out.clone();
    tokio::task::spawn_blocking(move || pack::write_zip(&target, entries))
        .await
        .map_err(|e| e.to_string())??;
    let name = names::mix_name(&mixdoc.name, &dir.to_string_lossy(), timeline);
    Ok(Mixed::packed(
        out,
        format!("{name}-tracks.zip"),
        ZIP,
        folder,
    ))
}

/// The archive's entries in order: the README, then each track's audio, MIDI and page.
pub fn entries(
    doc: &Value,
    wavs: &mut HashMap<usize, std::path::PathBuf>,
    origins: &Value,
) -> Result<Vec<Entry>, String> {
    let tracks = doc["tracks"].as_array().cloned().unwrap_or_default();
    let mut listed = Vec::new();
    let mut files = Vec::new();
    for (i, t) in tracks.iter().enumerate() {
        let base = base_name(i, tracks.len(), t["name"].as_str().unwrap_or_default());
        let wav = wavs.remove(&i);
        let made = if t["generated"] == true {
            Vec::new()
        } else {
            notes::notes_of(t)
        };
        let mid = if made.is_empty() {
            None
        } else {
            Some(midi::midi_bytes(
                t["name"].as_str().unwrap_or_default(),
                doc,
                &made,
            )?)
        };
        let f = Files {
            wav: wav.as_ref().map(|_| format!("{base}.wav")),
            mid: mid.as_ref().map(|_| format!("{base}.mid")),
        };
        files.push(Entry {
            name: format!("{base}.md"),
            body: Body::Text(describe::track_page(doc, i, &f, origins).into_bytes()),
        });
        if let Some(path) = wav {
            files.push(Entry {
                name: format!("{base}.wav"),
                body: Body::Wav(path),
            });
        }
        if let Some(bytes) = mid {
            files.push(Entry {
                name: format!("{base}.mid"),
                body: Body::Text(bytes),
            });
        }
        listed.push((base, f));
    }
    let mut all = vec![Entry {
        name: "README.md".into(),
        body: Body::Text(describe::readme(doc, &listed, origins).into_bytes()),
    }];
    all.append(&mut files);
    Ok(all)
}
