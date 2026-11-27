# A workspace file is a .k7x file

Every file of a workspace is k7x (`apps/k7-daw-wasm/k7x/docs/language.md`): no `.yaml` file of a workspace is
read, listed or written. The plan this belongs to is [k7x-plan.md](../../../../docs/k7x-plan.md).

## Two modules, two jobs

- **`src/yaml/workspaceFile.ts` is plain I/O.** `wsRead` and `wsWrite` take and give a file's text as
  it is on disk, atomically for a write, and convert nothing. They stay for the code that holds a
  file's text rather than its data — the file service's `/file` route, a copy, a rename.
- **`src/yaml/workspaceDoc.ts` is the k7x door.** It takes a `.k7x` path named outright and refuses
  any other. A file is read as its text (`wsText`) or its data (`wsData`), edited as a k7x document by
  key path (`wsEdit`), or written whole, canonically from data (`wsWriteData`) or as text that must
  read (`wsWriteText`). Every write is atomic and skipped when the text would not change.

## The rules

- **An edit of a file with short forms sees their expanded data.** The edit runs on a scratch
  document of that data, and the result goes back over the real one by `applySugared`
  (`apps/k7-daw-wasm/k7x/src/edit/sugared.rs`, reached as `text.sugar_apply`, `apps/k7-daw-wasm/song-schema/docs/sugar.md`), so a writer never has to know
  that `notes("c4 d4")` is a string. A file that holds no short form is edited as written, so a
  key renamed or an item inserted keeps its line and its comments: a data diff would delete and
  re-add it. A writer that keeps a short form itself — graphFile's, which rename inside `runs(...)`
  — passes `raw` and edits the document as written too.
- **An absent file is an error unless the caller says `create`**: then it is an empty document, and
  is only written once the edit put something in it.
- **A finding carries the line of the k7x text.** Each reader decodes the file and places its first
  finding on the key it is about (`apps/k7-daw-wasm/song/src/files/validate.rs`); a
  syntax error is one finding with its own line and column.
- **A library's pins and manifest are workspace files too**: `_library-pins.k7x` and `library.k7x`.
- **What is not a workspace file is read by name.** A
  machine's `preferences.k7x` under the studio home, the repository's `commands.yaml` and `env.yaml`
  are read and written by name with plain file calls, never through either module. The preferences
  are k7x and the others stay YAML (`preferencesFile.md`).

## What stays on the real files

The undo journal and the capture read and write the files that changed on
disk, by their real names: they must restore exactly the bytes that were there. A change travels as
k7x path ops, diffed on the k7x text itself (`apps/k7-daw-backend/packages/k7-sync/src/k7x/`).

## Known limits

- **A finding with no path has no line**, since the line is computed from the key path.
