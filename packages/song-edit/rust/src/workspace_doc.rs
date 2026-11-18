// workspace_doc.rs — a workspace's .k7x file read and written as k7x: its text, its data, or a document edited by key path.
// A reader would get wrong what an edit sees in a file with short forms: their EXPANDED data, put back by sugar's rule.

use crate::error::{Res, fail};
use crate::fs::Fs;
use crate::sugar::sugar_for;
use crate::workspace_file::{is_k7x_path, k7x_data, ws_read, ws_write};
use k7x::edit::{Document, StringifyOptions, Value, apply_sugared, parse, stringify};

/// How `ws_edit` treats the file: `create` lets an absent one be empty, `raw` hands the callback the short forms as written.
#[derive(Clone, Copy, Default)]
pub struct EditOpts {
    pub create: bool,
    pub raw: bool,
}

fn k7x_only(path: &str) -> Res<()> {
    if is_k7x_path(path) {
        Ok(())
    } else {
        fail(format!("{path}: not a .k7x file"))
    }
}

/// Writes `text` unless the file already holds it; says whether it wrote.
fn write_if_changed(fs: &mut dyn Fs, path: &str, text: &str) -> Res<bool> {
    if fs.exists(path) && ws_read(fs, path)? == text {
        return Ok(false);
    }
    ws_write(fs, path, text.as_bytes())?;
    Ok(true)
}

/// The file's k7x text as it is on disk.
///
/// # Errors
/// A path that is not .k7x, or the read's.
pub fn ws_text(fs: &dyn Fs, path: &str) -> Res<String> {
    k7x_only(path)?;
    ws_read(fs, path)
}

/// The file's data, its short forms expanded.
///
/// # Errors
/// A syntax error, or the read's.
pub fn ws_data(fs: &dyn Fs, path: &str) -> Res<Value> {
    k7x_data(path, &ws_text(fs, path)?)
}

/// Runs `edit` on the file as a document and writes it when its text changed; says whether it wrote.
///
/// # Errors
/// An absent file without `create`, a file that does not read, or what `edit` fails with.
pub fn ws_edit(fs: &mut dyn Fs, path: &str, opts: EditOpts, edit: impl FnOnce(&mut Document) -> Res<()>) -> Res<bool> {
    k7x_only(path)?;
    let exists = fs.exists(path);
    if !exists && !opts.create {
        return fail(format!("{path}: no such file"));
    }
    let before = if exists { ws_read(fs, path)? } else { String::new() };
    let mut doc = Document::new(&before);
    let sugar = if opts.raw { None } else { sugar_for(path) };
    let raw = doc.to_js()?;
    match sugar.filter(|s| k7x::edit::Sugar::parts(s).iter().any(|p| (p.present)(&raw))) {
        // A file with no short form in it, or an edit that keeps them itself, edits the document as written.
        None => edit(&mut doc)?,
        Some(sugar) => {
            // The edit sees the expanded data; what it leaves is put back over the short forms by the one rule.
            let header = doc.header();
            let expanded = sugar.try_expand(&raw)?;
            let mut scratch = Document::new(&stringify(
                &expanded,
                &StringifyOptions {
                    header: &header,
                    inline: None,
                },
            )?);
            edit(&mut scratch)?;
            apply_sugared(&mut doc, &scratch.to_js()?, Some(&sugar))?;
            doc.set_header(&scratch.header())?;
        }
    }
    let after = doc.text();
    if after == before {
        return Ok(false);
    }
    ws_write(fs, path, after.as_bytes())?;
    Ok(true)
}

/// Writes the whole file canonically from data; says whether it wrote.
///
/// # Errors
/// A path that is not .k7x, a value that does not print, or the write's.
pub fn ws_write_data(fs: &mut dyn Fs, path: &str, value: &Value, opts: &StringifyOptions) -> Res<bool> {
    k7x_only(path)?;
    write_if_changed(fs, path, &stringify(value, opts)?)
}

/// Writes k7x text as given once it reads; says whether it wrote.
///
/// # Errors
/// A syntax error of the text, or the write's.
pub fn ws_write_text(fs: &mut dyn Fs, path: &str, text: &str) -> Res<bool> {
    k7x_only(path)?;
    parse(text)?;
    write_if_changed(fs, path, text)
}
