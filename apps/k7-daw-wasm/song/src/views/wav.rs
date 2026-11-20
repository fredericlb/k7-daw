// views/wav.rs — how long a PCM or float WAV is, from the first 8 KiB of it and its size on disk.
// A reader would get wrong that a 0 `data` size is 0 seconds here, though `wavLayout` reads it as what is on disk.

/// As much of a header as any sane file spends before `data`.
pub const HEAD_BYTES: u64 = 8192;

const PCM: u16 = 1;
const FLOAT: u16 = 3;
const EXTENSIBLE: u16 = 0xfffe;

fn u16_at(b: &[u8], at: u64) -> Option<u16> {
    let at = usize::try_from(at).ok()?;
    Some(u16::from_le_bytes(b.get(at..at.checked_add(2)?)?.try_into().ok()?))
}

fn u32_at(b: &[u8], at: u64) -> Option<u32> {
    let at = usize::try_from(at).ok()?;
    Some(u32::from_le_bytes(b.get(at..at.checked_add(4)?)?.try_into().ok()?))
}

/// Whether a `fmt ` body describes uncompressed samples; the extensible tag names it by its sub-format.
fn is_plain(fmt: &[u8]) -> bool {
    match u16_at(fmt, 0) {
        Some(PCM | FLOAT) => true,
        Some(EXTENSIBLE) if fmt.len() >= 26 => matches!(u16_at(fmt, 24), Some(PCM | FLOAT)),
        _ => false,
    }
}

/// Whether four bytes spell `want` as Node's `"ascii"` decode reads them: the high bit of each is dropped.
fn is_id(bytes: Option<&[u8]>, want: &[u8; 4]) -> bool {
    bytes.is_some_and(|b| b.len() == 4 && b.iter().zip(want).all(|(x, w)| x & 0x7f == *w))
}

/// The duration in seconds of a PCM or float RIFF/WAVE file, or None for anything else.
pub fn wav_seconds(head: &[u8], file_size: u64) -> Option<f64> {
    if head.len() < 12 || !is_id(head.get(0..4), b"RIFF") || !is_id(head.get(8..12), b"WAVE") {
        return None;
    }
    let len = head.len() as u64;
    let mut at: u64 = 12;
    let mut format: Option<&[u8]> = None;
    while at + 8 <= len {
        let id = head.get(usize::try_from(at).ok()?..usize::try_from(at + 4).ok()?);
        let size = u64::from(u32_at(head, at + 4)?);
        let body = at + 8;
        if is_id(id, b"fmt ") {
            if size < 16 || body + size > len {
                return None;
            }
            format = head.get(usize::try_from(body).ok()?..usize::try_from(body + size).ok()?);
        } else if is_id(id, b"data") {
            let fmt = format?;
            if !is_plain(fmt) {
                return None;
            }
            let block_align = u64::from(u16_at(fmt, 12)?);
            let sample_rate = u64::from(u32_at(fmt, 4)?);
            if block_align == 0 || sample_rate == 0 {
                return None;
            }
            let on_disk = file_size.saturating_sub(body);
            // A 0 or oversized declared size is the recorder that never closed its file; here it is the header's word.
            return Some(size.min(on_disk) as f64 / (block_align * sample_rate) as f64);
        }
        at = body + size + (size & 1);
    }
    None
}
