// import.rs — a foreign project translated and emitted, then read back as files and compiled by the song crate, as the page would.
// What a reader gets wrong: the archives are built here from the XML under test, a zip in git is a binary nobody can read a diff of.

mod support;

use std::collections::{HashMap, HashSet};

use k7_import::{
    Bytes, Imported, Input, MAX_IMPORT_BYTES, SampleRef, World, check_droppable, check_size,
    emit_translation, translate,
};
use k7x::edit::Value;
use support::{gz, land, wav, zip_of};

fn world() -> World {
    World {
        taken_modules: HashSet::new(),
        plugins: None,
    }
}

fn input<'a>(file: &'a str, bytes: &'a [u8]) -> Input<'a> {
    Input {
        source: file,
        file,
        bytes,
        read_sample: None,
        inflate_limit: None,
    }
}

fn run(file: &str, bytes: &[u8], project: &str) -> Imported {
    let t = translate(&input(file, bytes)).unwrap_or_else(|e| panic!("{e}"));
    emit_translation(file, project, &t, &world(), 0)
}

fn dawproject(structure: &str, arrangement: &str, assets: &[(&str, &[u8])]) -> Vec<u8> {
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Project version="1.0">
  <Transport><Tempo value="120" unit="bpm"/><TimeSignature numerator="4" denominator="4"/></Transport>
  <Structure>{structure}</Structure>
  <Arrangement><Lanes timeUnit="beats">{arrangement}</Lanes></Arrangement>
</Project>"#
    );
    let mut entries: Vec<(&str, &[u8])> = vec![("project.xml", xml.as_bytes())];
    entries.extend_from_slice(assets);
    zip_of(&entries)
}

fn list(v: &Value, key: &str) -> Vec<Value> {
    match v.get(key) {
        Some(Value::List(l)) => l.clone(),
        other => panic!("{key} is not a list: {other:?}"),
    }
}

#[test]
fn a_dawproject_lands_as_a_project_that_compiles() {
    let notes = r#"<Notes id="n1">
        <Note time="0.0" duration="0.5" channel="0" key="36" vel="0.8"/>
        <Note time="1.0" duration="0.5" channel="0" key="38" vel="0.6"/></Notes>"#;
    let file = dawproject(
        r#"<Track id="t1" name="Drums" contentType="notes"><Channel id="c1" role="regular"/></Track>
           <Track id="t2" name="Vox" contentType="audio"><Channel id="c2" role="regular"/></Track>
           <Track id="t3" name="Master" contentType="audio notes"><Channel id="c3" role="master"/></Track>"#,
        &format!(
            r#"<Lanes track="t1"><Clips>
                 <Clip time="0.0" duration="4.0">{notes}</Clip>
                 <Clip time="8.0" duration="4.0" reference="n1"/>
               </Clips></Lanes>
               <Lanes track="t2"><Clips>
                 <Clip time="4.0" duration="4.0" fadeInTime="0.25"><Audio sampleRate="8000" duration="1.0"><File path="audio/vox.wav"/></Audio></Clip>
               </Clips></Lanes>"#
        ),
        &[("audio/vox.wav", &wav(1.0))],
    );
    let out = run("hiver.dawproject", &file, "hiver");

    assert_eq!(out.stats.sequence_files, 2);
    assert_eq!(out.stats.clip_files, 1);
    assert_eq!(
        out.stats.stand_ins, 1,
        "the drum lane has no instrument of its own"
    );
    assert!(
        out.log
            .iter()
            .any(|l| l.contains("3 tracks · 1 audio clips · 2 MIDI clips · 4 notes"))
    );
    assert_eq!(out.log.last().unwrap(), "wrote hiver");
    assert!(
        out.written
            .binaries
            .iter()
            .any(|(p, _)| p == "hiver/clips/vox.wav")
    );

    let mut p = land("hiver", &out.written);
    // The effects, read back as k7x.
    let lane = p.data("nodes/drums.k7x");
    let events = list(&lane, "events");
    assert_eq!(events.len(), 2);
    assert_eq!(events[1].get("start"), Some(&Value::Str("3:1".into())));
    assert_eq!(
        p.data("sequences/drums_1.k7x").get("to"),
        Some(&Value::Str("2:1".into()))
    );
    assert!(p.text("graph.k7x").contains("drums-instrument"));
    assert!(p.has("surfaces/timeline.k7x"));
    assert_eq!(
        p.data("project.k7x")
            .get("project")
            .and_then(|v| v.get("tempo")),
        Some(&Value::Num(120.0))
    );
    assert_eq!(
        p.data("clips/vox_0.k7x").get("source"),
        Some(&Value::Str("clips/vox.wav".into()))
    );

    let doc = p
        .compile()
        .unwrap_or_else(|e| panic!("does not compile: {e}"));
    assert_eq!(doc.tempo, 120.0);
    let drums = doc
        .tracks
        .iter()
        .find(|t| t.name == "drums")
        .expect("the drum lane");
    assert_eq!(
        drums.clips.iter().map(|c| c.start).collect::<Vec<_>>(),
        [0.0, 8.0]
    );
    let vox = doc
        .tracks
        .iter()
        .find(|t| t.name == "vox")
        .expect("the vocal lane");
    assert_eq!(vox.clips.iter().map(|c| c.start).collect::<Vec<_>>(), [4.0]);
}

