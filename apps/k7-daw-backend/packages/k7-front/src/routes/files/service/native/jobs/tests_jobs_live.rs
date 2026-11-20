//! tests_jobs_live.rs — the live harness (2361), `#[ignore]`: the real engine binary named by `K7_LIVE_ENGINE_BIN`; only the local render is here yet.
//! What a reader gets wrong: audio is compared by its `data` chunk, never the file's bytes (the `bext` chunk holds the render's date, 2323).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::json;

use super::render::{LocalRenderer, RenderOpts};

/// The engine's `data` chunk and what it says of the format: (format tag, bits, channels, bytes).
fn data_of(wav: &Path) -> (u16, u16, u16, Vec<u8>) {
    let b = std::fs::read(wav).unwrap();
    assert!(
        &b[..4] == b"RIFF" && &b[8..12] == b"WAVE",
        "{wav:?} is no wav"
    );
    let (mut at, mut fmt) = (12, (0, 0, 0));
    while at + 8 <= b.len() {
        let size = u32::from_le_bytes(b[at + 4..at + 8].try_into().unwrap()) as usize;
        let body = &b[at + 8..(at + 8 + size).min(b.len())];
        match &b[at..at + 4] {
            b"fmt " => {
                let n = |i: usize| u16::from_le_bytes(body[i..i + 2].try_into().unwrap());
                fmt = (n(0), n(14), n(2));
            }
            b"data" => return (fmt.0, fmt.1, fmt.2, body.to_vec()),
            _ => {}
        }
        at += 8 + size + (size & 1);
    }
    panic!("{wav:?} has no data chunk");
}

/// The loudest sample, as a fraction of full scale.
fn peak(tag: u16, bits: u16, data: &[u8]) -> f64 {
    match (tag, bits) {
        (1, 16) => data
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| f64::from(i16::from_le_bytes(*c)).abs() / 32768.0)
            .fold(0.0, f64::max),
        (1, 24) => data
            .as_chunks::<3>()
            .0
            .iter()
            .map(|c| f64::from(i32::from_le_bytes([0, c[0], c[1], c[2]]) >> 8).abs() / 8_388_608.0)
            .fold(0.0, f64::max),
        (3, 32) => data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| f64::from(f32::from_le_bytes(*c)).abs())
            .fold(0.0, f64::max),
        other => panic!("a wav format the test does not read: {other:?}"),
    }
}

/// Two seconds of 440 Hz, 16-bit stereo at 44.1 kHz.
fn write_sine(file: &Path) {
    let rate = 44_100u32;
    let mut data = Vec::new();
    for i in 0..2 * rate {
        let s = (8000.0 * (std::f64::consts::TAU * 440.0 * f64::from(i) / f64::from(rate)).sin())
            as i16;
        data.extend_from_slice(&s.to_le_bytes());
        data.extend_from_slice(&s.to_le_bytes());
    }
    let mut wav = b"RIFF".to_vec();
    wav.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    for v in [1u16, 2] {
        wav.extend_from_slice(&v.to_le_bytes());
    }
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * 4).to_le_bytes());
    for v in [4u16, 16] {
        wav.extend_from_slice(&v.to_le_bytes());
    }
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
    wav.extend_from_slice(&data);
    std::fs::write(file, wav).unwrap();
}

/// A ProjectDoc with one audio clip of the sine, as `e2e/fixtures/clip-loop` compiles to for this engine.
fn sine_doc(sine: &Path) -> String {
    let clip = json!({ "id": "c1", "type": "audio", "name": "sine", "source": sine, "start": 0, "length": 4,
        "offset": 0, "fadeIn": 0, "fadeOut": 0, "loopStart": 0, "loopEnd": 0, "notes": [] });
    let track = json!({ "id": "t1", "name": "sine", "role": "audio", "contentType": "audio", "group": "",
        "generated": false, "color": "#ffffff", "mute": false, "solo": false, "disabled": false,
        "inputChannels": [], "outputChannels": [], "monitor": "auto", "inputGain": 0, "clips": [clip],
        "plugins": [], "sends": [], "automation": [] });
    json!({ "name": "sine", "tempo": 120, "sampleRate": 48000, "timeSigNumerator": 4, "timeSigDenominator": 4,
        "masterVolume": 0, "masterPlugins": [], "recordDir": "", "tracks": [track], "groups": [], "markers": [],
        "settings": { "metronome": false, "metronomeLevel": 0.5, "countIn": 0 } })
    .to_string()
}

