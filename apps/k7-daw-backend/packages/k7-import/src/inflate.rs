// inflate.rs — unzip and gunzip with a ceiling on what they inflate to, since an import is a file a stranger may send.
// What a reader gets wrong: the sizes a zip DECLARES are not trusted, the bytes read are what is counted.

use std::io::{Cursor, Read};

use flate2::read::GzDecoder;

use crate::model::Bytes;

/// The most an imported archive may inflate to, every entry together.
pub const MAX_INFLATED_BYTES: usize = 512 * 1024 * 1024;

/// The most entries an archive may name: a bomb of empty files costs a table each.
pub const MAX_ENTRIES: usize = 20_000;

fn mib(limit: usize) -> usize {
    limit / (1024 * 1024)
}

/// Every file of a zip by its entry name, refusing an archive that inflates past `limit` bytes in all.
pub fn bounded_unzip(bytes: &[u8], limit: usize) -> Result<Vec<(String, Bytes)>, String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| format!("this zip does not read: {e}"))?;
    if archive.len() > MAX_ENTRIES {
        return Err(format!(
            "this archive names more than {MAX_ENTRIES} files — refused"
        ));
    }
    let past = || format!("this archive inflates past {} MiB — refused", mib(limit));
    let mut left = limit;
    let mut out = Vec::with_capacity(archive.len());
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("this zip does not read: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        if usize::try_from(entry.size()).map_or(true, |n| n > left) {
            return Err(past());
        }
        let mut data = Vec::with_capacity(entry.size() as usize);
        (&mut entry)
            .take(left as u64 + 1)
            .read_to_end(&mut data)
            .map_err(|e| format!("this zip does not read: {e}"))?;
        if data.len() > left {
            return Err(past());
        }
        left -= data.len();
        out.push((entry.name().to_owned(), Bytes::new(data)));
    }
    Ok(out)
}

/// A gzip stream inflated, refusing one that holds more than `limit` bytes.
pub fn bounded_gunzip(bytes: &[u8], limit: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    GzDecoder::new(bytes)
        .take(limit as u64 + 1)
        .read_to_end(&mut out)
        .map_err(|e| e.to_string())?;
    if out.len() > limit {
        return Err(format!(
            "this file inflates past {} MiB — refused",
            mib(limit)
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    fn gz(data: &[u8]) -> Vec<u8> {
        let mut e = GzEncoder::new(Vec::new(), Compression::default());
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }

    #[test]
    fn a_gzip_past_the_ceiling_is_refused_and_one_within_it_inflates() {
        let packed = gz(&vec![7u8; 4096]);
        assert_eq!(bounded_gunzip(&packed, 4096).unwrap().len(), 4096);
        assert!(bounded_gunzip(&packed, 4095).unwrap_err().contains("inflates past"));
    }

    #[test]
    fn a_zip_is_bounded_by_what_its_entries_inflate_to_in_all() {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            let opts = zip::write::SimpleFileOptions::default();
            for name in ["a.bin", "b.bin"] {
                z.start_file(name, opts).unwrap();
                z.write_all(&[1u8; 600]).unwrap();
            }
            z.finish().unwrap();
        }
        let bytes = buf.into_inner();
        assert_eq!(bounded_unzip(&bytes, 1200).unwrap().len(), 2);
        assert!(bounded_unzip(&bytes, 1199).unwrap_err().contains("inflates past"));
    }
}
