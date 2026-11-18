# The formats a clip can be in

A clip names a file, and the engine opens it through JUCE's registry of decoders. Which formats are
in that registry is part the build's choice and part the operating system's, and the two do not
agree: the same song is silent on a Linux engine and plays on a phone. Two things follow from it —
what is registered where, and what stays unplayable whatever the build.

## What is registered where

| decoder | comes from | in this build |
|---|---|---|
| wav, aiff | JUCE, always | everywhere |
| flac, ogg | JUCE, `JUCE_USE_FLAC` and `JUCE_USE_OGGVORBIS`, on by default | everywhere |
| mp3 | JUCE's own decoder, behind `JUCE_USE_MP3AUDIOFORMAT`, **off by default** | everywhere, because `CMakeLists.txt` sets it |
| mp3, m4a, … through CoreAudio | macOS and iOS | the desktop engine and the phone app |
| mp3, wma, … through Windows Media | Windows, `JUCE_USE_WINDOWS_MEDIA_FORMAT` | the Windows builds |
| — | Linux has no platform decoder at all | — |

mp3 is the one format a build has to CHOOSE. `AudioFormatManager::registerBasicFormats()` skips it
unless the flag is set, and a Linux engine has no second chance — so before the engine set the flag,
every mp3 a workspace held played everywhere and made a silent clip on the server's `engine`
service. The flag is set for the WHOLE
build rather than per platform: a phone, a Mac and a Linux engine must agree about what a song is, or
a gig prepared on one machine fails on another.

It is a software decoder, compiled into `juce_audio_formats` inside the Tracktion target, and that is
the whole cost — no library, no device, no server-side service. mp3's patents lapsed in 2017; the
disclaimer JUCE prints beside the flag predates that.

## What stays unplayable

`m4a`, `opus` and `webm` are three of the nine extensions a workspace takes as audio
(`packages/song-schema/src/audioFiles.ts`), and JUCE has no decoder for any of them. They reach the engine on
an Apple machine, through CoreAudio, and nowhere else: a file in one of them is silent on a Linux
engine, with the same one-line warning and no way to tell it from a missing file. A format the engine
cannot read is a reason to keep recordings as wav or flac.

## The turntable's cache follows the same list

`RecordSource` maps a wav and decodes anything else ONCE into a wav in the record cache
([turntable.md](turntable.md)), using `registerBasicFormats()` like everything else here — so a
record the clip path could not open is a record the platter cannot decode either, and the deck's
silence has the same cause.
