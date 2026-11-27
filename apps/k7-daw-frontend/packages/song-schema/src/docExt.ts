// docExt.ts — the extension a workspace document has, `.k7x`, for a regex that tests a name.
// A `.yaml` name is no workspace document: the media library's store and the config files beside it are read by name.
// The plain tests (`isDocFile`, `stripDocExt`) are the Rust crate's, in `@studio/k7-wasm/schema`.

/** The extension as pattern source, for a regex that needs more around it. */
export const DOC_EXT_SRC = ".k7x";

/** A regex matching `prefix` (pattern source) followed by the document extension at the end. */
export const docPattern = (prefix: string, flags = "i"): RegExp =>
  new RegExp(`${prefix}${DOC_EXT_SRC}$`, flags);
