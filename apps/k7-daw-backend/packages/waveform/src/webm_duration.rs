// webm_duration.rs — writes the Duration MediaRecorder leaves out of a WebM, by patching the file's first bytes alone.
// A reader would get wrong that only a head is read: the answer replaces its first `consumed` bytes, the rest of the file is untouched.

const ID_EBML: u64 = 0x1a45_dfa3;
const ID_SEGMENT: u64 = 0x1853_8067;
const ID_SEEKHEAD: u64 = 0x114d_9b74;
const ID_SEEK: u64 = 0x4dbb;
const ID_SEEKPOSITION: u64 = 0x53ac;
const ID_INFO: u64 = 0x1549_a966;
const ID_TIMECODESCALE: u64 = 0x2a_d7b1;
const ID_DURATION: u64 = 0x4489;

/// The bytes that replace the file's first `consumed`; `consumed` 0 means the file already had a duration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patched {
    pub bytes: Vec<u8>,
    pub consumed: usize,
}

struct Vint {
    value: u64,
    width: usize,
    unknown: bool,
}

struct Element {
    id: u64,
    size: Vint,
    start: usize,
    data_start: usize,
}

/// An EBML variable-length integer: an id keeps its marker bit, a size drops it.
fn read_vint(buf: &[u8], at: usize, keep_marker: bool) -> Option<Vint> {
    let first = *buf.get(at)?;
    let width = first.leading_zeros() as usize + 1;
    if width > 8 || at + width > buf.len() {
        return None;
    }
    let mask = (0xffu16 >> width) as u8;
    let bits = if keep_marker { first } else { first & mask };
    let mut value = u64::from(bits);
    let mut all_ones = bits == mask;
    for i in 1..width {
        value = (value << 8) | u64::from(buf[at + i]);
        if buf[at + i] != 0xff {
            all_ones = false;
        }
    }
    Some(Vint {
        value,
        width,
        unknown: !keep_marker && all_ones,
    })
}

fn read_element(buf: &[u8], at: usize) -> Option<Element> {
    let id = read_vint(buf, at, true)?;
    let size = read_vint(buf, at + id.width, false)?;
    let data_start = at + id.width + size.width;
    Some(Element {
        id: id.value,
        size,
        start: at,
        data_start,
    })
}

/// The end of an element's data; an unknown or absurd size saturates, so a loop over it ends.
fn data_end(el: &Element) -> usize {
    el.data_start
        .saturating_add(usize::try_from(el.size.value).unwrap_or(usize::MAX))
}

fn read_uint(buf: &[u8], at: usize, len: usize) -> Option<u64> {
    let bytes = buf.get(at..at.checked_add(len)?)?;
    Some(
        bytes
            .iter()
            .fold(0u64, |v, b| v.wrapping_mul(256).wrapping_add(u64::from(*b))),
    )
}

fn write_uint(buf: &mut [u8], at: usize, len: usize, mut value: u64) -> Option<()> {
    if len < 8 && value >= 256u64.pow(len as u32) {
        return None;
    }
    let slot = buf.get_mut(at..at.checked_add(len)?)?;
    for b in slot.iter_mut().rev() {
        *b = (value % 256) as u8;
        value /= 256;
    }
    Some(())
}

/// A size vint at least `width` bytes wide, wider only when the value needs it.
fn encode_size(value: u64, width: usize) -> Option<Vec<u8>> {
    let mut w = width;
    while w < 8 && value >= (1u64 << (7 * w)) - 1 {
        w += 1;
    }
    let mut out = vec![0u8; w];
    write_uint(&mut out, 0, w, value)?;
    out[0] |= 0x80 >> (w - 1);
    Some(out)
}

