# Getting a video ready — the probe, the plan, the copies

What a picture clip's copies are FOR is [../../song-video/docs/media.md](../../song-video/docs/media.md).
This page covers the modules that make them: `probe.ts` asks ffprobe, `demuxProbe.ts` reads the
file where there is none, `plan.ts` decides, and `transcode.ts` runs ffmpeg.

## ffmpeg makes every copy

Unlike a shot's sound (`packages/audio-analysis/docs/analysis.md`), a video copy has no engine
fallback: the engine decodes audio only. Without ffmpeg there is nothing to make, and the verb says
so rather than guessing. A desktop needs it
installed. The phone has none (`engine/k7-runtime/docs/android.md`).

## Without ffmpeg, an mp4, a .mov, a WebM or an mkv is still read

Knowing what a file IS is what lets a host with no ffmpeg say "ready as it is" rather than refuse,
and a phone's own camera files are mp4. So `probeVideo` falls back to mediabunny's demuxer
(`demuxProbe.ts`), the library the browser side already muxes with, for the ISO family (mp4, .mov)
and the Matroska one (WebM, mkv — a browser's own recordings, a picture lane's take). It answers in
ffprobe's own names: the codec and size off the track, the rate averaged over its first packets.
The pixel format comes from the codec string for VP9 and AV1, from the fourcc for ProRes, and from
the `avcC`/`hvcC` for H.264 and HEVC, since their WebCodecs strings carry no chroma or bit depth.
`demuxProbe.ffprobe.test.ts` holds the mp4/.mov read to ffprobe on H.264 (4:2:0, 10-bit, 4:2:2,
4:4:4), HEVC, AV1, VP9 and ProRes encodes. A codec mediabunny does not know (MPEG-4 Part 2, MJPEG)
reads as an unknown codec: it needs a copy either way, which this host cannot make. Any other
container is still a `NoFfmpegError`. Facts read this way carry `noFfmpeg`, and `picture prepare` then asks for nothing it cannot make: a source too tall to
preview is previewed as it is, and one that needs a copy is named, with what it is.

## The phone's browser plays HEVC

The playable list is what every browser decodes, which leaves HEVC out. On Android the browser is
the WebView, which hands HEVC to the phone's hardware decoder, so `ANDROID_PLAYABLE_CODECS` adds it
and what the phone's camera records plays without a copy. A 10-bit HDR recording is still refused:
whether a WebView takes it depends on the phone. The list is chosen by `process.platform`, so it is
the platform of the file service, which on the phone is also where the page runs.

The file probed and copied is an upload, so both open it under `-protocol_whitelist file,pipe`
through `@studio/ffmpeg`'s `input()` (`packages/ffmpeg/docs/ffmpeg.md`): a crafted container must
not make the file service fetch a URL.

## The plan is pure

`planVideo` reads only the probed facts, so the rule is pinned by tests that need no file. The rule
is a short list rather than a capability query. The file service, which decides, is not the
browser, which decodes, so it cannot ask the browser what it can play. The list is therefore what
EVERY browser this app runs in decodes: H.264, VP8, VP9 and AV1, 8-bit 4:2:0. A 10-bit or 4:2:2
stream of a listed codec counts as undecodable, since that is where browsers differ most and fail
silently. The export's own check (`assertDecodable`) is the backstop for whatever the list gets
wrong.

## A rotated stream gets an upright copy

A phone records portrait as a landscape stream with a display rotation (ffprobe's `rotation` side
data). Every browser reports such a `<video>` as portrait, but not every one uploads it to WebGL
that way: Chrome applies the rotation, Firefox uploads the frame as it is coded, and the compositor
then stretches a sideways picture into a portrait box. The export's decoder (mediabunny's samples)
also hands over coded frames. So a rotated stream counts as one a browser cannot be trusted with:
the plan asks for an optimized copy, and ffmpeg's re-encode applies the rotation, writing an
upright stream with none. `demuxProbe.ts` does not read an mp4's or a .mov's rotation: without
ffmpeg no copy can be made, and the phone's WebView, being Chromium, turns the clip upright itself.

## What the copies are

- **Optimized**: H.264, 8-bit 4:2:0, CRF 18, at the source's resolution. Near-transparent quality,
  since the export renders from it.
- **Proxy**: H.264 at 720p, CRF 26, `veryfast`.
- **Both**: no audio (a shot's sound is extracted apart), a keyframe every 30 frames so a scrub
  lands without decoding back to a distant keyframe, and `+faststart` so the preview can start
  before the file is all read.

Each copy is written to `<dest>.part` and renamed when ffmpeg finished, so an interrupted run leaves
nothing a compile would take for a finished copy.
