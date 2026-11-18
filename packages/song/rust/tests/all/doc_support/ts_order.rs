// doc_support/ts_order.rs — the TypeScript's orderTracks answers for fixed name lists, read off song-schema's trackKind.ts.
// A reader would get wrong that these are expectations, not a snapshot: change one only to follow the TypeScript.

/// Names, the stated order, and the names in the order the TypeScript gave.
pub type OrderCase = (
    &'static [&'static str],
    Option<&'static [&'static str]>,
    &'static [&'static str],
);

#[rustfmt::skip]
pub const ORDERS: &[OrderCase] = &[
    (&["pad", "3.vo.lead", "kick", "bass", "gt.riff", "vox", "ge.solo", "2.piano", "organ", "zzz", "1.x", "1.y", "99.last", "0.zero", "da.hats"], None, &["1.x", "1.y", "0.zero", "2.piano", "3.vo.lead", "vox", "kick", "da.hats", "bass", "gt.riff", "ge.solo", "organ", "pad", "zzz", "99.last"]),
    (&["pad", "3.vo.lead", "kick", "bass", "gt.riff", "vox", "ge.solo", "2.piano", "organ", "zzz", "1.x", "1.y", "99.last", "0.zero", "da.hats"], Some(&[]), &["1.x", "1.y", "0.zero", "2.piano", "3.vo.lead", "vox", "kick", "da.hats", "bass", "gt.riff", "ge.solo", "organ", "pad", "zzz", "99.last"]),
    (&["pad", "3.vo.lead", "kick", "bass", "gt.riff", "vox", "ge.solo", "2.piano", "organ", "zzz", "1.x", "1.y", "99.last", "0.zero", "da.hats"], Some(&["zzz", "kick", "pad"]), &["zzz", "kick", "pad", "1.x", "1.y", "0.zero", "2.piano", "3.vo.lead", "vox", "da.hats", "bass", "gt.riff", "ge.solo", "organ", "99.last"]),
    (&["pad", "3.vo.lead", "kick", "bass", "gt.riff", "vox", "ge.solo", "2.piano", "organ", "zzz", "1.x", "1.y", "99.last", "0.zero", "da.hats"], Some(&["b", "a", "b"]), &["1.x", "1.y", "0.zero", "2.piano", "3.vo.lead", "vox", "kick", "da.hats", "bass", "gt.riff", "ge.solo", "organ", "pad", "zzz", "99.last"]),
    (&["pad", "3.vo.lead", "kick", "bass", "gt.riff", "vox", "ge.solo", "2.piano", "organ", "zzz", "1.x", "1.y", "99.last", "0.zero", "da.hats"], Some(&["x", "x", "3.x", "..", "kick"]), &["kick", "1.x", "1.y", "0.zero", "2.piano", "3.vo.lead", "vox", "da.hats", "bass", "gt.riff", "ge.solo", "organ", "pad", "zzz", "99.last"]),
    (&["1.a", "1.b", "1.c", "2.d", "2.e", "5.f"], None, &["1.a", "1.b", "1.c", "2.d", "2.e", "5.f"]),
    (&["1.a", "1.b", "1.c", "2.d", "2.e", "5.f"], Some(&[]), &["1.a", "1.b", "1.c", "2.d", "2.e", "5.f"]),
    (&["1.a", "1.b", "1.c", "2.d", "2.e", "5.f"], Some(&["zzz", "kick", "pad"]), &["1.a", "1.b", "1.c", "2.d", "2.e", "5.f"]),
    (&["1.a", "1.b", "1.c", "2.d", "2.e", "5.f"], Some(&["b", "a", "b"]), &["1.a", "1.b", "1.c", "2.d", "2.e", "5.f"]),
    (&["1.a", "1.b", "1.c", "2.d", "2.e", "5.f"], Some(&["x", "x", "3.x", "..", "kick"]), &["1.a", "1.b", "1.c", "2.d", "2.e", "5.f"]),
    (&["Basse", "BATTERIE", "Choeur", "CH\u{152}UR", "synth lead", "Hi-Hat", "hihat", "808", "sub", "tele", "FOLK", "Wurli", "EP", "ep2", "mu-synth"], None, &["Choeur", "BATTERIE", "Hi-Hat", "hihat", "808", "Basse", "sub", "tele", "FOLK", "Wurli", "EP", "synth lead", "mu-synth", "CH\u{152}UR", "ep2"]),
    (&["Basse", "BATTERIE", "Choeur", "CH\u{152}UR", "synth lead", "Hi-Hat", "hihat", "808", "sub", "tele", "FOLK", "Wurli", "EP", "ep2", "mu-synth"], Some(&[]), &["Choeur", "BATTERIE", "Hi-Hat", "hihat", "808", "Basse", "sub", "tele", "FOLK", "Wurli", "EP", "synth lead", "mu-synth", "CH\u{152}UR", "ep2"]),
    (&["Basse", "BATTERIE", "Choeur", "CH\u{152}UR", "synth lead", "Hi-Hat", "hihat", "808", "sub", "tele", "FOLK", "Wurli", "EP", "ep2", "mu-synth"], Some(&["zzz", "kick", "pad"]), &["Choeur", "BATTERIE", "Hi-Hat", "hihat", "808", "Basse", "sub", "tele", "FOLK", "Wurli", "EP", "synth lead", "mu-synth", "CH\u{152}UR", "ep2"]),
    (&["Basse", "BATTERIE", "Choeur", "CH\u{152}UR", "synth lead", "Hi-Hat", "hihat", "808", "sub", "tele", "FOLK", "Wurli", "EP", "ep2", "mu-synth"], Some(&["b", "a", "b"]), &["Choeur", "BATTERIE", "Hi-Hat", "hihat", "808", "Basse", "sub", "tele", "FOLK", "Wurli", "EP", "synth lead", "mu-synth", "CH\u{152}UR", "ep2"]),
    (&["Basse", "BATTERIE", "Choeur", "CH\u{152}UR", "synth lead", "Hi-Hat", "hihat", "808", "sub", "tele", "FOLK", "Wurli", "EP", "ep2", "mu-synth"], Some(&["x", "x", "3.x", "..", "kick"]), &["Choeur", "BATTERIE", "Hi-Hat", "hihat", "808", "Basse", "sub", "tele", "FOLK", "Wurli", "EP", "synth lead", "mu-synth", "CH\u{152}UR", "ep2"]),
    (&["x", "9999999999999999999999.a", "3.x", ".x", "x.", "..", "0.1.y", "vo.", "VO.A", "1.\u{a}z"], None, &["0.1.y", "vo.", "3.x", "VO.A", "x", ".x", "x.", "..", "1.\u{a}z", "9999999999999999999999.a"]),
    (&["x", "9999999999999999999999.a", "3.x", ".x", "x.", "..", "0.1.y", "vo.", "VO.A", "1.\u{a}z"], Some(&[]), &["0.1.y", "vo.", "3.x", "VO.A", "x", ".x", "x.", "..", "1.\u{a}z", "9999999999999999999999.a"]),
    (&["x", "9999999999999999999999.a", "3.x", ".x", "x.", "..", "0.1.y", "vo.", "VO.A", "1.\u{a}z"], Some(&["zzz", "kick", "pad"]), &["0.1.y", "vo.", "3.x", "VO.A", "x", ".x", "x.", "..", "1.\u{a}z", "9999999999999999999999.a"]),
    (&["x", "9999999999999999999999.a", "3.x", ".x", "x.", "..", "0.1.y", "vo.", "VO.A", "1.\u{a}z"], Some(&["b", "a", "b"]), &["0.1.y", "vo.", "3.x", "VO.A", "x", ".x", "x.", "..", "1.\u{a}z", "9999999999999999999999.a"]),
    (&["x", "9999999999999999999999.a", "3.x", ".x", "x.", "..", "0.1.y", "vo.", "VO.A", "1.\u{a}z"], Some(&["x", "x", "3.x", "..", "kick"]), &["x", "3.x", "..", "0.1.y", "vo.", "VO.A", ".x", "x.", "1.\u{a}z", "9999999999999999999999.a"]),
];
