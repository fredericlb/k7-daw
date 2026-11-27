# The editor's buffer, paths and intake

What the editor decides about an open file's text and a project's paths, in Rust, reached from the
page through `k7-wasm`'s ops (`buffer.*`, `paths.*`, `intake.*`, `kit.*`, `clipboard.*`, `draft.*` and
the buffer's writers under `text.*`). The page's facades are `@studio/k7-wasm/{buffer,paths,intake,kit,clipboard,draft,text}`.

| rule | crate and file |
|---|---|
| which kind of project file a path is, a path made relative or split, the module behind an instance | `song-edit/src/project_paths.rs` |
| the open buffer's reads (slice rack, `io`, channels), a clip's window, a slice symbol's window | `song-edit/src/buffer_text.rs` |
| the names a dropped file and a recorded take get, the clip declaration, the drop's plan | `song-edit/src/intake.rs` |
| the rack builder: pads, notes, what stops a create, the kit node | `song-edit/src/kit.rs` |
| what a rectangle of notes or events copies and where a paste lands | `song-edit/src/clipboard.rs` |
| the curve editor's draft, its fields, a drawn point | `song-edit/src/curve_draft.rs` |
| where the cursor sits in a half-typed file, one scalar field, the tokens and ▶ lines of a tree | `k7x/src/{cursor,scan,tokens}.rs` |

## Choices

| choice | why |
|---|---|
| a path rule judges the text of a path and the files the page already holds, never a disk | the page's tree is a list; the same rule serves a native host's list |
| the dot of `.k7x` is any character but a line end, as `sugar.rs` has it | one rule for every caller: a file called `x_k7x` is a document name here, in the compiler's path rules and in the page's patterns |
| a span answered by `buffer.*` is the tree's, in UTF-16 units | CodeMirror owns the edits; the page maps a span through those since the tree was parsed (`apps/k7-daw-frontend/packages/ui-editor/docs/k7xTree.md`) |
| a writer of the buffer answers the splices as the `text.*` writers do | an editor applies them to the live buffer, so the undo history keeps the gesture and the cursor stays |
| a refusal is a message key and its parameters, never a sentence | the words are the page's (paraglide); a native interface has its own tables |
| a dropped file's accents are folded by the page, and Rust reads the folded name | `String.normalize` is the browser's own; the rule that follows (what a path may hold) is Rust's |
| a number JSON cannot carry crosses as its word (`"NaN"`, `"Infinity"`) | a take's measured length can be either, and the line it becomes says so |
| the typed-name resolvers rank in TypeScript, scoring through one `fuzzyScores` call per pool | the tie between two equal scores is the browser's collator, the same one the file tree sorts by |
| a node's presets stay in the order the browser's collator gives (`src/presetOrder.ts`) | an order for people is a locale's (`apps/k7-daw-frontend/docs/listOrder.md`), not a table in the module |
| `norm` stays one line of TypeScript | it is a string replace called in the paths' own loops, and a module call would cost more than it decides |
