// scaffold_set_nodes.rs — the `mix` archetype's node files, as the TypeScript's `setProjectFiles` writes them under `nodes/`.
// Hand-edited Rust, the archetype's source: no generator writes it.

pub const SET_NODES: &[(&str, &str)] = &[
    (
        "nodes/deckA.k7x",
        r#"// Deck A. A lane with no arrangement — it authors no `events` at all.
//
// That is the one thing a SET has that an arranged song does not. In a song a lane's
// file IS its arrangement; on a deck the material is whichever song the browser put on
// ten seconds ago, so what is committed is the deck's SETTINGS and its place in
// the graph. Its run FLOWS on into its own fade, and from there to the mixer.
extends("@meta/track")
"#,
    ),
    (
        "nodes/deckB.k7x",
        r#"// Deck B. A lane with no arrangement — it authors no `events` at all.
//
// That is the one thing a SET has that an arranged song does not. In a song a lane's
// file IS its arrangement; on a deck the material is whichever song the browser put on
// ten seconds ago, so what is committed is the deck's SETTINGS and its place in
// the graph. Its run FLOWS on into its own fade, opposed to deck A's, and from there to the mixer.
extends("@meta/track")
"#,
    ),
    (
        "nodes/fadeA.k7x",
        r#"// Deck A's fade — a NODE of the médiathèque's Fade module, one per deck.
//
// Two of these ARE the crossfade, and they are two rather than one for a reason the
// engine imposes: a module's second port is wired as a SIDECHAIN, and a sidechain reads a
// track only while that track also reaches an audible output. A deck feeding nothing but a
// crossfade would therefore be heard BESIDE it, raw, with no way to take it back — see
// engine/k7-runtime/docs/sidechain.md. One input each, and the blend happens at the mixer.
//
//
// `Level` IS this lane's gain, so the panel reads true on its own: 0 silent, 1 full. The
// desk drives the two in MIRROR — this one takes the control INVERTED, which
// surfaces/desk.k7x states rather than either module knowing about the other.
//
// The module is NOT copied into this project: pl8 owns the only editable catalogue, so a
// copy here would be a second Fade nobody maintains — packages/song/docs/nodes.md.
extends("@k7-core/audio/zz_utility/fade")
params({
  // Centred, like the fader that drives it: both decks at half, which is where a set starts.
  Level: 0.5,
})
"#,
    ),
    (
        "nodes/fadeB.k7x",
        r#"// Deck B's fade — a NODE of the médiathèque's Fade module, one per deck.
//
// Two of these ARE the crossfade, and they are two rather than one for a reason the
// engine imposes: a module's second port is wired as a SIDECHAIN, and a sidechain reads a
// track only while that track also reaches an audible output. A deck feeding nothing but a
// crossfade would therefore be heard BESIDE it, raw, with no way to take it back — see
// engine/k7-runtime/docs/sidechain.md. One input each, and the blend happens at the mixer.
//
//
// `Level` IS this lane's gain, so the panel reads true on its own: 0 silent, 1 full. The
// desk drives the two in MIRROR — this one takes the control as it stands, which
// surfaces/desk.k7x states rather than either module knowing about the other.
//
// The module is NOT copied into this project: pl8 owns the only editable catalogue, so a
// copy here would be a second Fade nobody maintains — packages/song/docs/nodes.md.
extends("@k7-core/audio/zz_utility/fade")
params({
  // Centred, like the fader that drives it: both decks at half, which is where a set starts.
  Level: 0.5,
})
"#,
    ),
    (
        "nodes/cueA.k7x",
        r#"// Deck A's CUE send — what the headphones hear of this deck.
//
// The same Fade module as the crossfade, because the question is the same one: how much of
// this deck goes down this path. `Level` is that amount, and the desk's CUE button is nothing
// more than this param going 0 <-> 1 — no switch anywhere in the engine.
//
// It is fed from `filterA`, which is to say AFTER the EQ and the filter and BEFORE the
// fade. That is what pre-fade listen means: moving the crossfader away from this deck must
// not take it out of your ears, while cutting its bass must be audible before it hits the room.
//
// Where it comes OUT is not this file's business and not the project's: `CUE` is a pair of
// the interface, named once per MACHINE in preferences.yaml. A machine with no
// second pair plays this bus nowhere at all — never into the room, which is measured in
// engine/k7-runtime/scripts/cue-bus.ts. See packages/song/docs/cue.md.
extends("@k7-core/audio/zz_utility/fade")
params({
  // SILENT at rest: a set starts with nothing in the headphones, and the button is what opens it.
  Level: 0,
})
"#,
    ),
    (
        "nodes/cueB.k7x",
        r#"// Deck B's CUE send — what the headphones hear of this deck.
//
// The same Fade module as the crossfade, because the question is the same one: how much of
// this deck goes down this path. `Level` is that amount, and the desk's CUE button is nothing
// more than this param going 0 <-> 1 — no switch anywhere in the engine.
//
// It is fed from `filterB`, which is to say AFTER the EQ and the filter and BEFORE the
// fade. That is what pre-fade listen means: moving the crossfader away from this deck must
// not take it out of your ears, while cutting its bass must be audible before it hits the room.
//
// Where it comes OUT is not this file's business and not the project's: `CUE` is a pair of
// the interface, named once per MACHINE in preferences.yaml. A machine with no
// second pair plays this bus nowhere at all — never into the room, which is measured in
// engine/k7-runtime/scripts/cue-bus.ts. See packages/song/docs/cue.md.
extends("@k7-core/audio/zz_utility/fade")
params({
  // SILENT at rest: a set starts with nothing in the headphones, and the button is what opens it.
  Level: 0,
})
"#,
    ),
    (
        "nodes/tempoA.k7x",
        r#"// Deck A's tempo — a NODE of the médiathèque's Tempo module, one per deck
// because each deck plays at its own BPM and a param belongs to a use, not to a module.
//
// The module is COLD: a tempo change is a time stretch, and a stretch has to know where
// the file ends. That is why it sits on a deck at all — a deck's source is a file on the
// machine's disk, which is the one input whose whole length is knowable up front. See
// packages/song/docs/coldModules.md.
//
// This is what the BPM fader in Mix moves. Both at 0 is a no-op: the record plays at its
// own tempo until the analysis has measured one and somebody has asked for another.
extends("@k7-core/cold/tempo")
params({
  // Measured by the pass that reads the file — the same analysis the waveform uses.
  "Source BPM": 0,
  // What the fader sets. 0 means "leave it alone".
  "Target BPM": 0,
})
"#,
    ),
    (
        "nodes/tempoB.k7x",
        r#"// Deck B's tempo — a NODE of the médiathèque's Tempo module, one per deck
// because each deck plays at its own BPM and a param belongs to a use, not to a module.
//
// The module is COLD: a tempo change is a time stretch, and a stretch has to know where
// the file ends. That is why it sits on a deck at all — a deck's source is a file on the
// machine's disk, which is the one input whose whole length is knowable up front. See
// packages/song/docs/coldModules.md.
//
// This is what the BPM fader in Mix moves. Both at 0 is a no-op: the record plays at its
// own tempo until the analysis has measured one and somebody has asked for another.
extends("@k7-core/cold/tempo")
params({
  // Measured by the pass that reads the file — the same analysis the waveform uses.
  "Source BPM": 0,
  // What the fader sets. 0 means "leave it alone".
  "Target BPM": 0,
})
"#,
    ),
    (
        "nodes/eqA.k7x",
        r#"// Deck A's EQ — a NODE of the médiathèque's EQ, one per deck, because
// cutting THIS deck's bass is the whole point and a param belongs to a use.
//
// Unlike the tempo beside it, this one is a REAL plugin: the module is a name and a set of
// crossover frequencies over Tracktion's 4-band EQ, so the compiler emits a plugin and the
// desk's knobs move it with `set-plugin-param`, live and per pixel. The tempo cannot do
// that — it is offline, and its fader re-sends the document instead.
//
// Flat. Only the three gains are stated: the crossover points come from the module, and
// restating them here would let one deck quietly disagree with the other about what
// "mid" means.
//
// The EQ module has several engines; the desk's is Tracktion's, named so a change of default
// moves no deck (packages/song/docs/engines.md).
extends("@k7-core/audio/eq")
engine("tracktion")
engines({
  tracktion: {
    params: { "Low-shelf gain": 0, "Mid gain 1": 0, "High-shelf gain": 0 },
  },
})
"#,
    ),
    (
        "nodes/eqB.k7x",
        r#"// Deck B's EQ — a NODE of the médiathèque's EQ, one per deck, because
// cutting THIS deck's bass is the whole point and a param belongs to a use.
//
// Unlike the tempo beside it, this one is a REAL plugin: the module is a name and a set of
// crossover frequencies over Tracktion's 4-band EQ, so the compiler emits a plugin and the
// desk's knobs move it with `set-plugin-param`, live and per pixel. The tempo cannot do
// that — it is offline, and its fader re-sends the document instead.
//
// Flat. Only the three gains are stated: the crossover points come from the module, and
// restating them here would let one deck quietly disagree with the other about what
// "mid" means.
//
// The EQ module has several engines; the desk's is Tracktion's, named so a change of default
// moves no deck (packages/song/docs/engines.md).
extends("@k7-core/audio/eq")
engine("tracktion")
engines({
  tracktion: {
    params: { "Low-shelf gain": 0, "Mid gain 1": 0, "High-shelf gain": 0 },
  },
})
"#,
    ),
    (
        "nodes/filterA.k7x",
        r#"// Deck A's filter — a NODE of the médiathèque's filter, on its sweep engine, one per deck.
//
// ONE param, and that is the module's whole shape: a desk control drives at most one, so
// "low-pass or high-pass" cannot be a second setting. Both filters live in its graph at
// once and the knob decides where each sits — which is how the hardware works too.
//
// 0 is the middle of the travel and is INAUDIBLE rather than bypassed: at rest the two
// corners sit outside the material, at 20 kHz and 20 Hz.
//
// The engine is named for the same reason the EQ's is.
extends("@k7-core/audio/filter")
engine("sweep")
engines({ sweep: { params: { Sweep: 0 } } })
"#,
    ),
    (
        "nodes/filterB.k7x",
        r#"// Deck B's filter — a NODE of the médiathèque's filter, on its sweep engine, one per deck.
//
// ONE param, and that is the module's whole shape: a desk control drives at most one, so
// "low-pass or high-pass" cannot be a second setting. Both filters live in its graph at
// once and the knob decides where each sits — which is how the hardware works too.
//
// 0 is the middle of the travel and is INAUDIBLE rather than bypassed: at rest the two
// corners sit outside the material, at 20 kHz and 20 Hz.
//
// The engine is named for the same reason the EQ's is.
extends("@k7-core/audio/filter")
engine("sweep")
engines({ sweep: { params: { Sweep: 0 } } })
"#,
    ),
    (
        "nodes/trimA.k7x",
        r#"// Deck A's TRIM — the input gain, and the first module of this deck's strip.
//
// First for a reason that is not taste: a deck is FIVE lanes — the record, and one per
// separated stem — and they all meet HERE. A trim on the deck's own lane would do nothing
// at all to a separated record, whose four parts play instead of the disc.
//
// Being the head, it also NAMES the chain: every plugin on this strip is filed under
// `trimA`, which is why `bus_only` lists this node and not the EQ.
//
// What it is for: two records cut ten years apart do not arrive at the same level, and
// nothing in this project normalises loudness. This is what puts them on the same footing
// so that 0 dB on either channel fader means the same thing.
extends("@k7-core/audio/zz_utility/trim")
params({
  // Unity, in the MIDDLE of the travel: the rest of a trim is "whatever the record gives".
  Trim: 0,
})
"#,
    ),
    (
        "nodes/levelA.k7x",
        r#"// Deck A's LEVEL — the channel fader, in decibels.
//
// Not the crossfader, and this is the distinction the desk was missing: most of a
// transition is made at the channel, the crossfader being the thing you use for a hard cut.
// A desk whose only level control is the crossfade can blend two records and nothing else.
//
// In DECIBELS, unlike the `fade` it feeds: a LEVEL is read in dB, and a surface sending
// 0.5 for "half" would look right and be six decibels wrong. The floor is -60 and reads as
// silence — a fader has a bottom, and -Infinity is not a position on one.
//
// It sits AFTER the cue tap, which is what pre-fade listen means: pulling this fader down
// must not take the deck out of your headphones.
extends("@k7-core/audio/zz_utility/gain")
params({
  // 0 dB = unity. A channel fader rests OPEN; it is the crossfader that decides what is heard.
  Gain: 0,
})
"#,
    ),
    (
        "nodes/echoA.k7x",
        r#"// Deck A's ECHO — a feedback delay, the only effect on this strip past the filter.
//
// AFTER the level, and that placement is the entire feature: an echo-out is cutting the
// channel and letting the repeats ring on, which works because what you cut is the delay's
// INPUT and not its output. Wired before the fader it would be cut along with everything else.
//
// `mix` is what the desk's knob moves — the ENGINE's dry/wet around the delay, whose own is
// held at full wet (engine/k7-runtime/docs/mix.md) — and it rests at 0: an effect you drop
// into a chain must not be heard before somebody asks for it.
//
// `Time` is in MILLISECONDS and follows nothing: the desk knows each deck's BPM, but no
// binding carries it here yet. See packages/k7-plugins/docs/turntables/fx.md.
// The delay's ECHO engine: the one of them whose Time is in milliseconds.
extends("@k7-core/audio/delay")
engine("echo")
engines({
  echo: {
    params: {
      // A dotted eighth at 120 BPM — the dub delay that displaces rather than doubles.
      Time: 375,
      // Three or four audible repeats: enough to hear, little enough not to pile up.
      Feedback: 0.45,
    },
  },
})
// DRY at rest. This is the one the desk drives.
mix(0)
"#,
    ),
    (
        "nodes/trimB.k7x",
        r#"// Deck B's TRIM — the input gain, and the first module of this deck's strip.
//
// First for a reason that is not taste: a deck is FIVE lanes — the record, and one per
// separated stem — and they all meet HERE. A trim on the deck's own lane would do nothing
// at all to a separated record, whose four parts play instead of the disc.
//
// Being the head, it also NAMES the chain: every plugin on this strip is filed under
// `trimB`, which is why `bus_only` lists this node and not the EQ.
//
// What it is for: two records cut ten years apart do not arrive at the same level, and
// nothing in this project normalises loudness. This is what puts them on the same footing
// so that 0 dB on either channel fader means the same thing.
extends("@k7-core/audio/zz_utility/trim")
params({
  // Unity, in the MIDDLE of the travel: the rest of a trim is "whatever the record gives".
  Trim: 0,
})
"#,
    ),
    (
        "nodes/levelB.k7x",
        r#"// Deck B's LEVEL — the channel fader, in decibels.
//
// Not the crossfader, and this is the distinction the desk was missing: most of a
// transition is made at the channel, the crossfader being the thing you use for a hard cut.
// A desk whose only level control is the crossfade can blend two records and nothing else.
//
// In DECIBELS, unlike the `fade` it feeds: a LEVEL is read in dB, and a surface sending
// 0.5 for "half" would look right and be six decibels wrong. The floor is -60 and reads as
// silence — a fader has a bottom, and -Infinity is not a position on one.
//
// It sits AFTER the cue tap, which is what pre-fade listen means: pulling this fader down
// must not take the deck out of your headphones.
extends("@k7-core/audio/zz_utility/gain")
params({
  // 0 dB = unity. A channel fader rests OPEN; it is the crossfader that decides what is heard.
  Gain: 0,
})
"#,
    ),
    (
        "nodes/echoB.k7x",
        r#"// Deck B's ECHO — a feedback delay, the only effect on this strip past the filter.
//
// AFTER the level, and that placement is the entire feature: an echo-out is cutting the
// channel and letting the repeats ring on, which works because what you cut is the delay's
// INPUT and not its output. Wired before the fader it would be cut along with everything else.
//
// `mix` is what the desk's knob moves — the ENGINE's dry/wet around the delay, whose own is
// held at full wet (engine/k7-runtime/docs/mix.md) — and it rests at 0: an effect you drop
// into a chain must not be heard before somebody asks for it.
//
// `Time` is in MILLISECONDS and follows nothing: the desk knows each deck's BPM, but no
// binding carries it here yet. See packages/k7-plugins/docs/turntables/fx.md.
// The delay's ECHO engine: the one of them whose Time is in milliseconds.
extends("@k7-core/audio/delay")
engine("echo")
engines({
  echo: {
    params: {
      // A dotted eighth at 120 BPM — the dub delay that displaces rather than doubles.
      Time: 375,
      // Three or four audible repeats: enough to hear, little enough not to pile up.
      Feedback: 0.45,
    },
  },
})
// DRY at rest. This is the one the desk drives.
mix(0)
"#,
    ),
];
