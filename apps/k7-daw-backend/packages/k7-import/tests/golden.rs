// golden.rs — one song taking every branch of the emitter, its whole output held byte for byte to files written by hand.
// What a reader gets wrong: tests/fixtures/golden is the expected output, not an input; a change to a file there is a change to what an import writes.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use k7_import::model::{ClipKind, ClipOut, NoteOut, PluginOut, SendOut, Song, TrackOut, WarpOut};
use k7_import::{World, emit};

fn plug(name: &str, role: &str) -> PluginOut {
    PluginOut {
        kind: "vst3".into(),
        name: name.into(),
        role: Some(role.into()),
        enabled: true,
        path: None,
        params: Vec::new(),
        params_complete: false,
    }
}

fn note(time: f64, duration: f64, key: f64, vel: f64) -> NoteOut {
    NoteOut { time, duration, key, vel }
}

fn track(id: &str, name: &str, role: &str) -> TrackOut {
    TrackOut {
        id: id.into(),
        name: name.into(),
        role: Some(role.into()),
        ..TrackOut::default()
    }
}

fn song() -> Song {
    let mut drums = track("t1", "Drums", "regular");
    drums.color = Some("#ff0000".into());
    drums.drums = true;
    let mut c = ClipOut::new(ClipKind::Midi, 0.0, 4.0);
    c.notes = vec![note(0.0, 0.5, 36.0, 1.0), note(1.0, 0.5, 38.0, 0.6)];
    drums.clips = vec![c];

    let mut keys = track("t2", "Keys", "regular");
    let mut eq = plug("Pro-Q 3", "audioFX");
    eq.params = vec![("Gain".into(), 0.5)];
    eq.params_complete = true;
    let mut diva = plug("Diva", "instrument");
    diva.params = vec![("Cutoff".into(), 0.3)];
    diva.params_complete = true;
    let mut arp = plug("Arp", "noteFX");
    arp.enabled = false;
    keys.plugins = vec![eq, diva, arp];
    keys.sends = vec![SendOut { to: "t5".into(), level: 0.5, enabled: true }];
    let mut c = ClipOut::new(ClipKind::Midi, 8.0, 8.0);
    c.loop_start = Some(0.0);
    c.loop_end = Some(4.0);
    c.disabled = true;
    c.notes = vec![note(0.0, 1.0, 60.0, 1.0)];
    keys.clips = vec![c];

    let mut bass = track("t3", "Bass", "regular");
    bass.mute = true;
    let mut eq = plug("Pro-Q 3", "audioFX");
    eq.path = Some("plugins/bassq.vstpreset".into());
    eq.params = vec![("Gain".into(), 0.1)];
    bass.plugins = vec![eq, plug("Diva", "instrument")];
    let audio = |source: &str, start: f64, length: f64| {
        let mut c = ClipOut::new(ClipKind::Audio, start, length);
        c.source = Some(source.into());
        c
    };
    let mut a = audio("audio/bass.wav", 0.0, 4.0);
    a.fade_in = Some(0.25);
    let mut b = audio("audio/bass copy.wav", 4.0, 4.0);
    b.offset = Some(1.0);
    b.fade_out = Some(0.5);
    b.warp = Some(WarpOut {
        algorithm: Some("complex".into()),
        markers: vec![(0.0, 0.0), (4.0, 2.1)],
    });
    bass.clips = vec![a, b, audio("audio/lost.wav", 8.0, 2.0)];

    let mut vox = track("t4", "Vox", "regular");
    let mut kontakt = plug("Kontakt 7", "audioFX");
    kontakt.params = vec![("Mix".into(), 0.5)];
    kontakt.params_complete = true;
    vox.plugins = vec![kontakt];
    vox.sends = vec![
        SendOut { to: "t5".into(), level: 1.0, enabled: true },
        SendOut { to: "t6".into(), level: 0.3, enabled: false },
    ];
    vox.clips = vec![audio("audio/vox.wav", 2.0, 2.0)];

    let mut verb = track("t5", "Verb", "effect");
    verb.plugins = vec![plug("ValhallaRoom", "audioFX")];
    let mut dead = track("t6", "Dead", "effect");
    dead.plugins = vec![plug("ValhallaRoom", "audioFX")];
    let mut master = track("t7", "Master", "master");
    master.plugins = vec![plug("Pro-L 2", "audioFX")];

    Song {
        name: "golden".into(),
        tempo: 98.0,
        sample_rate: 48000.0,
        time_signature: "7/8".into(),
        tracks: vec![drums, keys, bass, vox, verb, dead, master],
    }
}

fn expected() -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                let rel = p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
                out.insert(rel, std::fs::read(&p).unwrap());
            }
        }
    }
    let root: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/golden");
    let mut out = BTreeMap::new();
    walk(&root, &root, &mut out);
    out
}

#[test]
fn a_song_taking_every_branch_writes_the_files_it_always_wrote() {
    let bass = Arc::new(b"RIFFbass".to_vec());
    let assets = vec![
        ("audio/bass.wav".to_owned(), bass.clone()),
        ("audio/bass copy.wav".to_owned(), bass),
        ("audio/vox.wav".to_owned(), Arc::new(b"RIFFvox".to_vec())),
        ("plugins/bassq.vstpreset".to_owned(), Arc::new(b"VST3preset".to_vec())),
    ];
    let names = ["Pro-Q 3", "Diva", "Arp", "ValhallaRoom", "Pro-L 2"];
    let world = World {
        taken_modules: HashSet::new(),
        plugins: Some(names.iter().map(|n| n.to_lowercase()).collect()),
    };
    let (written, stats) = emit("song.k7", &song(), &assets, &world);

    let mut got: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for (p, t) in &written.texts {
        assert!(got.insert(p.clone(), t.clone().into_bytes()).is_none(), "{p} written twice");
    }
    for (p, b) in &written.binaries {
        assert!(got.insert(p.clone(), b.to_vec()).is_none(), "{p} written twice");
    }
    let want = expected();
    assert_eq!(got.keys().collect::<Vec<_>>(), want.keys().collect::<Vec<_>>());
    for (path, bytes) in &want {
        assert_eq!(
            String::from_utf8_lossy(&got[path]),
            String::from_utf8_lossy(bytes),
            "{path}"
        );
    }

    assert_eq!(stats.track_files, 4);
    assert_eq!(stats.clip_files, 4);
    assert_eq!(stats.sequence_files, 2);
    assert_eq!(stats.chain_files, 6);
    assert_eq!(stats.module_nodes, 5);
    assert_eq!(stats.node_files, 3);
    assert_eq!(stats.module_preset_files, 1);
    assert_eq!(stats.stand_ins, 1);
    assert_eq!(stats.missing_audio, ["bass_2"]);
    assert_eq!(stats.missing_plugins, ["Kontakt 7"]);
}
