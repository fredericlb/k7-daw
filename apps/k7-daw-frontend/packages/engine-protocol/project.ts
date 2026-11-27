// project.ts — the compiled project the engine plays: the ProjectDoc tree, exactly as it goes on the wire.
// Types only, GENERATED from the Rust structs of apps/k7-daw-wasm/song/src/doc and clips/doc.rs (ts-rs), so the C++ mirror
// in engine/ProjectDoc.h has one counterpart and no more.
//
// This is NOT "a k7 song". It is the OUTPUT of a compiler, and any app that can build
// one can drive the engine — pl8's module preview is a one-track doc, a DJ deck
// is two. That is why it lives here and not in @studio/song, which would drag the whole
// workspace compiler in behind it. `@studio/song` re-exports these and owns the code that
// PRODUCES them; `./build` has constructors for callers that have no song folder.

export type {
  AutomationDoc,
  ClipDoc,
  EventDoc,
  GroupDoc,
  Marker,
  NestDoc,
  NoteDoc,
  PluginDoc,
  ProjectDoc,
  RackSoundDoc,
  SendDoc,
  SettingsDoc,
  TimelineDoc,
  TimelineLaneDoc,
  TrackDoc,
  WarpDoc,
} from "@studio/k7-wasm/types";
