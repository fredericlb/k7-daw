//! scratch.rs — a fresh folder per test under the temp directory, removed when the test ends.

use std::path::{Path, PathBuf};

pub struct Scratch(PathBuf);

impl Scratch {
    pub fn new(tag: &str) -> Scratch {
        let mut b = [0u8; 6];
        getrandom::fill(&mut b).expect("random");
        let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
        let dir = std::env::temp_dir().join(format!("k7-sup-{tag}-{hex}"));
        std::fs::create_dir_all(&dir).expect("a scratch folder");
        Scratch(dir)
    }
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
