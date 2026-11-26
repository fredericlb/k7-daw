# The translator: importing other DAWs' projects

The crate `apps/k7-daw-backend/packages/k7-import` turns another DAW's project into the files of a k7 project folder. It
has no routes and no UI; the backend's `POST /import` is its only caller (`apps/k7-daw-backend/docs/import.md`).

## Shape

```text
.dawproject ──dawproject.rs──┐
.als ─────────ableton.rs─────┼──► Song (model.rs) ──emit.rs──► the files of <workspace>/<name>/
.zip (Live) ──als_zip.rs─────┘
```

| File | Role |
|---|---|
| `apps/k7-daw-backend/packages/k7-import/src/model.rs` | The neutral `Song` every importer produces: tracks with role, content type, colour, linear volume, 0..1 pan, mute, plugins, sends and clips; clips with start/length in beats, offset, fades, MIDI loop points, notes, disabled, warp markers. Deliberately no foreign format's model, so the emitter is format-blind. |
| `apps/k7-daw-backend/packages/k7-import/src/dawproject.rs` | Unzips in memory, parses `project.xml` (quick-xml, through `xml.rs`), maps it onto the model. No disk I/O: it returns the song plus every asset to write. |
| `apps/k7-daw-backend/packages/k7-import/src/ableton.rs` | The same for a gzipped `.als`, with a sample reader so the caller decides which sample paths may be read. |
| `apps/k7-daw-backend/packages/k7-import/src/als_zip.rs` | A zipped "Collect All and Save" folder: picks the one set outside `Backup/` (the shallowest), resolves samples inside the archive, then reads the set. |
| `apps/k7-daw-backend/packages/k7-import/src/inflate.rs` | Every unzip and gunzip goes through it, with a ceiling on what an archive may inflate to, counted on the bytes read — an import is a file a stranger may upload. |
| `apps/k7-daw-backend/packages/k7-import/src/emit.rs` | Writes the folder: `project.k7x`, the graph as segments (who feeds whom, which is how sends survive), one module per distinct plugin, nodes where a plugin's settings differ, clips, sequences, and a mixer strip per lane. |

Choices worth knowing before changing it:

- **Lossy on purpose, and loud about it.** Whatever the model cannot carry becomes a line in
  `warnings`: return and group tracks in Live, Session-view clips, un-comped take lanes, Live's
  native devices, rack structure, missing samples, and dropped clips with the reason (a dangling id,
  an id that is not clip content, a clip with nothing in it). The caller prints them in the import log.
- **A mute is a bypass.** A track that arrives muted is written as a bypassed lane, which is where a
  lane's mute lives in k7.
- **Instruments that cannot cross get a stand-in.** A MIDI lane whose instrument did not translate
  (a Live drum rack, say) plays `@k7-core/instruments/k7-rack` for drums or a library synth otherwise,
  and the log says so.
- **Modules land in the workspace's shared modules**, and names already used there are avoided, so an
  import never silently reuses or shadows another song's module of the same name.
- **Send levels are lost** in the graph's segments: a weighted send arrives at full level, with a
  note in the project.
- Unwarped audio counts its in-point in seconds in DAWproject; the model counts beats. That
  conversion is the most fragile line in `dawproject.rs` and is pinned by its tests.

## The caller

`POST /import` (`apps/k7-daw-backend/packages/k7-file-service/src/edits/import.rs`, the front's `native/import_run.rs`):

- a file of the workspace, `{rel, as?}`: `.dawproject`, `.als` or `.zip`; a `.als` reads the samples it names beside it;
- a dropped file's bytes, its name and folder in headers: `.dawproject` or `.zip` only, since a lone `.als` names its
  samples by path and must sit beside them.

It picks a fresh top-level folder (the file's stem, `-2`, `-3`… when taken; a name somebody typed is refused if occupied
rather than changed), writes the samples first, lands the texts as one set that creates the folder, and answers
`{ project, log }`.

In the editor, a click on such a file in the tree (`EditorTab.tsx`'s `open`) or `project import <path>`
opens the two-phase dialog of `apps/k7-daw-frontend/src/hooks/useProjectDialogs.ts` and
`apps/k7-daw-frontend/src/components/ImportDialog.tsx`: confirm runs the import and shows the log; **Open project** then
activates the result. The drop on a workspace row belongs to the home page
(`apps/k7-daw-frontend/src/projects/`).

## Tests

`apps/k7-daw-backend/packages/k7-import/tests/import.rs` imports a `.dawproject`, a `.als` with samples beside it and one
outside, and a zipped Live folder, reads the files back and compiles them; `tests/golden.rs` holds the emitter to a fixed
corpus. The route's own tests are the front's `native/tests_import.rs`.

## See also

- [Rendering, export and bouncing](./26-render-process-code.md)
