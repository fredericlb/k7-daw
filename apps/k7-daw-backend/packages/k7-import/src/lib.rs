// k7-import — a foreign project translated into the files of a k7 project, in memory: the mapper by extension, the folder's name, the log.
// What a reader gets wrong: nothing here touches a disk; the file service reads the samples, lands the files and takes the folder away on failure (docs/import.md).

mod ableton;
mod als_devices;
mod als_zip;
mod dawproject;
mod emit;
mod emit_audio;
mod emit_ctx;
mod emit_graph;
mod emit_modules;
mod inflate;
pub mod model;
mod xml;

use std::path::Path;

pub use ableton::{ReadSample, SampleRef, decode_time_signature};
pub use emit::{EmitStats, World, emit};
pub use emit_ctx::Written;
pub use inflate::{MAX_ENTRIES, MAX_INFLATED_BYTES};
pub use model::{Bytes, Translation};

/// The foreign project formats that can be translated, by extension.
pub const IMPORTABLE: [&str; 3] = [".dawproject", ".als", ".zip"];

/// The subset a DROP may carry: a lone `.als` names its samples by path and is imported from the tree.
pub const DROPPABLE: [&str; 2] = [".dawproject", ".zip"];

/// The most a file of any format may weigh before it is read: the upload's own ceiling.
pub const MAX_IMPORT_BYTES: usize = 512 * 1024 * 1024;

/// What one translation reads: the file, its bytes, and where Live's samples come from.
pub struct Input<'a> {
    /// What the log calls the input: a workspace-relative path, or just a filename for a drop.
    pub source: &'a str,
    pub file: &'a str,
    pub bytes: &'a [u8],
    /// Only a `.als` uses it, and only for the samples it names by path.
    pub read_sample: Option<ReadSample<'a>>,
    /// The inflate ceiling of an archive, `MAX_INFLATED_BYTES` when unset.
    pub inflate_limit: Option<usize>,
}

/// The extension of a file name, lowercase, with its dot; empty when it has none.
#[must_use]
pub fn ext_of(file: &str) -> String {
    let name = file.rsplit(['/', '\\']).next().unwrap_or(file);
    match name.rfind('.') {
        Some(d) if d > 0 => name[d..].to_ascii_lowercase(),
        _ => String::new(),
    }
}

/// The file's name without its folder and its extension.
#[must_use]
pub fn stem_of(file: &str) -> String {
    let name = Path::new(&file.replace('\\', "/"))
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    match name.rfind('.') {
        Some(d) if d > 0 => name[..d].to_owned(),
        _ => name,
    }
}

/// Refuses a size the importer would not hold, before its bytes are read.
pub fn check_size(file: &str, size: usize) -> Result<(), String> {
    if size > MAX_IMPORT_BYTES {
        return Err(format!(
            "cannot import \"{file}\": larger than {} MB, the most an import takes",
            MAX_IMPORT_BYTES / (1 << 20)
        ));
    }
    Ok(())
}

/// A drop takes the self-contained formats; a lone `.als` is imported from the tree.
pub fn check_droppable(file: &str) -> Result<(), String> {
    let ext = ext_of(file);
    if DROPPABLE.contains(&ext.as_str()) {
        return Ok(());
    }
    let what = if ext.is_empty() { "file" } else { &ext };
    Err(format!(
        "cannot import \"{file}\": a dropped project must be one of {} — a {what} names its samples by path and has to sit beside them, so drop it into the workspace and import it from the tree",
        DROPPABLE.join(", ")
    ))
}

/// The mapper for the input's extension, run; the archives inflate no further than the ceiling.
pub fn translate(input: &Input<'_>) -> Result<Translation, String> {
    let stem = stem_of(input.file);
    match ext_of(input.file).as_str() {
        ".als" => ableton::parse_als(input.bytes, &stem, input.read_sample, input.inflate_limit),
        ".dawproject" => dawproject::parse_dawproject(input.bytes, &stem, input.inflate_limit),
        // A zip IS an Ableton project: the folder Collect All and Save leaves, its samples inside it.
        ".zip" => als_zip::parse_als_zip(input.bytes, &stem, input.inflate_limit).map(|(t, _)| t),
        _ => Err(format!(
            "cannot import \"{}\": expected one of {}",
            input.source,
            IMPORTABLE.join(", ")
        )),
    }
}