fn scratch(name: &str) -> PathBuf {
    let dir = crate::routes::files::service::native::tests::tmp_root()
        .join(format!("k7-live-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::canonicalize(dir).unwrap()
}

/// The account's environment for a render: a home of its own, so the user's scan and settings are never read.
fn account(dir: &Path) -> Vec<(String, String)> {
    let home = dir.join("engine-home");
    std::fs::create_dir_all(&home).unwrap();
    let to = |p: PathBuf| p.to_string_lossy().into_owned();
    vec![
        ("PATH".into(), "/usr/bin:/bin".into()),
        ("TMPDIR".into(), to(std::env::temp_dir())),
        ("HOME".into(), to(home.clone())),
        ("STUDIO_HOME".into(), to(home.join(".k7"))),
        ("STUDIO_ENGINE_SILENT".into(), "1".into()),
    ]
}

/// What the TypeScript's `localRenderer` runs: `--render <doc> --out <dir>`, the manifest on stdout's last line.
fn by_the_cli(bin: &Path, env: &[(String, String)], doc: &Path, out: &Path) -> String {
    let run = Command::new(bin)
        .arg("--render")
        .arg(doc)
        .arg("--out")
        .arg(out)
        .env_clear()
        .envs(env.iter().cloned())
        .output()
        .unwrap();
    assert!(run.status.success(), "{:?}", run.status);
    let stdout = String::from_utf8_lossy(&run.stdout).into_owned();
    let line = stdout.lines().rfind(|l| !l.is_empty()).unwrap().to_owned();
    serde_json::from_str::<serde_json::Value>(&line).unwrap()["mix"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[tokio::test]
#[ignore = "needs a built engine: K7_LIVE_ENGINE_BIN"]
async fn local_render() {
    let bin = PathBuf::from(
        std::env::var("K7_LIVE_ENGINE_BIN").expect("K7_LIVE_ENGINE_BIN names a built k7-runtime"),
    );
    let dir = scratch("local");
    let (sine, env) = (dir.join("sine.wav"), account(&dir));
    write_sine(&sine);
    let doc = sine_doc(&sine);
    let render = LocalRenderer::new(bin.clone(), env.clone());

    // Native: the manifest comes back checked, and the data is the sine, loud enough to be heard.
    let moved = render
        .render(&doc, &dir.join("native"), &RenderOpts::default())
        .await
        .unwrap();
    assert_eq!(moved.stems, []);
    let (tag, bits, channels, native) = data_of(&dir.join("native").join(&moved.mix));
    assert_eq!(channels, 2);
    assert!(
        peak(tag, bits, &native) > 0.001,
        "silent: -60 dBFS is 0.001"
    );

    // The CLI as the TypeScript runs it, on the same document: the same audio.
    std::fs::create_dir_all(dir.join("cli")).unwrap();
    std::fs::write(dir.join("cli/doc.json"), &doc).unwrap();
    let mix = by_the_cli(&bin, &env, &dir.join("cli/doc.json"), &dir.join("cli"));
    assert_eq!(data_of(&dir.join("cli").join(mix)).3, native, "data chunks");

    // Staged: the render lands under its name whole, and nothing else stays beside it.
    let wav = dir.join("renders/fp.wav");
    render.render_to_wav(&doc, &wav).await.unwrap();
    assert_eq!(data_of(&wav).3, native, "the staged render's data chunk");
    let beside: Vec<_> = std::fs::read_dir(dir.join("renders"))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(beside, ["fp.wav"]);
    let _ = std::fs::remove_dir_all(&dir);
}
