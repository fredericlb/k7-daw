//! cli.rs — `k7-runtime`'s argv in the shape every caller spells it: the modes, their usage errors, the port, the settings
//! folder taken out; and the documents `--render` and `--multi-edit-test` hand the engine, checked first.

use k7_runtime_server::cli::{self, DEFAULT_PORT, Run};
use k7_runtime_server::engine::Api;
use k7_runtime_server::engine::cli::Mode;
use k7_runtime_server::engine::fake;
use k7_runtime_server::log::framed;

fn args(line: &str) -> Vec<String> {
    line.split_whitespace().map(str::to_string).collect()
}

fn api() -> Api {
    Api::new(fake::api()).unwrap()
}

#[test]
fn the_modes_are_read_as_main_read_them() {
    let mode = |line: &str| match cli::parse(&args(line)) {
        Run::Mode(m) => m,
        other => panic!("{line}: {other:?}"),
    };
    assert_eq!(
        mode("--scan-plugin VST3 /a.vst3"),
        Mode::ScanChild {
            format: "VST3".into(),
            id: "/a.vst3".into()
        }
    );
    assert_eq!(mode("scan"), Mode::Scan { force: false });
    assert_eq!(mode("scan --force"), Mode::Scan { force: true });
    assert_eq!(
        mode("--dump-params 4osc"),
        Mode::DumpParams { ty: "4osc".into() }
    );
    assert_eq!(
        mode("--dump-params-ext VST3 id s.vstpreset"),
        Mode::DumpParamsExt {
            format: "VST3".into(),
            id: "id".into(),
            state_file: "s.vstpreset".into()
        }
    );
    assert_eq!(mode("--dump-builtin-params"), Mode::DumpBuiltinParams);
    assert_eq!(mode("--dump-surge-layout"), Mode::DumpSurgeLayout);
    assert_eq!(mode("--list-audio-devices"), Mode::ListAudioDevices);
    assert_eq!(
        mode("--decode a.wav"),
        Mode::Decode {
            path: "a.wav".into()
        }
    );
    assert_eq!(
        mode("--render - --stems --out /o --base /b"),
        Mode::Render {
            doc: "-".into(),
            base: "/b".into(),
            out: "/o".into(),
            stems: true
        }
    );
    assert_eq!(
        mode("--multi-edit-test a.json --seconds 2.5 b.json --device D --device-type CoreAudio"),
        Mode::MultiEdit {
            docs: vec!["a.json".into(), "b.json".into()],
            seconds: 2.5,
            device: "D".into(),
            device_type: "CoreAudio".into(),
        }
    );
}

#[test]
fn a_mode_short_of_its_arguments_is_a_usage_error_or_the_port() {
    assert!(matches!(
        cli::parse(&args("--render a.json")),
        Run::Usage(_)
    ));
    assert!(matches!(
        cli::parse(&args("--multi-edit-test a.json")),
        Run::Usage(_)
    ));
    // A mode missing its arguments falls through to the port, which `atoi` reads as 0.
    assert_eq!(cli::parse(&args("--decode")), Run::Serve(0));
    assert_eq!(cli::parse(&[]), Run::Serve(DEFAULT_PORT));
    assert_eq!(cli::parse(&args("9001")), Run::Serve(9001));
    assert_eq!(cli::atoi(" 42x"), 42);
    assert_eq!(cli::atoi("-7"), -7);
    assert_eq!(cli::atoi("x"), 0);
}

#[test]
fn the_settings_dir_is_taken_out_wherever_it_sits() {
    let (dir, rest) = cli::take_settings_dir(&args("--render a.json --settings-dir /s --out /o"));
    assert_eq!(dir, "/s");
    assert_eq!(rest, args("--render a.json --out /o"));
    let (dir, rest) = cli::take_settings_dir(&args("9000 --settings-dir"));
    assert_eq!((dir.as_str(), rest), ("", args("9000 --settings-dir")));
}

#[test]
fn a_document_is_checked_before_the_engine_reads_it() {
    let doc = cli::validated_text(
        api(),
        r#"{"tempo": -3, "tracks": [{"id": "a"}, {"id": "a"}]}"#,
    )
    .unwrap();
    assert_eq!(doc["tempo"], 120.0);
    assert_eq!(doc["tracks"][1]["id"], "a#2");
    let enveloped = cli::validated_text(api(), r#"{"doc": {"name": "x"}}"#).unwrap();
    assert_eq!(enveloped["name"], "x");
    let bad = cli::validated_text(api(), r#"{"tracks": 3}"#).unwrap_err();
    assert!(bad.starts_with("bad project: "), "{bad}");
    assert!(
        cli::validated_text(api(), "{")
            .unwrap_err()
            .starts_with("bad json: ")
    );
}

#[test]
fn render_hands_the_engine_the_validated_document() {
    let dir = std::env::temp_dir().join(format!("k7-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("doc.json");
    std::fs::write(&file, r#"{"doc": {"name": "cli-render", "sampleRate": 5}}"#).unwrap();
    let line = format!("--render {} --out /out-cli-render", file.display());
    let code = cli::run(api(), &args(&line), |_| panic!("no serve"));
    assert_eq!(code, 0);
    let call = fake::cli::asked()
        .into_iter()
        .find(|c| c[0] == "render" && c[3] == "/out-cli-render")
        .expect("the render reached the engine");
    let doc: serde_json::Value = serde_json::from_str(&call[1]).unwrap();
    assert_eq!(
        (doc["name"].as_str(), doc["sampleRate"].as_i64()),
        (Some("cli-render"), Some(48000))
    );
    let missing = cli::run(api(), &args("--render /no/such.json --out /o"), |_| 0);
    assert_eq!(missing, 2);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_tagged_line_is_one_write_with_its_mark() {
    assert_eq!(framed("hello", false), "hello\n");
    assert_eq!(framed("hello", true), "[engine] hello\n");
}