/// The new project's folder: the name somebody TYPED, refused when taken, or the file's own stem, made free with a suffix.
pub fn folder_name(
    file: &str,
    typed: Option<&str>,
    taken: &dyn Fn(&str) -> bool,
) -> Result<String, String> {
    let typed = typed.filter(|t| !t.is_empty());
    let flat = typed
        .map_or_else(|| stem_of(file), str::to_owned)
        .replace(['/', '\\'], "_");
    // A leading `_` is a shared folder's (`_modules`), so a project never starts with one.
    let flat = flat.trim_start_matches(['.', '_']);
    let base = if flat.is_empty() { "imported" } else { flat };
    if typed.is_some() {
        if taken(base) {
            return Err(format!("\"{base}\" already exists"));
        }
        return Ok(base.to_owned());
    }
    let mut name = base.to_owned();
    let mut n = 2;
    while taken(&name) {
        name = format!("{base}-{n}");
        n += 1;
    }
    Ok(name)
}

/// The lines the import says: what was read, what was written and what was lost.
#[must_use]
pub fn import_log(
    source: &str,
    project: &str,
    t: &Translation,
    stand_ins: usize,
    outside_workspace: usize,
) -> Vec<String> {
    use k7x::edit::js_number as n;
    let (song, stats) = (&t.song, &t.stats);
    let mut log = vec![
        format!("read {source}"),
        format!(
            "project \"{}\" — {} bpm, {}, {} Hz",
            song.name,
            n(song.tempo),
            song.time_signature,
            n(song.sample_rate)
        ),
        format!(
            "{} tracks · {} audio clips · {} MIDI clips · {} notes · {} assets",
            stats.tracks, stats.audio_clips, stats.midi_clips, stats.notes, stats.assets
        ),
    ];
    // Said apart from the per-sample warnings because this is the one with a FIX: Collect All and Save.
    if outside_workspace > 0 {
        log.push(format!(
            "warning: {outside_workspace} sample(s) live outside the workspace and were not read — in Live, use File > Collect All and Save, then re-import"
        ));
    }
    log.extend(t.warnings.iter().map(|w| format!("warning: {w}")));
    if stand_ins > 0 {
        log.push(format!(
            "warning: {stand_ins} MIDI track(s) play a STAND-IN instrument — theirs did not cross over; replace it on the graph"
        ));
    }
    log.push(format!("wrote {project}"));
    log
}

/// A translation emitted as the project `project`: the files to land and the lines to say.
pub struct Imported {
    pub written: Written,
    pub stats: EmitStats,
    pub log: Vec<String>,
}

/// Emits `t` as `project`; `outside` counts the samples a `.als` named outside the workspace.
#[must_use]
pub fn emit_translation(
    source: &str,
    project: &str,
    t: &Translation,
    world: &World,
    outside: usize,
) -> Imported {
    let (written, stats) = emit(project, &t.song, &t.files, world);
    let log = import_log(source, project, t, stats.stand_ins, outside);
    Imported {
        written,
        stats,
        log,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_typed_name_is_refused_when_taken_and_a_derived_one_is_suffixed() {
        let taken = |n: &str| n == "song" || n == "song-2";
        assert_eq!(
            folder_name("a/song.dawproject", None, &taken).unwrap(),
            "song-3"
        );
        assert_eq!(
            folder_name("a/song.dawproject", Some("song"), &taken).unwrap_err(),
            "\"song\" already exists"
        );
        assert_eq!(
            folder_name("x.als", Some("__x/y"), &|_| false).unwrap(),
            "x_y"
        );
        assert_eq!(folder_name("...als", None, &|_| false).unwrap(), "imported");
    }

    #[test]
    fn a_lone_als_is_not_droppable_and_the_refusal_says_where_it_goes() {
        assert!(check_droppable("a.dawproject").is_ok());
        assert!(check_droppable("A.ZIP").is_ok());
        assert!(
            check_droppable("a.als")
                .unwrap_err()
                .contains("import it from the tree")
        );
    }

    #[test]
    fn a_file_past_the_ceiling_is_refused() {
        assert!(check_size("x.zip", MAX_IMPORT_BYTES).is_ok());
        assert!(
            check_size("x.zip", MAX_IMPORT_BYTES + 1)
                .unwrap_err()
                .contains("512 MB")
        );
    }
}
