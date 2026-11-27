// wasmFile.d.ts — the module's file as the bytes the render page's bundle carries (esbuild's binary loader).
declare module "*.wasm" {
  const bytes: Uint8Array;
  export default bytes;
}
