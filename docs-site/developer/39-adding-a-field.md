# Adding a field or a file kind

This page is the checklist for extending the song format end to end. Each step names the file to
touch and what forgetting it costs.

## Adding a field to an existing file kind

Take, as an example, a new optional number on a lane's event.

1. **Schema.** Add it to the zod schema in `apps/k7-daw-frontend/packages/song-schema`: here `PlacementSchema` in
   `apps/k7-daw-frontend/packages/song-schema/src/yaml/placementSchema.ts`. Module, node and lane fields go in
   `MODULE_FIELDS` in `apps/k7-daw-frontend/packages/song-schema/src/yaml/moduleSchema.ts` (shared by modules and their
   alternatives); project fields in `ProjectSettings` of `apps/k7-daw-wasm/song/src/files/project.rs`; clips,
   sequences and curves in `apps/k7-daw-wasm/song/src/files/entity.rs` and `apps/k7-daw-wasm/song/src/curves/file.rs`.
   The Rust readers are strict: until this step, the field is refused with `unknown key`.
   Give it bounds and a message that names the expected spelling; the message is what the user reads.
2. **Completion comes for free.** `apps/k7-daw-wasm/song/src/views/fields.rs` reads the keys the decoder's derive names, so
   the editor offers the field. Only a widget field must be added by hand to `WIDGET_FIELDS` there.
3. **Timecodes.** If the value is a timecode, add its key to `TIMECODE_KEYS` in
   `apps/k7-daw-frontend/packages/song-schema/src/yaml/placementSchema.ts`, so every writer quotes it.
4. **Keyed lists.** If the field is a list whose items should sync as items, give it key fields in
   `LIST_KEYS` (`apps/k7-daw-backend/packages/k7-sync/src/k7x/locate.rs`); otherwise a concurrent edit to one item replaces
   the whole list.
5. **Inheritance.** For a module field, decide how it merges: by default the child's value replaces
   the parent's. A map that should merge per key needs a case in `mergeAuthored`
   (`apps/k7-daw-wasm/song/src/resolve/extends.rs`); a field a node must never inherit goes in `OWN_ONLY`.
6. **Compile.** Read the field in the song crate's compile (`apps/k7-daw-wasm/song/src/`) and, if the engine needs it, add it to the wire type in
   `apps/k7-daw-frontend/packages/engine-protocol/project.ts`. The engine mirrors those types in C++ and diffs documents, so
   a new wire field must always be sent (with its default) and must be handled on the engine side
   (see the engine section).
7. **Cross-file checks.** If the field names another file or node, check it with the other reference
   checks of the song crate (`apps/k7-daw-wasm/song/src/lanes/references.rs`) and report with a message naming the lane and the target. If it names a
   path, pass it through `confineTo`/`projectRoots` (`apps/k7-daw-wasm/song-edit/src/confine.rs`): every file
   service runs as the same OS user, so a `../` is another account's file.
8. **Writer.** If k7 changes the field, write a text transform in the kind's `*File.ts`
   (`apps/k7-daw-frontend/packages/song-schema/src/yaml/`) or a writer in `apps/k7-daw-frontend/packages/song-edit/src/`, delete the key when it
   returns to its default, and expose it as a verb in `apps/k7-daw-wasm/verbs`. Never re-serialise a whole
   document: comments and other values must stay byte for byte, or the history falls back to whole-text
   operations.
9. **Tests.** A schema case beside the schema; a writer test that compares text; a compile change is a case in the song crate's tests
   (`apps/k7-daw-wasm/song/tests/all/`). If a demo uses the field, `apps/k7-daw-wasm/k7x/tests/corpus.rs` checks that edits to it replay.
10. **Docs.** Add the field to the user guide's song file reference with its type, default and an
    example.

## Adding a file kind

1. **Folder and schema.** Name the folder (a constant beside the schema, as `CHAINS_DIR` or
   `ASSEMBLIES_DIR`) and write its schema in `apps/k7-daw-frontend/packages/song-schema/src/yaml/`. A file is named by its id;
   never add an `id:` field inside it.
2. **Validation.** Route it in `family_of` (`apps/k7-daw-wasm/song/src/files/route.rs`) and give it an arm in
   `judge` (`apps/k7-daw-wasm/song/src/files/validate.rs`). Order matters: the router tests folders first,
   then basenames. Without this the editor checks syntax only.
3. **Completion.** Add a kind to `file_kind_for` and `fields_at` in
   `apps/k7-daw-wasm/song/src/views/fields.rs`.
4. **k7x short form**, if one is wanted: a `SugarPart` in `apps/k7-daw-wasm/song-edit/src/sugar.rs`,
   returned by `sugar_for(path)`. It must expand to plain data the schema already accepts.
5. **Loader.** Read it in `apps/k7-daw-wasm/song-edit/src/loader.rs` — a flat folder of entities can be added to
   `ENTITY_SCHEMAS` and read through `readYamlEntities`/`exportSub`; an optional folder with its own
   reader follows `apps/k7-daw-wasm/song/src/views/surfaces.rs` or `apps/k7-daw-wasm/song/src/assemblies/mod.rs`. Always go through
   `apps/k7-daw-wasm/song-edit/src/workspace_doc.rs` (`wsData`, `wsEdit`, `wsWriteData`), so a file is read
   and written as k7x.
6. **Diagnostics.** Decide whether a broken file stops the compile (a thrown error) or is reported as a
   `Diagnostic` and skipped. A drawing aid (surfaces, assemblies) should not stop the music.
7. **Layout.** If the file names nodes by name, it cannot live in a chain project, which keeps
   no name for a non-lane node.
8. **File service.** `.k7x` files already move the revision token, are journalled by
   the change set and synced by live capture. Add a route (`apps/k7-daw-backend/packages/k7-front/src/routes/files/service/routes.rs`)
   only if the browser needs a derived view of the new kind.
9. **Scaffold.** If new projects should have one, add it to the archetype's file map in
   `apps/k7-daw-wasm/song-edit/src/scaffold.rs`, `scaffold_live.rs` or `apps/k7-daw-wasm/song-edit/src/scaffold_set.rs`, with comments: the
   scaffolded file is the user's first example of the format.

## Running the checks

```bash
npm test --workspace @studio/song-schema
npm test --workspace @studio/song
npm test --workspace @studio/song-edit
npm run lint
```
