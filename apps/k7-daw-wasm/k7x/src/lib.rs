// lib.rs — k7x in Rust: the reader (parse.rs), the document editor and printer (edit/), the editor's outline (outline.rs).
// Spans are BYTE offsets into the UTF-8 text; whatever crosses to the page counts UTF-16 units (docs/rust.md).

pub mod ast;
pub mod edit;
pub mod json;
pub mod outline;
mod parse;

pub use ast::{Entry, ListForm, MapForm, MapNode, Node, PathSeg, Scalar};
pub use bumpalo::Bump;
pub use parse::{MAX_DEPTH, SyntaxError, parse_tree, parse_tree_lossless, parse_value_end};
