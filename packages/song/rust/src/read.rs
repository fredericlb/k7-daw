// read.rs — a project's families and its head read through a Source: values by id, the findings of the files that failed.
// A reader would get wrong that a broken file is left out and named, not an error: the rest of the family still reads.

use crate::cache::{Cache, FileValue, Read};
use crate::files::{Finding, Marker, RootFile, Settings, SettingsDoc, Severity, markers_of, time_sig_of};
use crate::jsorder::{own_key_sort, pair_cmp};
use crate::source::path::join;
use crate::source::{Kind, Source, SourceError};
use std::collections::HashMap;
use std::sync::Arc;

/// What a family folder holds: each file's value by id in listing order, and the findings of those that did not decode.
pub struct FamilyRead<T> {
    pub values: Vec<(String, Arc<T>)>,
    /// The file name of each value, in the same order (`a.K7X` keeps its case).
    pub names: Vec<String>,
    pub findings: Vec<Finding>,
}

impl<T> FamilyRead<T> {
    pub fn get(&self, id: &str) -> Option<&Arc<T>> {
        self.values.iter().find(|(i, _)| i == id).map(|(_, v)| v)
    }
}

/// A `.k7x` name's base, in any case of the extension; a bare `.k7x` is not a document.
pub(crate) fn k7x_base(name: &str) -> Option<&str> {
    let cut = name.len().checked_sub(4)?;
    let (base, ext) = (name.get(..cut)?, name.get(cut..)?);
    (!base.is_empty() && ext.eq_ignore_ascii_case(".k7x")).then_some(base)
}

/// The documents a listing holds as (base, file name) in JavaScript's order: pairs sorted bare, integer-like bases first (decisions 381, 391).
pub(crate) fn doc_files(listing: &[crate::source::Entry]) -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = Vec::new();
    let mut at: HashMap<&str, usize> = HashMap::new();
    for entry in listing.iter().filter(|e| e.kind == Kind::File) {
        let Some(id) = k7x_base(&entry.name) else { continue };
        match at.get(id) {
            Some(&i) => files[i].1 = entry.name.clone(),
            None => {
                at.insert(id, files.len());
                files.push((id.to_owned(), entry.name.clone()));
            }
        }
    }
    files.sort_by(|a, b| pair_cmp((&a.0, &a.1), (&b.0, &b.1)));
    let mut keys: Vec<String> = files.iter().map(|f| f.0.clone()).collect();
    own_key_sort(&mut keys);
    let by_base: HashMap<String, String> = files.into_iter().collect();
    keys.into_iter()
        .filter_map(|k| by_base.get(&k).cloned().map(|name| (k, name)))
        .collect()
}

/// `s` cut to `n` characters, so a message never carries a whole path of a hostile length.
fn cut_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// A missing folder or one that cannot be listed is empty; a path the Source refuses is not.
fn absent_is_empty<T: Default>(r: Result<T, SourceError>) -> Result<T, SourceError> {
    match r {
        Err(SourceError::NotFound | SourceError::Io(_)) => Ok(T::default()),
        other => other,
    }
}

