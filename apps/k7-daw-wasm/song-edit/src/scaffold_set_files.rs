// scaffold_set_files.rs — the `mix` archetype's project, graph, cue sheet, desk and assemblies, as the TypeScript's `setProjectFiles` writes them.
// Hand-edited Rust, the archetype's source: no generator writes it; `@@NAME@@` stands for the project's name.

pub const SET_FILES: &[(&str, &str)] = &[
    (
        "project.k7x",
        r#"// A SET: two decks meeting at the mixer, and a list of songs to play through them.
//
// There is no arrangement, and nothing here says so: a project draws what it DECLARES,
// and this one declares no `surfaces/timeline.k7x`. What makes it a set rather than a
// bare rig is the `turntables` surface in `surfaces/` — and the PLAYLIST that surface
// names, which is where the songs come from. See apps/k7-daw-wasm/song/docs/setProject.md.
project({
  name: "@@NAME@@",
  // The rate the decks are played at. No tempo and no time signature: a set has no bars.
  sampleRate: 48000,
})
"#,
    ),
    (
        "mix.k7x",
        r#"// The CUE SHEET — where THIS set's cue points are, song by song.
//
// NOT `views/mix.k7x`, which is a different file entirely (the desk's old bindings; a
// surface carries its `mapping` now). This one is at the root, and it is the desk's.
//
// It does NOT say which songs the set plays: that is the playlist `surfaces/desk.k7x`
// names, a file in `_playlists/` at the root of the repository, written in k7 under the
// cog. See apps/k7-daw-wasm/song/docs/playlists.md.
//
// What is left here is the half a playlist has no place for. A cue belongs to the DESK
// that plays a song, not to a list other sets read: two bands covering the same track
// drop the needle in two different places, and a playlist carrying one set's marks would
// hand them to the other.
//
// The desk writes this file itself — you drop a cue and the line appears. It is here to
// be read, corrected and committed, not filled in by hand.
//
// A ref is `@k7-core/` followed by the médiathèque id, verbatim, and the ids are what
// pl8 shows: `songs/<artiste>/<album>/<titre>`.
//
// `markers` is the desk's eight cue buttons, positional, in seconds into the FILE —
// never the transport's clock, which moves the moment somebody touches a tempo fader:
//
//   ref("@k7-core/songs/artiste/album/titre").markers([12.5, null, 48.25]) // boutons 1 et 3; le 2 est vide
cues([])
"#,
    ),
    (
        "graph.k7x",
        r#"// Each deck through its own trim, EQ, filter, LEVEL, echo and fade, and the two meeting
// at `MIX`. That order is a channel strip's, and every step of it is explained below.
//
// The fade is what a crossfader is here, and it is TWO modules rather than one because
// a module's second port is wired as a sidechain, and a sidechain reads a track only
// while that track also reaches an audible output. A deck feeding nothing but a
// crossfade would therefore be heard BESIDE it, raw, with nothing able to take it back:
// not a mute, not a disconnect, not a routing — all four were measured, see
// apps/k7-daw-engine/k7-runtime/docs/sidechain.md. One input each, and the blend at the mixer.
//
// Each fade's `Level` IS its lane's gain, so its panel reads true on its own: 0 silent, 1
// full. The desk drives the two in MIRROR — one control, deck A inverted — which is said
// in `surfaces/desk.k7x` rather than by either module knowing about the other.
//
// The names here are NODES, from nodes/ — not modules. The modules themselves
// live in the médiathèque and are not copied in.
//
// The EQ and the filter sit AFTER the tempo and BEFORE the fade, which is where a
// mixing desk puts them: a channel strip is shaped on its way to the fader, so cutting a
// deck's bass is a property of that deck and not of the blend.
//
// Filter LAST of the three. It is the effect a hand throws — the one you want on top of
// whatever the EQ has already done, rather than equalised afterwards.
//
// THE TRIM IS FIRST, and it is first because a deck is FIVE lanes: putting it on the
// deck's own lane would leave it doing nothing at all on a separated record, whose four
// stems play instead of the disc. Being the chain's head it trims all five together,
// which is what an input gain is for — two records cut ten years apart do not arrive at
// the same level, and nothing here normalises loudness.
//
// THE LEVEL IS THE CHANNEL FADER, and it is a different control from the crossfade below
// it: most of a transition is made at the channel, the crossfader being for hard cuts.
// In DECIBELS, unlike the fade — a level is read in dB, a crossfade position is not.
//
// THE ECHO IS AFTER THE LEVEL, which is the whole of what makes an echo-out possible:
// cut the channel and the delay line keeps ringing, because what you cut was its INPUT
// and not its output. Before the fader it would be cut with everything else.
//
// …and the headphone tap stays on `filter`, ahead of both — that is what pre-fade listen
// means. Neither the channel fader nor the crossfader may take a deck out of your ears.
//
// Nothing about two is fixed. A third deck is a third lane, a third tempo and a second
// meeting point; the desk reads whatever is here. See apps/k7-daw-wasm/song/docs/ports.md.
//
// EACH DECK IS FIVE LANES: the record, and one per separated stem. Only one side plays —
// a separated song leaves the record's lane empty and the four parts SUM back to it; an
// unseparated one is the other way round. How loud each part plays is its PLATTER's own
// gain: 0 drops the instrument, 1 is as it was cut, 2 doubles it.
// See apps/k7-daw-wasm/song-schema/docs/stems.md.
//
// Five feeders is what makes `trim*` a MEETING POINT, and a meeting point breaks a run:
// the trim, the EQ, the filter and the fade become a chain of their own — a bus — and
// each of the five lanes reaches it as a send. The parts must sum BEFORE the EQ, because
// a deck's strip equalises what it sends to the fader and not each instrument apart.
//
// Both fades land on `MIX`, the mixer's sum, and `MIX -> OUT` is the run out to the
// interface — the MASTER CHAIN. A module named on that line processes the whole blend,
// which on a set is where a limiter belongs. `OUT` carries the output level as
// its `Volume`.

modules({
  deckA: ["platterA"],
  platterA: ["tempoA"],
  tempoA: ["trimA"],
  stemDrumsA: ["platterDrumsA"],
  platterDrumsA: ["trimA"],
  stemBassA: ["platterBassA"],
  platterBassA: ["trimA"],
  stemOtherA: ["platterOtherA"],
  platterOtherA: ["trimA"],
  stemVocalsA: ["platterVocalsA"],
  platterVocalsA: ["trimA"],
  trimA: ["eqA"],
  eqA: ["filterA"],
  filterA: ["levelA", "cueA"],
  cueA: ["CUE"],
  levelA: ["echoA"],
  echoA: ["fadeA"],
  fadeA: ["MIX"],
  deckB: ["platterB"],
  platterB: ["tempoB"],
  tempoB: ["trimB"],
  stemDrumsB: ["platterDrumsB"],
  platterDrumsB: ["trimB"],
  stemBassB: ["platterBassB"],
  platterBassB: ["trimB"],
  stemOtherB: ["platterOtherB"],
  platterOtherB: ["trimB"],
  stemVocalsB: ["platterVocalsB"],
  platterVocalsB: ["trimB"],
  trimB: ["eqB"],
  eqB: ["filterB"],
  filterB: ["levelB", "cueB"],
  cueB: ["CUE"],
  levelB: ["echoB"],
  echoB: ["fadeB"],
  fadeB: ["MIX"],
  MIX: ["OUT"],
})

// Heard through the bus ONLY — see scaffoldSet.ts for why this is not optional.
bypassed(["deckA", "stemDrumsA", "stemBassA", "stemOtherA", "stemVocalsA", "deckB", "stemDrumsB", "stemBassB", "stemOtherB", "stemVocalsB"])

// The strips, for the SAME reason and NOT the same word: each one now feeds two buses (the
// fade and the cue), so its own dry path would reach the master unfaded. `bypassed` cannot say
// this — it would ALSO turn the EQ off, the chain being named after its first module. See
// apps/k7-daw-wasm/song/docs/busOnly.md.
bus_only(["trimA", "trimB"])
"#,
    ),
    (
        "assemblies/turntableA.k7x",
        r#"// turntableA — deck A's nodes, drawn as one appliance.
//
// This file changes NOTHING about the sound: it says which of the graph's nodes are one
// box and nothing else. Its plugs and its knobs come from the template it names, which
// lives in the account's library — see apps/k7-daw-wasm/song/docs/assemblies.md.
uses("@k7-core/turntable")
members({
  deck: "deckA",
  platter: "platterA",
  tempo: "tempoA",
  trim: "trimA",
  eq: "eqA",
  filter: "filterA",
  level: "levelA",
  echo: "echoA",
  fade: "fadeA",
  cue: "cueA",
  stemDrums: "stemDrumsA",
  platterDrums: "platterDrumsA",
  stemBass: "stemBassA",
  platterBass: "platterBassA",
  stemOther: "stemOtherA",
  platterOther: "platterOtherA",
  stemVocals: "stemVocalsA",
  platterVocals: "platterVocalsA",
})
"#,
    ),
    (
        "assemblies/turntableB.k7x",
        r#"// turntableB — deck B's nodes, drawn as one appliance.
//
// This file changes NOTHING about the sound: it says which of the graph's nodes are one
// box and nothing else. Its plugs and its knobs come from the template it names, which
// lives in the account's library — see apps/k7-daw-wasm/song/docs/assemblies.md.
uses("@k7-core/turntable")
members({
  deck: "deckB",
  platter: "platterB",
  tempo: "tempoB",
  trim: "trimB",
  eq: "eqB",
  filter: "filterB",
  level: "levelB",
  echo: "echoB",
  fade: "fadeB",
  cue: "cueB",
  stemDrums: "stemDrumsB",
  platterDrums: "platterDrumsB",
  stemBass: "stemBassB",
  platterBass: "platterBassB",
  stemOther: "stemOtherB",
  platterOther: "platterOtherB",
  stemVocals: "stemVocalsB",
  platterVocals: "platterVocalsB",
})
"#,
    ),
    (
        "surfaces/desk.k7x",
        r#"// The DESK, and what each of its controls drives.
//
// A surface: a special view the project asks its editor to draw (apps/k7-daw-wasm/song/docs/surfaces.md).
// `split` means a PANE — a slot in the layout, over the graph, the way the arrangement sits
// in a song. Press its number, or fullscreen it, exactly as you would any other pane.
//
// `mapping` is keyed by the CONTROL, because this is about a desk: a control drives at
// most one param, and two targets on one fader is a thing you cannot see going wrong. The
// reverse is allowed — two controls may name the same param, and both move it.
//
// `module` is the name the CHAINS file uses, not a médiathèque ref: a set has two uses
// of the tempo module, and a ref could not tell them apart. That ambiguity is exactly why
// these bindings are not in the module's own `controller.k7x`, which belongs to the
// device and is shared by every use of it — a MIDI CC is a property of the device, a desk
// control is a property of THIS set.
//
// `param` is spelled as the module's ui.k7x binds it, so this file, the panel and a MIDI
// map line up key for key.
//
// The control ids: see apps/k7-daw-frontend/packages/k7-plugins/docs/turntables/controls.md for the list.
//
// `playlist` is what the song browser lists — the twelve songs to hand tonight rather
// than the two hundred and seventy-nine on the volume. A NAME under `_playlists/` at the
// root of the repository, not a path: the folder is fixed. Write the playlists themselves
// in k7, under the cog. See apps/k7-daw-wasm/song/docs/playlists.md.
//
// Left commented because a new set has no set list yet — the desk says so on its browser
// until this line names one, which is a sentence you can act on.
type("turntables")
layout("split")
// playlist("ete-2026")
mapping({
  crossfade: { module: "fadeA", param: "Level", invert: true, also: { module: "fadeB", param: "Level" } },
  "deckA.tempo": { module: "tempoA", param: "Target BPM" },
  "deckB.tempo": { module: "tempoB", param: "Target BPM" },
  "deckA.eqLow": { module: "eqA", param: "Low-shelf gain" },
  "deckA.eqMid": { module: "eqA", param: "Mid gain 1" },
  "deckA.eqHigh": { module: "eqA", param: "High-shelf gain" },
  "deckB.eqLow": { module: "eqB", param: "Low-shelf gain" },
  "deckB.eqMid": { module: "eqB", param: "Mid gain 1" },
  "deckB.eqHigh": { module: "eqB", param: "High-shelf gain" },
  "deckA.filter": { module: "filterA", param: "Sweep" },
  "deckB.filter": { module: "filterB", param: "Sweep" },
  "deckA.cue": { module: "cueA", param: "Level" },
  "deckB.cue": { module: "cueB", param: "Level" },
  "deckA.trim": { module: "trimA", param: "Trim" },
  "deckB.trim": { module: "trimB", param: "Trim" },
  "deckA.level": { module: "levelA", param: "Gain" },
  "deckB.level": { module: "levelB", param: "Gain" },
  "deckA.echo": { module: "echoA", param: "@mix" },
  "deckB.echo": { module: "echoB", param: "@mix" },
  "deckA.stemDrums": { module: "platterDrumsA", param: "Gain" },
  "deckA.stemBass": { module: "platterBassA", param: "Gain" },
  "deckA.stemOther": { module: "platterOtherA", param: "Gain" },
  "deckA.stemVocals": { module: "platterVocalsA", param: "Gain" },
  "deckB.stemDrums": { module: "platterDrumsB", param: "Gain" },
  "deckB.stemBass": { module: "platterBassB", param: "Gain" },
  "deckB.stemOther": { module: "platterOtherB", param: "Gain" },
  "deckB.stemVocals": { module: "platterVocalsB", param: "Gain" },
})
"#,
    ),
];