/// The head with a Duration of `duration_ms`, or `None` when its layout is not one this patcher knows.
#[must_use]
pub fn patch_head(head: &[u8], duration_ms: f64) -> Option<Patched> {
    let ebml = read_element(head, 0)?;
    if ebml.id != ID_EBML || ebml.size.unknown {
        return None;
    }
    let seg = read_element(
        head,
        ebml.data_start + usize::try_from(ebml.size.value).ok()?,
    )?;
    if seg.id != ID_SEGMENT {
        return None;
    }
    let seg_data = seg.data_start;

    // The Segment's first children up to Info, noting a SeekHead whose positions follow it.
    let mut seek_head: Option<(usize, usize)> = None;
    let mut p = seg_data;
    let mut info: Option<Element> = None;
    while p < head.len() {
        let el = read_element(head, p)?;
        if el.id == ID_INFO {
            info = Some(el);
            break;
        }
        if el.size.unknown {
            return None;
        }
        if el.id == ID_SEEKHEAD {
            seek_head = Some((el.data_start, data_end(&el)));
        }
        p = data_end(&el);
    }
    let info = info?;
    if info.size.unknown {
        return None;
    }
    let info_end = data_end(&info);
    if info_end > head.len() {
        return None;
    }

    let mut scale: u64 = 1_000_000;
    let mut duration_at: Option<usize> = None;
    let mut duration_len = 0usize;
    let mut q = info.data_start;
    while q < info_end {
        let el = read_element(head, q)?;
        if el.id == ID_TIMECODESCALE {
            scale = read_uint(head, el.data_start, usize::try_from(el.size.value).ok()?)?;
        }
        if el.id == ID_DURATION {
            duration_at = Some(el.data_start);
            duration_len = usize::try_from(el.size.value).ok()?;
        }
        q = data_end(&el);
    }
    let ticks = duration_ms * 1_000_000.0 / scale as f64;

    let mut out = head[..info_end].to_vec();
    if let Some(at) = duration_at {
        let current = match duration_len {
            8 => f64::from_be_bytes(out.get(at..at + 8)?.try_into().ok()?),
            4 => f64::from(f32::from_be_bytes(out.get(at..at + 4)?.try_into().ok()?)),
            _ => f64::NAN,
        };
        if current > 0.0 {
            return Some(Patched {
                bytes: Vec::new(),
                consumed: 0,
            });
        }
        match duration_len {
            8 => out
                .get_mut(at..at + 8)?
                .copy_from_slice(&ticks.to_be_bytes()),
            4 => out
                .get_mut(at..at + 4)?
                .copy_from_slice(&(ticks as f32).to_be_bytes()),
            _ => return None,
        }
        return Some(Patched {
            bytes: out,
            consumed: info_end,
        });
    }

    // No Duration: append one to Info, which grows Info, the Segment if sized, and every SeekPosition past Info.
    let mut duration_el = vec![0x44, 0x89, 0x88];
    duration_el.extend_from_slice(&ticks.to_be_bytes());
    let info_size = encode_size(info.size.value + 11, info.size.width)?;
    let grown = 11 + info_size.len() - info.size.width;

    let seg_size_at = seg.data_start - seg.size.width;
    let mut seg_size = out.get(seg_size_at..seg.data_start)?.to_vec();
    if !seg.size.unknown {
        seg_size = encode_size(seg.size.value + grown as u64, seg.size.width)?;
        if seg_size.len() != seg.size.width {
            return None;
        }
    }
    if let Some(range) = seek_head {
        shift_seek_positions(&mut out, range, info.start - seg_data, grown as u64)?;
    }

    let info_size_at = info.data_start - info.size.width;
    let mut bytes = Vec::with_capacity(out.len() + grown);
    bytes.extend_from_slice(&out[..seg_size_at]);
    bytes.extend_from_slice(&seg_size);
    bytes.extend_from_slice(&out[seg.data_start..info_size_at]);
    bytes.extend_from_slice(&info_size);
    bytes.extend_from_slice(&out[info.data_start..info_end]);
    bytes.extend_from_slice(&duration_el);
    Some(Patched {
        bytes,
        consumed: info_end,
    })
}

