//! build.rs — the waveform module TypeScript runs (`packages/audio-analysis/src/waveformWasm.generated.ts`, base64),
//! decoded into `OUT_DIR/waveform.wasm` for `src/waveform.rs` to embed. The committed file is the one source of both.

use std::path::Path;

use base64::Engine;

fn main() {
    let generated = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../../packages/audio-analysis/src/waveformWasm.generated.ts");
    println!("cargo:rerun-if-changed={}", generated.display());
    let text = std::fs::read_to_string(&generated)
        .unwrap_or_else(|e| panic!("{}: {e}", generated.display()));
    let start = text.find("WASM =").expect("the generated file names WASM") + "WASM =".len();
    let quoted = &text[start..];
    let open = quoted.find('"').expect("WASM is a string") + 1;
    let close = open + quoted[open..].find('"').expect("WASM's string ends");
    let wasm = base64::engine::general_purpose::STANDARD
        .decode(&quoted[open..close])
        .expect("WASM is base64");
    let out =
        Path::new(&std::env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("waveform.wasm");
    std::fs::write(out, wasm).expect("OUT_DIR is writable");
}