fn als(tracks: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Ableton MajorVersion="5" MinorVersion="12.0_12402" Creator="Ableton Live 12.0.2">
 <LiveSet>
  <MainTrack><DeviceChain><Mixer>
    <Tempo><Manual Value="100"/></Tempo>
    <TimeSignature><Manual Value="201"/></TimeSignature>
  </Mixer></DeviceChain></MainTrack>
  <Tracks>{tracks}</Tracks>
 </LiveSet>
</Ableton>"#
    )
}

const MIDI_TRACK: &str = r#"<MidiTrack Id="1">
  <Name><EffectiveName Value="Keys"/></Name>
  <DeviceChain>
    <Mixer><Speaker><Manual Value="true"/></Speaker></Mixer>
    <MainSequencer><ClipTimeable><ArrangerAutomation><Events>
      <MidiClip Time="0" Id="1">
        <CurrentStart Value="0"/><CurrentEnd Value="4"/>
        <Loop><LoopStart Value="0"/><LoopEnd Value="4"/><StartRelative Value="0"/><LoopOn Value="false"/></Loop>
        <Notes><KeyTracks><KeyTrack Id="0">
          <Notes><MidiNoteEvent Time="0" Duration="1" Velocity="127" OffVelocity="64"/></Notes>
          <MidiKey Value="60"/>
        </KeyTrack></KeyTracks></Notes>
      </MidiClip>
    </Events></ArrangerAutomation></ClipTimeable></MainSequencer>
    <DeviceChain><Devices>
      <PluginDevice Id="0"><On><Manual Value="true"/></On><PluginDesc><Vst3PluginInfo>
        <Name Value="Diva"/>
        <Preset><Vst3Preset><ProcessorState>
          0A0B
          0C0D
        </ProcessorState></Vst3Preset></Preset>
      </Vst3PluginInfo></PluginDesc></PluginDevice>
      <Eq8 Id="1"/>
    </Devices></DeviceChain>
  </DeviceChain>
</MidiTrack>"#;

fn audio_clip(start: f64, name: &str, relative: &str) -> String {
    format!(
        r#"<AudioClip Time="{start}" Id="9">
  <CurrentStart Value="{start}"/><CurrentEnd Value="{}"/><IsWarped Value="false"/>
  <Loop><LoopStart Value="0"/><LoopEnd Value="1"/><StartRelative Value="0"/><LoopOn Value="false"/></Loop>
  <SampleRef><FileRef>
    <RelativePath Value="{relative}"/><Path Value="/Users/someone/{name}"/><Name Value="{name}"/>
  </FileRef><DefaultSampleRate Value="8000"/></SampleRef>
</AudioClip>"#,
        start + 2.0
    )
}

