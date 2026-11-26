# Importing a foreign project

`POST /import` turns an Ableton Live set (`.als`), a DAWproject (`.dawproject`) or a zipped Ableton folder (`.zip`) into a new k7
project folder. The translation is the `k7-import` crate (`packages/k7-import`); the route is the front's `native/import_run.rs`,
over the file service's `edits/import.rs` (what is read, named and planned) and the change set's landing (`docs/file-changes.md`).

## Choices

| choice | why |
|---|---|
| the translation is a crate of the backend's workspace that returns files and touches no disk | the file service reads the workspace, names the folder and lands the files under its locks; the translator stays a function of bytes |
| it depends on `song-edit` for a clip's window and on `k7x` for the printer, and on neither `Fs` nor the verbs | an import writes a folder that does not exist yet, so there is no project to stage over; the printer and the window are the only parts of the editor it shares, and every file comes out as k7 itself would print it |
| `quick-xml` reads the XML, `zip` and `flate2` inflate | a foreign file is untrusted XML and compressed bytes; the first is the one parser the crate adds, the others the backend already links |
| the XML is read into a plain tree, and the mappers walk it by tag | both formats are read by tag and by attribute, and Live's schema drifts between versions, so a typed model would break on every drift |
| one request for both entries: `{rel, as?}` for a file of the workspace, or the file's bytes with its name and `as` in headers | the page holds the bytes of a drop and the workspace holds the file of a click; a name in a URL is personal data, a name in a header is not |
| a drop takes the self-contained formats (`.dawproject`, `.zip`); a lone `.als` is imported from the tree | an `.als` names its samples by path and the importer resolves them beside it in the workspace, so a dropped one would import a project with no sound and nothing to say which files were meant |
| the folder is the name somebody typed, refused when taken; a derived name gets a `-2` suffix; a project never starts with `_` or `.` | the person looks the folder up afterwards, so a typed name is never quietly renamed; `_` and `.` are the shared and hidden folders' names |
| the folder is claimed with an exclusive `mkdir` before the first sample is written | two imports of one name must not mix their samples in a folder; the loser is told the name is taken |
| the samples go first into the new folder, then every text lands as one change set that creates the folder; a set that fails takes the folder away | a body cannot carry audio, and a project half landed is a project nobody asked for; the shared files written beside it are taken away too |
| the import is a single landing of the workspace, which has no project to image, so it is no journal step | the journal images a project folder and the new one has no before |
| the engine's plugin list is the `plugins` member of the verb host; no engine reads as "unknown" and the check of plugin names is skipped | a plugin the machine cannot load is stood in for by a placeholder node, and with no list every plugin would be taken for missing |
| a `.als` from the tree is translated twice, the first time to learn which samples it names | a mapper reads samples in order and the workspace's files are read by the route, so the set is asked what it needs before it is asked to translate |
| a sample reference climbing out of the workspace, an absolute one, or a hidden one is counted in the log and never read | an absolute path names a disk on another machine, and following it would let a crafted set read any file of this one |
| a file over 512 MiB is refused before it is read, an archive inflates to at most 512 MiB in all and names at most 20,000 files, and the bytes read are counted rather than the sizes declared | a few kilobytes declare gigabytes, and the page and the backend both hold what is read |
| XML that declares a doctype or an entity is refused whole, and a document nests at most 400 deep | neither format writes one, and a declared entity can expand without bound |
| nothing a zip names becomes a path: files are named from slugs and a content hash, and an audio file's extension is cut down to plain characters | a zip entry's `..` or backslash only chooses which sample a clip finds |
| a name in `_modules` that exists is never reused | an import must not silently shadow another song's module |

## What an Ableton set holds that is not the song

- **The set of a zip is the one `.als` outside `Backup/`.** Live writes a dated copy there on every save, each a valid `.als` of an
  older state; taking one would import last week's arrangement without a word. A Mac's `__MACOSX/` and `._` files are skipped the
  same way, and two sets side by side are refused by name rather than guessed between.
- **A sample is found where the set says, else by its file name** when exactly one file in the zip bears it: the relative path is
  right for a collected project, the name catches a folder zipped from a level above or below it.
- **Take lanes are left out.** The comp Live plays is ordinary clips on the main lane; a take nobody picked is not the song, and its
  file need not be in the project. They are counted in the log, like Session view clips.
