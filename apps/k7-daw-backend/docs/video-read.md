# Reading a video without ffprobe

`probe` asks ffprobe first. Where the program is absent (the phone), `k7-mediatek`'s `video_read.rs` fills the same
facts from the file's own headers (decision 3f-6).

## Choices

| choice | why |
|---|---|
| `mp4parse` (Firefox's, MPL-2.0) reads ISO files, `matroska-demuxer` (MIT) reads Matroska and WebM | two small pure-Rust crates build for the phone targets; shipping ffmpeg there is not an option |
| the codec and pixel format of an ISO file come from our own walk of the sample description | `mp4parse` names no HEVC, ProRes or unknown entry and keeps no `hvcC`; it supplies the movie timescale, the track header's matrix and the sample durations |
| the container is sniffed by magic bytes | a file's name is the user's; a `.mp4` that is a WAV must not be read as a film |
| ISO reads only the `moov` box, found by hopping over the top-level boxes | `mdat` is never read, and a size past the end of the file or past 64 MiB is refused before any allocation |
| Matroska reads through a 64 MiB read budget | the crate checks an element's size against the stream, and the budget bounds what a crafted index can make it read |
| a panic in either parser is an `unsupported` | a hostile file must not take the front down |
| a file neither reads answers 415 with the code `unsupported`; a readable file with no video stream answers `facts: null` | the caller tells "cannot say" from "no picture" |
| the frame rate is the samples over their summed durations (ISO) or `av_reduce` of the track's default duration with a 30000 bound (Matroska) | these are the numbers ffprobe states, so the plan cannot tell who read the file |
| the colour description (`colr`, Matroska `Colour`) is not read | `VideoFacts` carries none; the picture's colour handling does not use the probe |

Known limits: a fragmented ISO file states no duration or rate in its `moov`, so both read 0; an edit list does not
shorten the duration; a Matroska rotation is not read (ffprobe states none for the usual cameras).

Third-party notices: `apps/k7-daw-frontend/notices/components.json` lists both crates.