fn audio_track(clips: &str) -> String {
    format!(
        r#"<AudioTrack Id="2">
  <Name><EffectiveName Value="Vox"/></Name>
  <DeviceChain>
    <Mixer><Speaker><Manual Value="true"/></Speaker></Mixer>
    <MainSequencer><Sample><ArrangerAutomation><Events>{clips}</Events></ArrangerAutomation></Sample></MainSequencer>
    <DeviceChain><Devices/></DeviceChain>
  </DeviceChain>
</AudioTrack>"#
    )
}

#[test]
fn an_als_reads_the_samples_beside_it_and_counts_the_one_outside() {
    let xml = als(&format!(
        "{MIDI_TRACK}{}",
        audio_track(&format!(
            "{}{}",
            audio_clip(4.0, "vox.wav", "Samples/vox.wav"),
            audio_clip(12.0, "lost.wav", "")
        ))
    ));
    let bytes = gz(&xml);
    let beside: HashMap<&str, Bytes> = HashMap::from([("Samples/vox.wav", Bytes::new(wav(1.0)))]);
    let asked = std::cell::RefCell::new(Vec::new());
    let read = |r: &SampleRef| -> Option<Bytes> {
        asked.borrow_mut().push(r.name.clone());
        r.relative.as_deref().and_then(|p| beside.get(p).cloned())
    };
    let t = translate(&Input {
        source: "songs/hiver.als",
        file: "hiver.als",
        bytes: &bytes,
        read_sample: Some(&read),
        inflate_limit: None,
    })
    .unwrap();
    assert_eq!(t.song.tempo, 100.0);
    assert_eq!(t.song.time_signature, "4/4");
    assert_eq!(*asked.borrow(), ["vox.wav", "lost.wav"]);
    assert!(
        t.warnings
            .iter()
            .any(|w| w.contains("sample not imported: /Users/someone/lost.wav"))
    );
    assert!(t.warnings.iter().any(|w| w.contains("Eq8")));

    let out = emit_translation("songs/hiver.als", "hiver", &t, &world(), 1);
    assert!(
        out.log
            .iter()
            .any(|l| l.starts_with("warning: 1 sample(s) live outside the workspace"))
    );
    assert_eq!(
        out.stats.clip_files, 1,
        "the clip on the missing sample is left out"
    );

    let mut p = land("hiver", &out.written);
    assert!(p.has("clips/vox.wav"));
    assert_eq!(
        p.data("clips/vox_0.k7x").get("source"),
        Some(&Value::Str("clips/vox.wav".into()))
    );
    // A sole use of the plugin folds into its module, and the preset lands beside it, decoded from hex.
    let preset = out
        .written
        .binaries
        .iter()
        .find(|(path, _)| path == "_modules/instruments/diva/diva.vstpreset")
        .expect("the preset of the one plugin");
    assert_eq!(preset.1.as_slice(), [0x0A, 0x0B, 0x0C, 0x0D]);
    assert!(
        out.written
            .texts
            .iter()
            .any(|(p, _)| p == "_modules/instruments/diva/module.k7x")
    );

    let doc = p
        .compile()
        .unwrap_or_else(|e| panic!("does not compile: {e}"));
    assert_eq!(doc.tempo, 100.0);
    let vox = doc
        .tracks
        .iter()
        .find(|t| t.name == "vox")
        .expect("the vocal lane");
    assert_eq!(vox.clips.len(), 1);
    assert_eq!(vox.clips[0].start, 4.0);
}

