// providerInfo.ts — what the palette shows of each completion provider: its display class and the clue beside the field.
// What a reader gets wrong: the providers themselves are Rust (apps/k7-daw-wasm/verbs/src/complete/); only their names and looks are here.

// The DISPLAY class of an argument, mirrored on the wire and kept in step by a test.
const ARG_CLASSES = [
  "track",
  "module",
  "file",
  "marker",
  "choice",
  "timecode",
  "number",
  "note",
  "text",
] as const;

export type ArgClass = (typeof ARG_CLASSES)[number];

// Which class each provider serves; the names are the providers the crate's completion answers (apps/k7-daw-wasm/verbs/src/complete/).
export const PROVIDER_CLASS = {
  tracks: "track",
  timelines: "file",
  laneTargets: "file",
  timelineLanes: "track",
  moduleAlternatives: "choice",
  moduleEngines: "choice",
  // A clip id and an audio path are both material, which the palette colours as a file.
  clipTargets: "file",
  // A chain is neither a module nor anything on disk the `file` class would colour.
  chains: "choice",
  curves: "choice",
  modules: "module",
  // A node is drawn as a module is — it is one, in use — so the palette colours them alike.
  nodes: "module",
  baseModules: "module",
  engineModules: "module",
  categories: "module",
  moduleTargets: "module",
  markerNames: "marker",
  sequenceFiles: "file",
  audioFiles: "file",
  // A sample is a file, in a library's samples/ or the project's.
  samples: "file",
  // A playlist IS its file: no marker inside a folder, the extension is the whole rule.
  playlists: "file",
  // A library is a name from the origin's list, not a path.
  libraries: "choice",
  // A lane IS its file too, the same way — its node file, and the id is the stem.
  pictureLanes: "file",
  // Not "file": the surface's file does not exist yet — naming its type is what creates it.
  surfaceTypes: "choice",
  // A box is not a module and not a file the palette would colour: it is one of a closed set of names.
  assemblies: "choice",
  // …and a template IS its folder in the library, named the way a module ref is.
  assemblyTemplates: "module",
} as const satisfies Record<string, ArgClass>;

export type ProviderName = keyof typeof PROVIDER_CLASS;

// What each provider offers, at PROVIDER level: "a sequence" and "an audio file" are different answers.
export const PROVIDER_HINT: Record<ProviderName, string> = {
  moduleAlternatives:
    "one of this module's alternatives — unnamed ones answer to their number",
  moduleEngines:
    "one of this module's engines — the folders under its engines/",
  tracks: "a track of this project",
  timelines: "a timeline of this project (timelines/)",
  laneTargets: "a timeline of this project, or main",
  timelineLanes: "a lane of that timeline",
  clipTargets:
    "a clip of this project, or one audio file (e.g. clips/kick.wav)",
  chains:
    "a chain of this project, or one the médiathèque holds (@k7-core/<path>)",
  curves: "an automation curve of this project (curves/<id>.k7x)",
  modules: "a module of this project",
  nodes: "a node of this project — one of its uses of a module",
  assemblies: "a box this project declares (assemblies/<name>.k7x)",
  assemblyTemplates:
    "an assembly template the library holds (@k7-core/<path>) — its nodes and wiring are copied in",
  baseModules:
    "the module to inherit from — this project's and the account's libraries",
  engineModules:
    "a plugin the engine scanned (@engine/<slug>) — it has no folder until one is given",
  categories:
    "<category>/<name> — the category decides the module's kind and its ports",
  moduleTargets:
    "a module of this project or an io source (io/MIDI), or <category>/ to name a new one",
  markerNames: "a marker of this project",
  sequenceFiles: "a sequence of this project (sequences/<name>.k7x)",
  audioFiles: "an audio file of this project (e.g. clips/kick.wav)",
  samples:
    "a sample a library or this project holds under samples/ (@<library>/<path>, @project/<path>)",
  playlists: "a set list of this repository (_playlists/<name>.k7x)",
  libraries:
    "one of the origin's libraries, as its refs name it (@<library>/…)",
  pictureLanes: "a picture lane of this song (a @meta/picture-track node)",
  surfaceTypes:
    "which view to draw — the arrangement, the frame, or the desk. Its layout follows.",
};

// What a scalar class accepts, for arguments with no provider and no hint — nothing else in the palette says it.
export const CLASS_HINT: Record<ArgClass, string> = {
  timecode: "bar:beat:sixteenth — 5, 5:1, 5:1:1",
  number: "a number",
  note: "a note name (C4, F#3) or a MIDI number 0…127",
  choice: "", // its values ARE the clue — see argHint
  text: "", // free text: there is nothing to promise
  // The project-backed classes never reach here — their provider answers first.
  track: "",
  module: "",
  file: "",
  marker: "",
};

export const PROVIDER_NAMES: ReadonlySet<string> = new Set(
  Object.keys(PROVIDER_CLASS),
);
