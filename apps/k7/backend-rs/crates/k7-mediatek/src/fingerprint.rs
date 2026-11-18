//! fingerprint.rs — the content fingerprint a `{file}.cache` sidecar is stamped with, as
//! `packages/audio-analysis/src/fingerprint.ts` takes it: FNV-1a/64 over the size, then the first and last 64 KB.
//! What a reader gets wrong: it reads content, not times, so a sidecar that travels with its file stays fresh.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const HEAD: u64 = 65536;
const TAIL: u64 = 65536;
const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const PRIME: u64 = 0x0000_0100_0000_01b3;

fn step(h: u64, byte: u8) -> u64 {
    (h ^ u64::from(byte)).wrapping_mul(PRIME)
}

/// `<size>-<16 hex digits>`; the error is the file's, as `statSync` or `openSync` would throw it.
pub fn file_fingerprint(source: &Path) -> std::io::Result<String> {
    let mut file = File::open(source)?;
    let size = file.metadata()?.len();
    let mut h = OFFSET;
    for b in size.to_le_bytes() {
        h = step(h, b);
    }
    let head_len = size.min(HEAD);
    let mut buf = vec![0u8; head_len as usize];
    if head_len > 0 {
        let n = read_up_to(&mut file, &mut buf)?;
        for &b in &buf[..n] {
            h = step(h, b);
        }
    }
    let tail_start = size.saturating_sub(TAIL);
    let tail_len = (size - tail_start) as usize;
    if tail_len > 0 {
        file.seek(SeekFrom::Start(tail_start))?;
        let mut tail = vec![0u8; tail_len];
        let n = read_up_to(&mut file, &mut tail)?;
        for &b in &tail[..n] {
            h = step(h, b);
        }
    }
    Ok(format!("{size}-{h:016x}"))
}

/// What one `readSync` of `buf.len()` bytes hands back: as many as are there.
fn read_up_to(file: &mut File, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match file.read(&mut buf[n..])? {
            0 => break,
            k => n += k,
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_the_size_and_both_ends() {
        let dir = std::env::temp_dir().join(format!("k7-fp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("empty.bin");
        std::fs::write(&f, b"").unwrap();
        let mut h = OFFSET;
        for b in 0u64.to_le_bytes() {
            h = step(h, b);
        }
        assert_eq!(file_fingerprint(&f).unwrap(), format!("0-{h:016x}"));
        let big = dir.join("big.bin");
        let bytes: Vec<u8> = (0..200_000u32).map(|i| (i * 7 % 251) as u8).collect();
        std::fs::write(&big, &bytes).unwrap();
        let mut h = OFFSET;
        for b in (bytes.len() as u64).to_le_bytes() {
            h = step(h, b);
        }
        for &b in bytes[..65536].iter().chain(&bytes[bytes.len() - 65536..]) {
            h = step(h, b);
        }
        assert_eq!(file_fingerprint(&big).unwrap(), format!("200000-{h:016x}"));
        assert!(file_fingerprint(&dir.join("absent")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