#[test]
fn a_zipped_ableton_folder_finds_its_samples_inside_and_never_takes_a_backup() {
    let set = gz(&als(&audio_track(&audio_clip(
        0.0,
        "vox.wav",
        "Samples/vox.wav",
    ))));
    let old = gz(&als(""));
    let sound = wav(1.0);
    let file = zip_of(&[
        ("hiver Project/hiver.als", &set),
        ("hiver Project/Backup/hiver [2026-01-01].als", &old),
        ("hiver Project/Samples/vox.wav", &sound),
        ("__MACOSX/hiver Project/._hiver.als", b"fork"),
    ]);
    let out = run("hiver.zip", &file, "hiver");
    assert_eq!(out.stats.clip_files, 1);
    let mut p = land("hiver", &out.written);
    assert!(p.has("clips/vox.wav"));
    assert!(p.compile().is_ok());
}

#[test]
fn a_name_somebody_typed_is_refused_when_taken_and_a_derived_one_gets_a_suffix() {
    let taken = |n: &str| n == "hiver";
    assert_eq!(
        k7_import::folder_name("hiver.dawproject", Some("hiver"), &taken).unwrap_err(),
        "\"hiver\" already exists"
    );
    assert_eq!(
        k7_import::folder_name("hiver.dawproject", None, &taken).unwrap(),
        "hiver-2"
    );
}

#[test]
fn a_file_too_big_or_too_inflated_or_not_what_it_says_is_refused() {
    assert!(check_size("big.dawproject", MAX_IMPORT_BYTES + 1).is_err());
    assert!(check_droppable("lone.als").is_err());

    let file = dawproject("", "", &[("audio/big.wav", &vec![0u8; 4096])]);
    let mut small = input("x.dawproject", &file);
    small.inflate_limit = Some(1024);
    let err = translate(&small).err().unwrap();
    assert!(err.contains("inflates past"), "{err}");

    let err = translate(&input("x.als", b"not gzip")).err().unwrap();
    assert!(err.contains("not gzip-compressed"), "{err}");
    let err = translate(&input("x.dawproject", &zip_of(&[("other.xml", b"<a/>")])))
        .err()
        .unwrap();
    assert!(err.contains("project.xml missing"), "{err}");
    let entity = gz(&format!(r#"<!DOCTYPE x [<!ENTITY a "aaaa">]>{}"#, als("")));
    let err = translate(&input("x.als", &entity)).err().unwrap();
    assert!(err.contains("declares XML entities"), "{err}");
    let old = gz(&als("").replace("12.0_12402", "10.1_1"));
    assert!(
        translate(&input("x.als", &old))
            .err()
            .unwrap()
            .contains("Live 11 and 12")
    );
}

#[test]
fn a_unwarped_clip_region_is_in_seconds_and_a_warped_one_reads_through_its_markers() {
    let warped = r#"<AudioClip Time="0" Id="9">
  <CurrentStart Value="0"/><CurrentEnd Value="4"/><IsWarped Value="true"/>
  <Loop><LoopStart Value="0"/><LoopEnd Value="8"/><StartRelative Value="4"/><LoopOn Value="true"/></Loop>
  <WarpMarkers><WarpMarker SecTime="0" BeatTime="4"/><WarpMarker SecTime="2" BeatTime="8"/></WarpMarkers>
  <SampleRef><FileRef><RelativePath Value="a.wav"/><Name Value="a.wav"/></FileRef></SampleRef>
</AudioClip>"#;
    let bytes = gz(&als(&audio_track(warped)));
    let read = |_: &SampleRef| Some(Bytes::new(wav(1.0)));
    let t = translate(&Input {
        source: "x.als",
        file: "x.als",
        bytes: &bytes,
        read_sample: Some(&read),
        inflate_limit: None,
    })
    .unwrap();
    let clip = &t.song.tracks[0].clips[0];
    // Beat 4 of the clip's grid is the file's second 0: the in-point is 0 beats into the file.
    assert_eq!(clip.offset, None);
    assert_eq!(
        clip.warp.as_ref().unwrap().markers,
        [(4.0, 0.0), (8.0, 2.0)]
    );
}
