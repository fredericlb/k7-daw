//! log_slice.rs — a slice of the merged log for the `log` job: with an offset, the new whole lines since it;
//! without one, or past the end (a rotation), the last `tail`. A read is capped, so a reader far behind catches
//! up over several polls and a line longer than the cap is skipped rather than held on for ever.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use serde_json::{Value, json};

use crate::paths::Paths;

/// The most of the log one `log` job reads, whatever the file has grown to.
pub const LOG_READ_CAP: u64 = 1024 * 1024;

fn split_lines(buf: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = buf;
    while !rest.is_empty() {
        let end = rest.iter().position(|&b| b == b'\n').unwrap_or(rest.len());
        let line = rest[..end].strip_suffix(b"\r").unwrap_or(&rest[..end]);
        out.push(String::from_utf8_lossy(line).into_owned());
        rest = &rest[(end + 1).min(rest.len())..];
    }
    out
}

/// `count` bytes of `p` from `from`, fewer where the file ends first.
fn read_range(p: &Path, from: u64, count: u64) -> Vec<u8> {
    let Ok(mut f) = std::fs::File::open(p) else {
        return Vec::new();
    };
    if f.seek(SeekFrom::Start(from)).is_err() {
        return Vec::new();
    }
    let mut buf = Vec::new();
    let _ = f.take(count).read_to_end(&mut buf);
    buf
}

/// The merged log, else the engine-only log a run without the supervisor writes.
pub fn read_log(paths: &Paths, offset: Option<u64>, tail: Option<i64>) -> Value {
    let Some(file) = [paths.merged_log(), paths.engine_log()]
        .into_iter()
        .find(|p| p.is_file())
    else {
        return json!({"offset": 0, "lines": []});
    };
    let Ok(size) = std::fs::metadata(&file).map(|m| m.len()) else {
        return json!({"offset": 0, "lines": []});
    };
    if let Some(offset) = offset.filter(|o| *o <= size) {
        let count = (size - offset).min(LOG_READ_CAP);
        let buf = read_range(&file, offset, count);
        let Some(last_nl) = buf.iter().rposition(|&b| b == b'\n') else {
            let to = if count == LOG_READ_CAP {
                offset + count
            } else {
                offset
            };
            return json!({"offset": to, "lines": []});
        };
        let whole = &buf[..=last_nl];
        return json!({"offset": offset + last_nl as u64 + 1, "lines": split_lines(whole)});
    }
    // Seeded from the tail, and again when a rotation left the reader's offset past the end.
    let count = size.min(LOG_READ_CAP);
    let mut buf = read_range(&file, size - count, count);
    // Started mid-file: the first line is a fragment.
    if count < size
        && let Some(nl) = buf.iter().position(|&b| b == b'\n')
    {
        buf.drain(..=nl);
    }
    let mut all = split_lines(&buf);
    let n = tail.unwrap_or(300).max(1) as usize;
    if all.len() > n {
        all.drain(..all.len() - n);
    }
    json!({"offset": size, "lines": all})
}
