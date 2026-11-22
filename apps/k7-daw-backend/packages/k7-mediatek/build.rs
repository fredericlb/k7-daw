//! build.rs — copies the waveform module (`apps/k7-daw-frontend/packages/audio-analysis/waveform.wasm`) to `OUT_DIR/waveform.wasm` for `src/waveform.rs` to embed.
//! What a reader gets wrong: the committed binary is the source of the embed; `npm run rebuild:generated` rebuilds it from `apps/k7-daw-backend/packages/waveform`.

use std::path::Path;

fn main() {
    // Read when cargo runs the script, not compiled in: a cached build script carries the checkout that compiled it.
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let wasm =
        Path::new(&dir).join("../../../k7-daw-frontend/packages/audio-analysis/waveform.wasm");
    println!("cargo:rerun-if-changed={}", wasm.display());
    let out =
        Path::new(&std::env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("waveform.wasm");
    std::fs::copy(&wasm, out).unwrap_or_else(|e| panic!("{}: {e}", wasm.display()));
}
