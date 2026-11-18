# The tracks export

`GET /export/tracks` answers one zip: for each track of the open project its audio without effects, its notes as a
MIDI file and a page of the chain it has in the project. It sits beside `/export` in `shares/` (`file-shares.md`) and
code lives in `k7-front/src/routes/files/service/native/shares/tracks/`.

## What the zip holds

| entry | content |
|---|---|
| `README.md` | the project's name, tempo, sample rate, time signature, master volume and master chain with every param, and the tracks with the files each got |
| `<nn>-<track>.wav` | the track's clips as placed, with no module at all; absent when the track has no audio clip |
| `<nn>-<track>.mid` | the track's notes, one tempo and one time signature from the project; absent when it has no notes |
| `<nn>-<track>.md` | name, content type, role, group, whether the lane is frozen, the chain in order with, for each node, the modules it extends and, for every param sorted by name, the value and where it comes from, the params that are automated and the range of their curve, the sends with their levels |

| choice | what forced it |
|---|---|
| the audio is the engine's own stem render of a stripped copy of the compiled document, through the port `/export` renders with | the same renderer serves a local engine and an engine's machine; a second renderer would drift from the first |
| the strip removes every module (the Mixer included), the sends, the automation, mute and solo, the master chain, and sets `masterVolume` to unity; the clips are not touched | a track's audio is the material the author placed; what the chain does to it is the project's, and is told in the page instead |
| a track is renamed `t<index>` in the stripped document and mapped back through the stem manifest | two tracks of one name, or two names that sanitize alike, would write the same stem file |
| a track with no enabled audio clip, a pattern lane and a bus are left out of the render; a render that finds nothing audible leaves the stems out and the zip still holds the `.mid` and `.md` files | their stem would be silent, and the engine skips a silent stem; the notes and pages do not depend on it |
| a frozen lane's audio and notes are its authored clips (`frozenMaterial`), not the render the engine plays in their place | the render is taken at the frozen module's output, so it already carries the chain the page tells |
| MIDI clips are left out of the render | with no instrument they make no sound; the notes travel as the `.mid` |
| notes follow the engine's own placement: a disabled clip is skipped, a loop with no `skip` repeats its window to the clip's end, a partial last pass kept and cut there, and one with a `skip` rounds the pass count as the engine does; a clip with no length is four beats long, a pad's `skip` and `repeat` are realised, a clip's `offset` is not read | the file must say what the engine plays, not what the author might have meant |
| a beat is the time signature's denominator note, as the engine's clock counts it, and the file's grid is 480 ticks to the quarter: a note at beat `b` lands at `b * 480 * 4 / denominator`, the tempo meta stays per quarter; one format-0 track | in 7/8 a beat is an eighth, so a bar of 7 beats is 1680 ticks and plays at the engine's speed; the project holds one tempo and one time signature |
| each param on a node's page says whether the node sets it, a module it extends gave it, or nothing in the files did (a module default); the value stays the compiled one | the engine's document carries no origin, so it is read from the project's nodes and the modules they extend, by the loader the compile uses; a project whose files will not read gets the values with no origin |
| the MIDI file is written by `midly` and the archive by `zip`, both with default features off and deflate by `zlib-rs`, which is pure Rust | no C library enters the build (`docs/rust-builds.md`) |
| the WAVs are stored, the `.md` and `.mid` deflated | audio does not deflate, and the archive is not worth a second pass over gigabytes |
| the archive is written into the private folder, each WAV removed as it is copied in, and streamed like `/export`'s file; the folder goes with the body | the answer has a known length, and the folder holds the audio once |
| the gates are `/export`'s: the same `timeline` parameter, the same refusals (no project open, no timeline of that name, no engine) | a user who is refused one is refused the other, in the same words |
| the workspace rides in the query when the header is absent and `Sec-Fetch-Site` is absent, `same-origin` or `none`, for this route alone | the browser downloads a link, which sends no header; a page on another site could put the same link in an image and start a render on the user's engine |
| the page names the request with `dl=<token>` and the route sets the cookie `k7dl-<token>` on its answer: `ok` on a zip, `err.<sentence>` on a refusal; the panel polls it | a link gives the page no status, so the cookie is the one signal that the download began or why it did not; it holds a second click off until then |

## Not proven here

The audio is the engine's output: no test in this crate renders it. A bed that has an engine runs
`e2e/specs/app/offlineRender.spec.ts`, whose tracks case asks the zip of `clip-lane.k7`. Whether the engine plays a
track whose chain is empty at unity, with no Mixer module, and whether a 7/8 project's notes sit where the engine plays them (the beat of the time signature's denominator), are read from its code and left to that run.
