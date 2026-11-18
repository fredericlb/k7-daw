// build.rs — reads the compiler's version from the checked-in module and hands it to the crate as a constant.
// What a reader gets wrong: it computes no hash: the one value is `SOURCES`, which the module proves by its own test.

use std::path::PathBuf;

const MODULE: &str = "../../../../../packages/k7-wasm/src/k7Wasm.generated.ts";

fn main() {
    println!("cargo:rerun-if-changed={MODULE}");
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let path = root.join(MODULE);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "k7-file-service reads the compiler's version from {}: {e}",
            path.display()
        )
    });
    // Only the first ten lines are asked, so a long generated body is never scanned.
    let sources = text
        .lines()
        .take(10)
        .find_map(|l| {
            let rest = l.trim().strip_prefix("export const SOURCES")?;
            let quoted = rest.split('"').nth(1)?;
            Some(quoted.to_owned())
        })
        .unwrap_or_else(|| {
            panic!(
                "k7-file-service found no `export const SOURCES = \"...\"` in the first ten lines of {}",
                path.display()
            )
        });
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default()).join("sources.rs");
    std::fs::write(&out, format!("/// The compiler's version, the `SOURCES` of the checked-in module.\npub const SONG_SOURCES: &str = {sources:?};\n"))
        .unwrap_or_else(|e| panic!("k7-file-service writes {}: {e}", out.display()));
}
