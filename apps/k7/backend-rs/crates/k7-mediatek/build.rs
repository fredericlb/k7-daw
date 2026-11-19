//! build.rs — copies the waveform module (`packages/audio-analysis/waveform.wasm`) to `OUT_DIR/waveform.wasm` for `src/waveform.rs` to embed.
//! What a reader gets wrong: the committed binary is the source of the embed; `npm run rebuild:generated` rebuilds it from `packages/audio-analysis/rust`.

use std::path::Path;

fn main() {
    let wasm = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../../packages/audio-analysis/waveform.wasm");
    println!("cargo:rerun-if-changed={}", wasm.display());
    let out =
        Path::new(&std::env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("waveform.wasm");
    std::fs::copy(&wasm, out).unwrap_or_else(|e| panic!("{}: {e}", wasm.display()));
}
