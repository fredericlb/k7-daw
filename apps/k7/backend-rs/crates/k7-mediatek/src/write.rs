//! write.rs — what the front's writes need of the data volume: a random name, a part file beside its destination, and a
//! move that crosses devices. What a reader gets wrong: `part_of` names a file BESIDE the destination, so a rename lands.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// `randomUUID`: a version 4 UUID, lowercase.
pub fn uuid() -> String {
    let mut b = [0u8; 16];
    let _ = getrandom::fill(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    )
}

/// `<path>.<uuid>.part`: beside it, a name no other writer shares.
pub fn part_of(abs: &Path) -> PathBuf {
    let mut name = abs.as_os_str().to_owned();
    name.push(format!(".{}.part", uuid()));
    PathBuf::from(name)
}

/// `moveFile`: a rename, or a copy then a removal across devices.
pub fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    match std::fs::rename(from, to) {
        Err(e) if e.kind() == ErrorKind::CrossesDevices => {
            std::fs::copy(from, to)?;
            let _ = std::fs::remove_file(from);
            Ok(())
        }
        other => other,
    }
}
