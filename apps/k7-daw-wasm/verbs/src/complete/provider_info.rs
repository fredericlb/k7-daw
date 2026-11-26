// provider_info.rs — what the palette shows of each completion provider: its display class and the clue beside the field.
// A reader would get wrong that a class is a type: it only picks the colour, so a clip target and an audio path are both `file`.

/// The display classes of an argument, mirrored on the wire as `ArgClass` in `web-protocol/commandTypes.ts`.
pub const ARG_CLASSES: [&str; 9] = [
    "track", "module", "file", "marker", "choice", "timecode", "number", "note", "text",
];

/// Each provider with its display class and its clue, in the order the catalogue documents them.
const PROVIDERS: [(&str, &str, &str); 25] = [
    ("tracks", "track", "a track of this project"),
    ("timelines", "file", "a timeline of this project (timelines/)"),
    ("laneTargets", "file", "a timeline of this project, or main"),
    ("timelineLanes", "track", "a lane of that timeline"),
    (
        "moduleAlternatives",
        "choice",
        "one of this module's alternatives — unnamed ones answer to their number",
    ),
    (
        "moduleEngines",
        "choice",
        "one of this module's engines — the folders under its engines/",
    ),
    // A clip id and an audio path are both material, which the palette colours as a file.
    (
        "clipTargets",
        "file",
        "a clip of this project, or one audio file (e.g. clips/kick.wav)",
    ),
    // A chain is neither a module nor anything on disk the `file` class would colour.
    (
        "chains",
        "choice",
        "a chain of this project, or one the médiathèque holds (@k7-core/<path>)",
    ),
    (
        "curves",
        "choice",
        "an automation curve of this project (curves/<id>.k7x)",
    ),
    ("modules", "module", "a module of this project"),
    // A node is drawn as a module is — it is one, in use — so the palette colours them alike.
    (
        "nodes",
        "module",
        "a node of this project — one of its uses of a module",
    ),
    (
        "baseModules",
        "module",
        "the module to inherit from — this project's and the account's libraries",
    ),
    (
        "engineModules",
        "module",
        "a plugin the engine scanned (@engine/<slug>) — it has no folder until one is given",
    ),
    (
        "categories",
        "module",
        "<category>/<name> — the category decides the module's kind and its ports",
    ),
    (
        "moduleTargets",
        "module",
        "a module of this project or an io source (io/MIDI), or <category>/ to name a new one",
    ),
    ("markerNames", "marker", "a marker of this project"),
    (
        "sequenceFiles",
        "file",
        "a sequence of this project (sequences/<name>.k7x)",
    ),
    (
        "audioFiles",
        "file",
        "an audio file of this project (e.g. clips/kick.wav)",
    ),
    // A sample is a file, in a library's samples/ or the project's.
    (
        "samples",
        "file",
        "a sample a library or this project holds under samples/ (@<library>/<path>, @project/<path>)",
    ),
    // A playlist IS its file: no marker inside a folder, the extension is the whole rule.
    (
        "playlists",
        "file",
        "a set list of this repository (_playlists/<name>.k7x)",
    ),
    // A library is a name from the origin's list, not a path.
    (
        "libraries",
        "choice",
        "one of the origin's libraries, as its refs name it (@<library>/…)",
    ),
    // A lane IS its file too, the same way — its node file, and the id is the stem.
    (
        "pictureLanes",
        "file",
        "a picture lane of this song (a @meta/picture-track node)",
    ),
    // Not "file": the surface's file does not exist yet — naming its type is what creates it.
    (
        "surfaceTypes",
        "choice",
        "which view to draw — the arrangement, the frame, or the desk. Its layout follows.",
    ),
    // A box is one of a closed set of names, not a module and not a file the palette would colour.
    (
        "assemblies",
        "choice",
        "a box this project declares (assemblies/<name>.k7x)",
    ),
    // …and a template IS its folder in the library, named the way a module ref is.
    (
        "assemblyTemplates",
        "module",
        "an assembly template the library holds (@k7-core/<path>) — its nodes and wiring are copied in",
    ),
];

fn row(name: &str) -> Option<&'static (&'static str, &'static str, &'static str)> {
    PROVIDERS.iter().find(|p| p.0 == name)
}

/// Whether the catalogue knows a provider by this name.
pub fn is_provider(name: &str) -> bool {
    row(name).is_some()
}

/// Every provider name, in the table's order.
pub fn provider_names() -> impl Iterator<Item = &'static str> {
    PROVIDERS.iter().map(|p| p.0)
}

/// The display class a provider's arguments take.
pub fn provider_class(name: &str) -> Option<&'static str> {
    row(name).map(|p| p.1)
}

/// What a provider offers, at PROVIDER level: "a sequence" and "an audio file" are different answers.
pub fn provider_hint(name: &str) -> Option<&'static str> {
    row(name).map(|p| p.2)
}

/// What a scalar class accepts, for arguments with no provider and no hint; the other classes promise nothing.
pub fn class_hint(class: &str) -> &'static str {
    match class {
        "timecode" => "bar:beat:sixteenth — 5, 5:1, 5:1:1",
        "number" => "a number",
        "note" => "a note name (C4, F#3) or a MIDI number 0…127",
        _ => "",
    }
}