- **A clip whose sample cannot be read is left out**, and the log names the file: placed on a file that is not there, one clip
  would make the loader refuse the whole song.
- **An unwarped clip's region is in seconds and a warped one's in beats**, both in the same `<Loop>` fields, and only `IsWarped`
  tells them apart. At 110 bpm the mistake lands every unwarped clip 1.83 times off, in a way that looks like an arrangement.
- **`<Loop>` holds two different things by its brace.** With the loop on, playback starts at `StartRelative` and the
  `LoopStart`/`LoopEnd` pair braces the whole clip; with it off, the pair is the region. A MIDI clip's brace is not carried, since a
  full-clip brace would slice the window from 0 and drop every note.
- **A warped clip's in-point is a beat of the clip's grid, not of the file.** The first warp marker says which beat the file's
  second 0 sits on, so the in-point is read through the markers. Copied straight, every clip whose first marker is off beat 0 plays
  the file that many beats late, and the ones that line up are what hides it.
- **The pitch lives on the `KeyTrack`, the mute is the inverse of `Speaker`, the pan is −1..1, the time signature is one packed
  integer**: the readings that fail silently when wrong.
- **Only a `<SampleRef>`'s `FileRef` is audio.** A bare one points at a device preset or a VST binary, and a real set held three
  strangers' home directories among them.
- **Live's own devices carry no portable state** and are reported and skipped; a rack is flattened away and the plugins inside
  cross over. Live names no role, so the first plugin of a MIDI track is its instrument.
- **A plugin's state is kept as its preset, not as parameters**, because the parameters Live names are a subset of a plugin's;
  the engine's parameter dump is a process of the machine and is not asked for from a request.

## What a DAWproject holds

- **A container plays a window of its children.** A child clamped to nothing is dropped rather than emitted at zero length, since the
  same material appears under several containers and a pile of zero-length clips reads as takes the song never had.
- **A child cut at its head plays from where the cut lands in it**, and an unwarped audio leaf counts its in-point in seconds.
  Bitwig writes every audio clip as a container whose `playStart` selects a slice of one take, and a read from the child alone
  plays every such clip from the top of the take.
- **Content written once and placed several times is read through its `reference`**: the content is shared and the placement,
  its time, duration and loop, is each clip's own. A reference to something that is not content is said to be that, rather than
  said to be undefined.
- **`enable="false"` is carried as a disabled placement**, under a disabled container too: the clip stays visible and silent.

## What the emitter decides

- **One file per recording, not per clip.** Audio is keyed by its content, since some exports write one file per clip, byte for
  byte identical; each clip keeps its own file for its window, fades and warp, and only the bytes are shared.
- **Every timing field is a timecode string**, never a bare number, which parses as bars.
- **A placement does not carry the clip's extent.** That belongs to the material, as a window in the clip's or sequence's file; a
  window at the material's start says so by omission.
- **A MIDI lane always has an instrument.** One that did not cross over is replaced by a library instrument, a node saying it is a
  stand-in, and the log counts them: the SFZ kit for a drum rack, the synth otherwise.
- **One module per plugin, one insert per use**, folded back into the module when the plugin is used once, unless the module's
  name is a lane's: a lane wins a bare name, and the step would otherwise name itself.
- **A plugin the engine does not know gets no module**: each use is a node on the not-supported placeholder, carrying the real name.
- **Audio and no MIDI is a bounced track**, so its instrument and note FX are dropped and its audio FX kept.
- **Every lane ends on a strip**, a node on the library's mixer with no module of its own, so the card draws its controls; a
  return bus and the master chain get none.
- **A lane runs into `MIX`, and `MIX` into the master run or `OUT`**, the shape every project has and the repair tools read.
- **Dead return buses are dropped, and the drop cascades.**
- **The graph is segments, so a send's level is lost** and the file says so at its top.
- **The surface is written**, because a project draws what it declares: an arrangement is what both formats carry.

## What is checked

`apps/k7-daw-backend/packages/k7-import/tests/golden.rs` holds a song that takes every branch of the emitter to its whole output, byte for byte;
`apps/k7-daw-backend/packages/k7-import/tests/import.rs` imports a DAWproject, an `.als` with a sample beside it and one outside, and a zipped folder, reads the files back
as k7x and compiles the result with the song crate; `apps/k7-daw-backend/packages/k7-front/src/routes/files/service/native/tests_import.rs` drives the route over a real account.
