// scaffold_set_sources.rs — the `mix` archetype's platter and stem nodes, as the TypeScript's `setProjectFiles` writes them under `nodes/`.
// Hand-edited Rust, the archetype's source: no generator writes it.

pub const SET_SOURCES: &[(&str, &str)] = &[
    (
        "nodes/platterA.k7x",
        r#"// Le plateau de la platine A — le disque lui-même.
//
// C'est la SOURCE de cette lane : elle ne prend pas d'entrée et fabrique son son toute seule.
// Avant, elle n'était déclarée nulle part — le pupitre injectait ce plugin dans le document au
// moment de l'envoi, donc la vue chaînes ne montrait rien avant le tempo.
//
// LE DISQUE N'EST PAS ICI, et c'est délibéré. Il est choisi en direct, il change dix fois par
// soirée, et l'écrire dans le projet reviendrait à commiter ce qu'on jouait ce soir-là. Le
// pupitre le pose sur le champ `deckFile` du document, en même temps que la vitesse.
//
// `Rate: 0` est donc l'état au repos : un plateau qu'on ouvre est à l'arrêt.
extends("@meta/audio/turntable")
"#,
    ),
    (
        "nodes/platterDrumsA.k7x",
        r#"// Le plateau de la platine A — la piste « drums » du disque séparé.
//
// C'est la SOURCE de cette lane : elle ne prend pas d'entrée et fabrique son son toute seule.
// Avant, elle n'était déclarée nulle part — le pupitre injectait ce plugin dans le document au
// moment de l'envoi, donc la vue chaînes ne montrait rien avant le tempo.
//
// LE DISQUE N'EST PAS ICI, et c'est délibéré. Il est choisi en direct, il change dix fois par
// soirée, et l'écrire dans le projet reviendrait à commiter ce qu'on jouait ce soir-là. Le
// pupitre le pose sur le champ `deckFile` du document, en même temps que la vitesse.
//
// `Rate: 0` est donc l'état au repos : un plateau qu'on ouvre est à l'arrêt.
extends("@meta/audio/turntable")
params({
  // 0 drops the instrument, 1 is as it was cut, 2 doubles it. The desk's drums knob
  // moves this, and it starts CENTRED on unity — left removes, right adds.
  "Gain": 1,
})
"#,
    ),
    (
        "nodes/platterBassA.k7x",
        r#"// Le plateau de la platine A — la piste « bass » du disque séparé.
//
// C'est la SOURCE de cette lane : elle ne prend pas d'entrée et fabrique son son toute seule.
// Avant, elle n'était déclarée nulle part — le pupitre injectait ce plugin dans le document au
// moment de l'envoi, donc la vue chaînes ne montrait rien avant le tempo.
//
// LE DISQUE N'EST PAS ICI, et c'est délibéré. Il est choisi en direct, il change dix fois par
// soirée, et l'écrire dans le projet reviendrait à commiter ce qu'on jouait ce soir-là. Le
// pupitre le pose sur le champ `deckFile` du document, en même temps que la vitesse.
//
// `Rate: 0` est donc l'état au repos : un plateau qu'on ouvre est à l'arrêt.
extends("@meta/audio/turntable")
params({
  // 0 drops the instrument, 1 is as it was cut, 2 doubles it. The desk's bass knob
  // moves this, and it starts CENTRED on unity — left removes, right adds.
  "Gain": 1,
})
"#,
    ),
    (
        "nodes/platterOtherA.k7x",
        r#"// Le plateau de la platine A — la piste « other » du disque séparé.
//
// C'est la SOURCE de cette lane : elle ne prend pas d'entrée et fabrique son son toute seule.
// Avant, elle n'était déclarée nulle part — le pupitre injectait ce plugin dans le document au
// moment de l'envoi, donc la vue chaînes ne montrait rien avant le tempo.
//
// LE DISQUE N'EST PAS ICI, et c'est délibéré. Il est choisi en direct, il change dix fois par
// soirée, et l'écrire dans le projet reviendrait à commiter ce qu'on jouait ce soir-là. Le
// pupitre le pose sur le champ `deckFile` du document, en même temps que la vitesse.
//
// `Rate: 0` est donc l'état au repos : un plateau qu'on ouvre est à l'arrêt.
extends("@meta/audio/turntable")
params({
  // 0 drops the instrument, 1 is as it was cut, 2 doubles it. The desk's other knob
  // moves this, and it starts CENTRED on unity — left removes, right adds.
  "Gain": 1,
})
"#,
    ),
    (
        "nodes/platterVocalsA.k7x",
        r#"// Le plateau de la platine A — la piste « vocals » du disque séparé.
//
// C'est la SOURCE de cette lane : elle ne prend pas d'entrée et fabrique son son toute seule.
// Avant, elle n'était déclarée nulle part — le pupitre injectait ce plugin dans le document au
// moment de l'envoi, donc la vue chaînes ne montrait rien avant le tempo.
//
// LE DISQUE N'EST PAS ICI, et c'est délibéré. Il est choisi en direct, il change dix fois par
// soirée, et l'écrire dans le projet reviendrait à commiter ce qu'on jouait ce soir-là. Le
// pupitre le pose sur le champ `deckFile` du document, en même temps que la vitesse.
//
// `Rate: 0` est donc l'état au repos : un plateau qu'on ouvre est à l'arrêt.
extends("@meta/audio/turntable")
params({
  // 0 drops the instrument, 1 is as it was cut, 2 doubles it. The desk's vocals knob
  // moves this, and it starts CENTRED on unity — left removes, right adds.
  "Gain": 1,
})
"#,
    ),
    (
        "nodes/platterB.k7x",
        r#"// Le plateau de la platine B — le disque lui-même.
//
// C'est la SOURCE de cette lane : elle ne prend pas d'entrée et fabrique son son toute seule.
// Avant, elle n'était déclarée nulle part — le pupitre injectait ce plugin dans le document au
// moment de l'envoi, donc la vue chaînes ne montrait rien avant le tempo.
//
// LE DISQUE N'EST PAS ICI, et c'est délibéré. Il est choisi en direct, il change dix fois par
// soirée, et l'écrire dans le projet reviendrait à commiter ce qu'on jouait ce soir-là. Le
// pupitre le pose sur le champ `deckFile` du document, en même temps que la vitesse.
//
// `Rate: 0` est donc l'état au repos : un plateau qu'on ouvre est à l'arrêt.
extends("@meta/audio/turntable")
"#,
    ),
    (
        "nodes/platterDrumsB.k7x",
        r#"// Le plateau de la platine B — la piste « drums » du disque séparé.
//
// C'est la SOURCE de cette lane : elle ne prend pas d'entrée et fabrique son son toute seule.
// Avant, elle n'était déclarée nulle part — le pupitre injectait ce plugin dans le document au
// moment de l'envoi, donc la vue chaînes ne montrait rien avant le tempo.
//
// LE DISQUE N'EST PAS ICI, et c'est délibéré. Il est choisi en direct, il change dix fois par
// soirée, et l'écrire dans le projet reviendrait à commiter ce qu'on jouait ce soir-là. Le
// pupitre le pose sur le champ `deckFile` du document, en même temps que la vitesse.
//
// `Rate: 0` est donc l'état au repos : un plateau qu'on ouvre est à l'arrêt.
extends("@meta/audio/turntable")
params({
  // 0 drops the instrument, 1 is as it was cut, 2 doubles it. The desk's drums knob
  // moves this, and it starts CENTRED on unity — left removes, right adds.
  "Gain": 1,
})
"#,
    ),
    (
        "nodes/platterBassB.k7x",
        r#"// Le plateau de la platine B — la piste « bass » du disque séparé.
//
// C'est la SOURCE de cette lane : elle ne prend pas d'entrée et fabrique son son toute seule.
// Avant, elle n'était déclarée nulle part — le pupitre injectait ce plugin dans le document au
// moment de l'envoi, donc la vue chaînes ne montrait rien avant le tempo.
//
// LE DISQUE N'EST PAS ICI, et c'est délibéré. Il est choisi en direct, il change dix fois par
// soirée, et l'écrire dans le projet reviendrait à commiter ce qu'on jouait ce soir-là. Le
// pupitre le pose sur le champ `deckFile` du document, en même temps que la vitesse.
//
// `Rate: 0` est donc l'état au repos : un plateau qu'on ouvre est à l'arrêt.
extends("@meta/audio/turntable")
params({
  // 0 drops the instrument, 1 is as it was cut, 2 doubles it. The desk's bass knob
  // moves this, and it starts CENTRED on unity — left removes, right adds.
  "Gain": 1,
})
"#,
    ),
    (
        "nodes/platterOtherB.k7x",
        r#"// Le plateau de la platine B — la piste « other » du disque séparé.
//
// C'est la SOURCE de cette lane : elle ne prend pas d'entrée et fabrique son son toute seule.
// Avant, elle n'était déclarée nulle part — le pupitre injectait ce plugin dans le document au
// moment de l'envoi, donc la vue chaînes ne montrait rien avant le tempo.
//
// LE DISQUE N'EST PAS ICI, et c'est délibéré. Il est choisi en direct, il change dix fois par
// soirée, et l'écrire dans le projet reviendrait à commiter ce qu'on jouait ce soir-là. Le
// pupitre le pose sur le champ `deckFile` du document, en même temps que la vitesse.
//
// `Rate: 0` est donc l'état au repos : un plateau qu'on ouvre est à l'arrêt.
extends("@meta/audio/turntable")
params({
  // 0 drops the instrument, 1 is as it was cut, 2 doubles it. The desk's other knob
  // moves this, and it starts CENTRED on unity — left removes, right adds.
  "Gain": 1,
})
"#,
    ),
    (
        "nodes/platterVocalsB.k7x",
        r#"// Le plateau de la platine B — la piste « vocals » du disque séparé.
//
// C'est la SOURCE de cette lane : elle ne prend pas d'entrée et fabrique son son toute seule.
// Avant, elle n'était déclarée nulle part — le pupitre injectait ce plugin dans le document au
// moment de l'envoi, donc la vue chaînes ne montrait rien avant le tempo.
//
// LE DISQUE N'EST PAS ICI, et c'est délibéré. Il est choisi en direct, il change dix fois par
// soirée, et l'écrire dans le projet reviendrait à commiter ce qu'on jouait ce soir-là. Le
// pupitre le pose sur le champ `deckFile` du document, en même temps que la vitesse.
//
// `Rate: 0` est donc l'état au repos : un plateau qu'on ouvre est à l'arrêt.
extends("@meta/audio/turntable")
params({
  // 0 drops the instrument, 1 is as it was cut, 2 doubles it. The desk's vocals knob
  // moves this, and it starts CENTRED on unity — left removes, right adds.
  "Gain": 1,
})
"#,
    ),
    (
        "nodes/stemDrumsA.k7x",
        r#"// Deck A's drums — the lane that plays one of the separated files.
//
// A lane with no arrangement, exactly like the deck beside it: the material arrives when a
// song is put on, and it is `stems/drums.wav` in that song's own folder. A song
// that has never been separated leaves this lane empty, and the deck's own lane plays instead
// — which is why a deck still plays a record nobody prepared.
//
// Muted in graph.k7x, like every source lane here: it is heard through the bus only, where
// it is summed with the three beside it — an unmuted lane would reach the master twice.
extends("@meta/track")
"#,
    ),
    (
        "nodes/stemBassA.k7x",
        r#"// Deck A's bass — the lane that plays one of the separated files.
//
// A lane with no arrangement, exactly like the deck beside it: the material arrives when a
// song is put on, and it is `stems/bass.wav` in that song's own folder. A song
// that has never been separated leaves this lane empty, and the deck's own lane plays instead
// — which is why a deck still plays a record nobody prepared.
//
// Muted in graph.k7x, like every source lane here: it is heard through the bus only, where
// it is summed with the three beside it — an unmuted lane would reach the master twice.
extends("@meta/track")
"#,
    ),
    (
        "nodes/stemOtherA.k7x",
        r#"// Deck A's other — the lane that plays one of the separated files.
//
// A lane with no arrangement, exactly like the deck beside it: the material arrives when a
// song is put on, and it is `stems/other.wav` in that song's own folder. A song
// that has never been separated leaves this lane empty, and the deck's own lane plays instead
// — which is why a deck still plays a record nobody prepared.
//
// Muted in graph.k7x, like every source lane here: it is heard through the bus only, where
// it is summed with the three beside it — an unmuted lane would reach the master twice.
extends("@meta/track")
"#,
    ),
    (
        "nodes/stemVocalsA.k7x",
        r#"// Deck A's vocals — the lane that plays one of the separated files.
//
// A lane with no arrangement, exactly like the deck beside it: the material arrives when a
// song is put on, and it is `stems/vocals.wav` in that song's own folder. A song
// that has never been separated leaves this lane empty, and the deck's own lane plays instead
// — which is why a deck still plays a record nobody prepared.
//
// Muted in graph.k7x, like every source lane here: it is heard through the bus only, where
// it is summed with the three beside it — an unmuted lane would reach the master twice.
extends("@meta/track")
"#,
    ),
    (
        "nodes/stemDrumsB.k7x",
        r#"// Deck B's drums — the lane that plays one of the separated files.
//
// A lane with no arrangement, exactly like the deck beside it: the material arrives when a
// song is put on, and it is `stems/drums.wav` in that song's own folder. A song
// that has never been separated leaves this lane empty, and the deck's own lane plays instead
// — which is why a deck still plays a record nobody prepared.
//
// Muted in graph.k7x, like every source lane here: it is heard through the bus only, where
// it is summed with the three beside it — an unmuted lane would reach the master twice.
extends("@meta/track")
"#,
    ),
    (
        "nodes/stemBassB.k7x",
        r#"// Deck B's bass — the lane that plays one of the separated files.
//
// A lane with no arrangement, exactly like the deck beside it: the material arrives when a
// song is put on, and it is `stems/bass.wav` in that song's own folder. A song
// that has never been separated leaves this lane empty, and the deck's own lane plays instead
// — which is why a deck still plays a record nobody prepared.
//
// Muted in graph.k7x, like every source lane here: it is heard through the bus only, where
// it is summed with the three beside it — an unmuted lane would reach the master twice.
extends("@meta/track")
"#,
    ),
    (
        "nodes/stemOtherB.k7x",
        r#"// Deck B's other — the lane that plays one of the separated files.
//
// A lane with no arrangement, exactly like the deck beside it: the material arrives when a
// song is put on, and it is `stems/other.wav` in that song's own folder. A song
// that has never been separated leaves this lane empty, and the deck's own lane plays instead
// — which is why a deck still plays a record nobody prepared.
//
// Muted in graph.k7x, like every source lane here: it is heard through the bus only, where
// it is summed with the three beside it — an unmuted lane would reach the master twice.
extends("@meta/track")
"#,
    ),
    (
        "nodes/stemVocalsB.k7x",
        r#"// Deck B's vocals — the lane that plays one of the separated files.
//
// A lane with no arrangement, exactly like the deck beside it: the material arrives when a
// song is put on, and it is `stems/vocals.wav` in that song's own folder. A song
// that has never been separated leaves this lane empty, and the deck's own lane plays instead
// — which is why a deck still plays a record nobody prepared.
//
// Muted in graph.k7x, like every source lane here: it is heard through the bus only, where
// it is summed with the three beside it — an unmuted lane would reach the master twice.
extends("@meta/track")
"#,
    ),
];
