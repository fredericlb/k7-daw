//! files.rs — what a job answers for each file it left in its `.job-out` folder: its name, its length, a wav's seconds.
//! What a reader gets wrong: the name is one segment below the job's folder, never a path of the server's.

use std::io::Read;

use serde_json::{Value, json};
use song::views::wav::{HEAD_BYTES, wav_seconds};

use super::Error;
use super::gate::out_file;

/// A wav's seconds from its header; `None` for any other file or one that cannot be read.
pub fn seconds_of(path: &str, len: u64) -> Option<f64> {
    let mut head = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(HEAD_BYTES)
        .read_to_end(&mut head)
        .ok()?;
    wav_seconds(&head, len)
}

/// `{out, len, seconds}` of the file `out` in `dir`, a regular file reached through no link.
pub fn job_file(dir: &str, out: &str) -> Result<Value, Error> {
    let path = out_file(dir, out)?;
    let meta = std::fs::symlink_metadata(&path)
        .map_err(|_| Error::new(500, format!("the job wrote no '{out}'")))?;
    if !meta.is_file() {
        return Err(Error::new(500, format!("the job's '{out}' is no file")));
    }
    let seconds = seconds_of(&path, meta.len());
    Ok(json!({ "out": out, "len": meta.len(), "seconds": seconds }))
}

/// The files of a job named by `outs`, in that order, each judged by `job_file`.
pub fn job_files(dir: &str, outs: &[String]) -> Result<Vec<Value>, Error> {
    outs.iter().map(|o| job_file(dir, o)).collect()
}