/// Moves, in place, every SeekPosition that points past Info by `grown` bytes.
fn shift_seek_positions(
    buf: &mut [u8],
    (start, end): (usize, usize),
    info_offset: usize,
    grown: u64,
) -> Option<()> {
    let mut p = start;
    while p < end {
        let seek = read_element(buf, p)?;
        let seek_end = data_end(&seek);
        if seek.id == ID_SEEK {
            let mut q = seek.data_start;
            while q < seek_end {
                let el = read_element(buf, q)?;
                if el.id == ID_SEEKPOSITION {
                    let len = usize::try_from(el.size.value).ok()?;
                    let pos = read_uint(buf, el.data_start, len)?;
                    if pos > info_offset as u64 {
                        write_uint(buf, el.data_start, len, pos + grown)?;
                    }
                }
                q = data_end(&el);
            }
        }
        p = seek_end;
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn el(id: &[u8], data: &[u8], unknown: bool) -> Vec<u8> {
        let mut v = id.to_vec();
        if unknown {
            v.extend_from_slice(&[0x01, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
        } else {
            v.push(0x80 | data.len() as u8);
        }
        v.extend_from_slice(data);
        v
    }
    fn cat(parts: &[&[u8]]) -> Vec<u8> {
        parts.concat()
    }

    fn ebml() -> Vec<u8> {
        el(
            &[0x1a, 0x45, 0xdf, 0xa3],
            &el(&[0x42, 0x82], &[0x77, 0x65], false),
            false,
        )
    }
    fn scale() -> Vec<u8> {
        el(&[0x2a, 0xd7, 0xb1], &[0x0f, 0x42, 0x40], false)
    }
    fn info() -> Vec<u8> {
        el(
            &[0x15, 0x49, 0xa9, 0x66],
            &cat(&[&scale(), &el(&[0x4d, 0x80], &[], false)]),
            false,
        )
    }
    fn tracks() -> Vec<u8> {
        el(&[0x16, 0x54, 0xae, 0x6b], &[0xaa, 0xbb], false)
    }
    fn cluster() -> Vec<u8> {
        vec![
            0x1f, 0x43, 0xb6, 0x75, 0x01, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 1, 2, 3, 4,
        ]
    }
    fn duration_of(b: &[u8]) -> f64 {
        for i in 0..b.len().saturating_sub(10) {
            if b[i..i + 3] == [0x44, 0x89, 0x88] {
                return f64::from_be_bytes(b[i + 3..i + 11].try_into().unwrap());
            }
        }
        f64::NAN
    }

    #[test]
    fn appends_a_duration_to_info_in_a_mediarecorder_shaped_file() {
        let seg = el(
            &[0x18, 0x53, 0x80, 0x67],
            &cat(&[&info(), &tracks(), &cluster()]),
            true,
        );
        let file = cat(&[&ebml(), &seg]);
        let out = patch_head(&file, 12_345.0).unwrap();
        assert_eq!(duration_of(&out.bytes), 12_345.0);
        assert_eq!(
            file[out.consumed..out.consumed + 4],
            [0x16, 0x54, 0xae, 0x6b]
        );
        // Info grew by the 11 bytes of the Duration element.
        let info_size_at = ebml().len() + 12 + 4;
        assert_eq!(
            out.bytes[info_size_at],
            0x80 | (info().len() - 5 + 11) as u8
        );
    }

    #[test]
    fn grows_a_sized_segment_and_shifts_the_seek_positions_past_info() {
        let seek_pos = (info().len() + 30) as u8;
        let seek = el(
            &[0x4d, 0xbb],
            &cat(&[
                &el(&[0x53, 0xab], &[0x16, 0x54, 0xae, 0x6b], false),
                &el(&[0x53, 0xac], &[0, seek_pos], false),
            ]),
            false,
        );
        let seek_head = el(&[0x11, 0x4d, 0x9b, 0x74], &seek, false);
        let body = cat(&[&seek_head, &info(), &tracks()]);
        let seg = cat(&[&[0x18, 0x53, 0x80, 0x67, 0x40, body.len() as u8], &body]);
        let out = patch_head(&cat(&[&ebml(), &seg]), 1000.0).unwrap();
        let b = &out.bytes;
        assert_eq!(b[ebml().len() + 5], body.len() as u8 + 11);
        let pos_at = b.iter().position(|x| *x == 0xac).unwrap() + 2;
        assert_eq!(b[pos_at + 1], seek_pos + 11);
    }

    #[test]
    fn leaves_a_file_that_already_has_a_duration_alone() {
        let dur = el(&[0x44, 0x89], &[0x40, 0x8f, 0x40, 0, 0, 0, 0, 0], false);
        let info = el(&[0x15, 0x49, 0xa9, 0x66], &cat(&[&scale(), &dur]), false);
        let seg = el(&[0x18, 0x53, 0x80, 0x67], &cat(&[&info, &cluster()]), true);
        assert_eq!(patch_head(&cat(&[&ebml(), &seg]), 5.0).unwrap().consumed, 0);
    }

    #[test]
    fn refuses_what_is_not_a_webm() {
        assert_eq!(patch_head(&[1, 2, 3, 4], 5.0), None);
    }

    #[test]
    fn keeps_the_tail_as_the_original_bytes() {
        let seg = el(
            &[0x18, 0x53, 0x80, 0x67],
            &cat(&[&info(), &cluster()]),
            true,
        );
        let file = cat(&[&ebml(), &seg]);
        let out = patch_head(&file, 2000.0).unwrap();
        assert_eq!(out.bytes.len() + file.len() - out.consumed, file.len() + 11);
        assert_eq!(file[file.len() - 4..], [1, 2, 3, 4]);
    }
}
