# Recording a project's output

A project with no timeline — a `live` rig, a set of effects you play through, a camera and its
frame — has one button in the project bar: **record**. It records what the project sends out:
the sound that reaches `OUT`, and the picture `RENDER` draws, when the project has one. Pressing
it again stops the take and keeps it in the project's record directory: a `.wav` for the sound
alone, a `.webm` holding both when there is a picture, the picture alone when the graph reaches
no `OUT`.

A project WITH a timeline records takes onto its lanes instead, armed and rolled with the
transport; it has no such button.

## The sound: a recorder no node names

The engine's own `Recorder` built-in does the capture (`engine/k7-runtime/docs/recorder.md`): it
writes a wav as the audio passes, free-running, the transport never consulted. The compile puts
it **last on the master run** of a project with no timeline whose graph reaches `OUT`
(`src/outputRecorder.ts`), keyed `MIX:@record` (`OUTPUT_RECORDER_ID`), so a take is the whole mix,
post-master-chain — what `OUT` receives.

- **It is not a node.** Recording what a project sends out is a property of the project, not
  something placed in its graph: a node could be put anywhere and capture one voice, be deleted
  and leave the rig with nothing to record with, and its card's buttons drew a second, smaller
  record control beside the bar's. The graph shows the sound's path, and nothing on it records.
- **One id for every project's recorder**, so the button waits for the engine to run THIS
  project's document, as play does: the last project's recorder state is still in the store a
  moment after a switch, and a press then would be refused as "no recorder node".
- **No timeline, and something reaching `OUT`**, decides it. A rig whose graph sends nothing out
  has nothing to record; a project with a timeline already records on its lanes.

## The picture: captured by the page

The engine never sees a picture: the compositor is the browser's
([videoOutputs.md](videoOutputs.md), [camera.md](camera.md)). So while a take rolls, the page
draws the project's frame a second time, **off screen**, on a driver that never parks
(`packages/k7-plugins/src/video/OutputPicture.tsx`), and records that canvas with a
`MediaRecorder`, its chunks spooled to disk as a picture lane's take is
(`apps/k7/frontend/docs/picture.md`).

- **Not the viewport.** It may be hidden, and a hidden viewport stops drawing; a recording must not
  depend on which window happens to be open. The copy costs a GL context while a take rolls, and
  nothing otherwise.
- **The page that pressed record records the picture.** With no page open there is no picture to
  record — a camera is opened by a browser, and so is every lane's video.
- **The button waits for the picture's read.** The engine runs the recorder before the page's Worker has
  compiled the picture, and a press while that read is pending sees no picture and files the sound alone. So
  the bar offers record only once the read has answered (`useOutputRecording.ts`'s `outputAvailable`).
- **The sound waits for the picture.** With a picture, the engine's recorder is started when the
  `MediaRecorder` says it has started, not at the press: drawing the frame and opening its recorder
  take a moment, and a sound begun at the press would run ahead of its picture in the joined file.
  The two are then aligned to within one message down the socket. The picture's code is fetched
  with the project, so the moment is short; a take stopped before the picture began records nothing.

## Keeping it

On stop the page waits for the engine to close its take, then **joins the two itself**
(`packages/ui-eyes/src/joinTake.ts`): the picture's packets copied as its `MediaRecorder` encoded
them, the engine's wav read through the project's media route and encoded to Opus by WebCodecs,
one WebM streamed to a scratch file on disk as it is made. It uploads that into the project's
`clips/` — the record directory the engine writes its takes into — and runs
`project record save <take.wav|none> [<picture.webm>]` (`packages/commands/commands.yaml`). The
file service, which runs on the engine's machine, keeps what it is handed in that directory and
answers the take's file name (`apps/k7/backend-rs/crates/k7-file-service/src/media/mod.rs`):

| what was recorded | what is kept |
|---|---|
| the sound | the engine's wav |
| the sound and a picture | one `.webm`, joined by the page |
| a picture, with nothing reaching `OUT` | the picture as recorded |
| both, in a browser that cannot encode Opus | the wav and the picture, two recordings side by side |

- **A take is found by its name in `clips/`.** An engine that plays a mirror of the project — a
  phone's own, a machine's over the tunnel — names the take in its mirror, which the tunnel copies
  back under the same name; the file service waits for that copy to land and keeps it. Only the
  name is taken from the path the page hands over, so nothing outside the record directory is opened.
- **Joined in the browser, not with ffmpeg.** The page already holds the picture and already ships
  the muxer the video export uses (mediabunny); the machine running the file service — a laptop, a
  phone — is not guaranteed a binary it would need for this one job. The picture is copied, never
  re-encoded: only the sound is, which costs a few seconds per hour of take.
- **The join is streamed to disk**, in the origin's private file system as a picture lane's take
  is: an hour of picture is gigabytes a tab should not hold.
- **Both paths are CHECKED** against the project's record directory before anything is opened:
  they come from a browser, and a browser naming any file on the machine is what the check is for.
- **The take keeps its own time**, the wav's mtime, not the moment of the save: a take kept an hour
  later belongs to the hour it was played.
- **The engine's take is cleared only once kept.** A refused save leaves the take where it is and
  says why; the uploaded picture is removed only with a kept recording.
- **A picture that could not be uploaded goes to the downloads**, since it cannot be recorded again.