/// Every `.k7x` file of a project's family folder, decoded; the project is an absolute path.
pub fn read_family<T: FileValue>(
    source: &dyn Source,
    cache: &mut Cache,
    project: &str,
) -> Result<FamilyRead<T>, SourceError> {
    let mut out = FamilyRead {
        values: Vec::new(),
        names: Vec::new(),
        findings: Vec::new(),
    };
    let folder = join(project, T::DIR);
    let is_dir = absent_is_empty(source.stat(&folder).map(|s| s.kind == Kind::Dir))?;
    if !is_dir {
        return Ok(out);
    }
    let listing = absent_is_empty(source.list(&folder))?;
    let files = doc_files(&listing);
    for (id, name) in files {
        let id = id.as_str();
        let entry_name = name;
        let read: Read<T> = match cache.read::<T>(source, &join(&folder, &entry_name)) {
            Ok(r) => r,
            Err(SourceError::NotFound) => continue,
            // A file the Source refuses is skipped alone, with a warning naming it (decision 24); the rest still reads.
            Err(SourceError::Outside) => {
                out.findings.push(Finding {
                    file: format!("{}/{}", T::DIR, entry_name),
                    line: 1,
                    col: 1,
                    message: "outside the project's folders — skipped".to_owned(),
                    severity: Severity::Warning,
                });
                continue;
            }
            // A file the disk will not read costs that file alone (decision 248).
            Err(SourceError::Io(why)) => {
                out.findings.push(Finding {
                    file: format!("{}/{}", T::DIR, entry_name),
                    line: 1,
                    col: 1,
                    message: format!("the file could not be read ({})", cut_chars(&why, 80)),
                    severity: Severity::Error,
                });
                continue;
            }
            Err(e) => return Err(e),
        };
        match read {
            Ok(value) => {
                out.values.push((id.to_owned(), value));
                out.names.push(entry_name);
            }
            Err(finding) => out
                .findings
                .push(finding.in_file(&format!("{}/{}", T::DIR, entry_name))),
        }
    }
    Ok(out)
}

/// The project's root and its settings, as a ProjectDoc carries them.
pub struct Head {
    /// project.k7x, when it is there; its value or its refusal.
    pub root: Option<Arc<RootFile>>,
    pub markers: Vec<Marker>,
    pub settings: SettingsDoc,
    /// The first finding of project.k7x and of settings.k7x, each named by its file name.
    pub findings: Vec<Finding>,
    /// What the TypeScript raises in the compile and not in the read: a time signature that does not make a bar.
    pub compile_findings: Vec<Finding>,
    pub format_too_new: bool,
}

/// A root file's decoded value, or None when the folder has no such file.
fn root_file<T: FileValue>(
    source: &dyn Source,
    cache: &mut Cache,
    project: &str,
    name: &str,
) -> Result<Option<Read<T>>, SourceError> {
    let path = join(project, name);
    match source.stat(&path) {
        Ok(s) if s.kind == Kind::File => {}
        Ok(_) | Err(SourceError::NotFound | SourceError::Io(_)) => return Ok(None),
        Err(e) => return Err(e),
    }
    cache.read::<T>(source, &path).map(Some)
}

/// Reads `project.k7x` and `settings.k7x`; a newer format refuses the read before any schema runs.
pub fn read_head(source: &dyn Source, cache: &mut Cache, project: &str) -> Result<Head, SourceError> {
    let mut head = Head {
        root: None,
        markers: Vec::new(),
        settings: SettingsDoc::default(),
        findings: Vec::new(),
        compile_findings: Vec::new(),
        format_too_new: false,
    };
    match root_file::<RootFile>(source, cache, project, "project.k7x")? {
        None => {}
        Some(Err(finding)) => head.findings.push(finding.in_file("project.k7x")),
        Some(Ok(root)) => {
            if let Some(too_new) = &root.too_new {
                head.format_too_new = true;
                head.findings.push(too_new.finding.in_file("project.k7x"));
                head.root = Some(root);
                return Ok(head);
            }
            if let Some(file) = &root.project {
                let text = file
                    .project
                    .as_ref()
                    .and_then(|p| p.time_signature.as_ref())
                    .map(|t| t.0.as_str());
                match time_sig_of(text) {
                    Ok(sig) => head.markers = file.markers.as_ref().map(|m| markers_of(m, sig)).unwrap_or_default(),
                    Err(message) => {
                        let (line, col) = root.time_signature_at.unwrap_or((1, 1));
                        head.compile_findings.push(Finding {
                            file: "project.k7x".to_owned(),
                            line,
                            col,
                            message,
                            severity: crate::files::Severity::Error,
                        });
                    }
                }
            }
            head.root = Some(root);
        }
    }
    match root_file::<Settings>(source, cache, project, "settings.k7x")? {
        None => {}
        Some(Err(finding)) => head.findings.push(finding.in_file("settings.k7x")),
        Some(Ok(settings)) => head.settings = settings.doc(),
    }
    Ok(head)
}
