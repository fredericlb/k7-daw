//! separation_cut.rs — a whole cut, on the tiny stand-in model (`onnx_model.rs`): the stems come out in the model's
//! order and rebuild the mix, a second asker joins the cut running, an unknown provider gives way to the CPU and
//! says so, and what cannot be cut fails with a reason. The cuts that need ONNX Runtime say so and pass when none
//! is installed (`STUDIO_ONNXRUNTIME_DIR`, or beside the test): it is a download, which no clean checkout holds.

use std::path::Path;
use std::process::Command;

use k7_supervisor::separation::audio::{SAMPLE_RATE, wav_pcm24};
use k7_supervisor::separation::onnx::available;
use k7_supervisor::separation::{Request, cuts_in_flight, run};
use serde_json::Value;

use crate::onnx_model::model;
use crate::scratch::Scratch;
use crate::separation::sine;

fn runtime() -> bool {
    let there = available();
    if !there {
        eprintln!(
            "skipped: no ONNX Runtime (set STUDIO_ONNXRUNTIME_DIR to a folder holding libonnxruntime)"
        );
    }
    there
}

/// A stereo clip of two tones, written as a WAV, and the folder holding it with the model.
fn setup(tag: &str, seconds: f64) -> (Scratch, Vec<Vec<f32>>) {
    let dir = Scratch::new(tag);
    let frames = (seconds * f64::from(SAMPLE_RATE)) as usize;
    let left: Vec<f32> = sine(SAMPLE_RATE, frames).iter().map(|v| v * 0.8).collect();
    let right: Vec<f32> = sine(SAMPLE_RATE, frames)
        .iter()
        .rev()
        .map(|v| v * 0.6)
        .collect();
    let clip = vec![left, right];
    std::fs::write(dir.path().join("mix.wav"), wav_pcm24(&clip, SAMPLE_RATE)).unwrap();
    std::fs::write(dir.path().join("model.onnx"), model()).unwrap();
    (dir, clip)
}

fn request(dir: &Path, providers: &[&str], outputs: &[&str]) -> Request {
    let s = |p: &str| dir.join(p).to_string_lossy().into_owned();
    Request {
        user: "tester".into(),
        source: s("mix.wav"),
        model: s("model.onnx"),
        out_dir: s("stems"),
        outputs: outputs.iter().map(|o| o.to_string()).collect(),
        providers: providers.iter().map(|p| p.to_string()).collect(),
    }
}

/// Every report of a cut, and the last one.
fn cut(r: Request) -> (Vec<Value>, Value) {
    let mut seen = Vec::new();
    run(r, |job| seen.push(job.clone())).expect("a cut");
    let last = seen.last().cloned().expect("a report");
    (seen, last)
}

/// The channels of a 24-bit stereo WAV.
fn read(path: &str) -> Vec<Vec<f32>> {
    let b = std::fs::read(path).unwrap();
    let frames = (b.len() - 44) / 6;
    let mut out = vec![Vec::new(), Vec::new()];
    for i in 0..frames * 2 {
        let o = 44 + i * 3;
        let u = u32::from(b[o]) | u32::from(b[o + 1]) << 8 | u32::from(b[o + 2]) << 16;
        let s = if u & 0x80_0000 != 0 {
            u as i32 - 0x100_0000
        } else {
            u as i32
        };
        out[i % 2].push(s as f32 / 8_388_607.0);
    }
    out
}

fn worst(a: &[Vec<f32>], b: &[Vec<f32>], sign: f32) -> f32 {
    a.iter()
        .zip(b)
        .flat_map(|(x, y)| x.iter().zip(y).map(move |(p, q)| (p - sign * q).abs()))
        .fold(0.0, f32::max)
}

fn wait_idle() {
    for _ in 0..200 {
        if cuts_in_flight() == 0 {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    panic!("a cut is still counted");
}

#[test]
fn an_output_that_is_not_one_is_refused_up_front() {
    let (dir, _) = setup("badout", 0.1);
    let err = run(request(dir.path(), &[], &["guitar"]), |_| {}).unwrap_err();
    assert_eq!(err, "'guitar' is not a separation output");
}

#[test]
fn what_is_not_audio_fails_the_cut_with_a_reason() {
    let (dir, _) = setup("notaudio", 0.1);
    std::fs::write(dir.path().join("mix.wav"), "not a wav").unwrap();
    let (_, last) = cut(request(dir.path(), &["cpu"], &[]));
    assert_eq!(last["done"], true);
    assert_eq!(last["failed"], true);
    assert_eq!(last["phase"], "failed");
    assert!(!last["error"].as_str().unwrap().is_empty());
    wait_idle();
}

#[test]
fn the_stems_come_out_in_the_models_order_and_rebuild_the_mix() {
    if !runtime() {
        return;
    }
    // Longer than one window, so the overlap-add folds two.
    let (dir, clip) = setup("stems", 9.0);
    let (seen, last) = cut(request(
        dir.path(),
        &["cpu"],
        &["drums", "bass", "instrumental"],
    ));
    assert_eq!(last["failed"], false, "{last}");
    assert_eq!(last["phase"], "done");
    assert_eq!(last["percent"], 100);
    let files: Vec<&str> = last["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f.as_str().unwrap())
        .collect();
    assert_eq!(files.len(), 3);
    assert!(files[0].ends_with("drums.wav") && files[2].ends_with("instrumental.wav"));
    assert!(worst(&read(files[0]), &clip, 1.0) < 1e-4);
    assert!(worst(&read(files[1]), &clip, -1.0) < 1e-4);
    // Drums + bass + other: the mix, its negation and the mix again.
    assert!(worst(&read(files[2]), &clip, 1.0) < 1e-4);
    let said: Vec<&str> = seen
        .iter()
        .flat_map(|j| j["lines"].as_array().unwrap())
        .map(|l| l.as_str().unwrap())
        .collect();
    assert!(
        said.iter()
            .any(|l| l.starts_with("execution provider: cpu")),
        "{said:?}"
    );
    wait_idle();
}

#[test]
fn an_unknown_provider_gives_way_to_the_cpu_and_says_so() {
    if !runtime() {
        return;
    }
    let (dir, _) = setup("provider", 0.5);
    let (_, last) = cut(request(dir.path(), &["bogus", "cpu"], &["vocals"]));
    assert_eq!(last["failed"], false, "{last}");
    let said: Vec<&str> = last["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l.as_str().unwrap())
        .collect();
    assert!(
        said.iter().any(|l| l.contains("execution provider: cpu")
            && l.contains("skipped bogus: not in this build")),
        "{said:?}"
    );
    wait_idle();
}

#[test]
fn a_provider_that_will_not_load_fails_the_cut_naming_each() {
    if !runtime() {
        return;
    }
    let (dir, _) = setup("noprovider", 0.2);
    let (_, last) = cut(request(dir.path(), &["bogus"], &[]));
    assert_eq!(last["failed"], true);
    assert!(
        last["error"]
            .as_str()
            .unwrap()
            .contains("no ONNX execution provider would load")
    );
    assert!(
        last["error"]
            .as_str()
            .unwrap()
            .contains("bogus: not in this build")
    );
    wait_idle();
}

#[cfg(target_vendor = "apple")]
#[test]
fn coreml_is_tried_first_and_the_cpu_answers_when_it_gives_way() {
    if !runtime() {
        return;
    }
    let (dir, _) = setup("coreml", 0.3);
    let (_, last) = cut(request(dir.path(), &["coreml", "cpu"], &["bass"]));
    assert_eq!(last["failed"], false, "{last}");
    let said: Vec<&str> = last["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l.as_str().unwrap())
        .collect();
    eprintln!("{said:?}");
    assert!(
        said.iter()
            .any(|l| l.starts_with("execution provider: coreml") || l.contains("skipped coreml: ")),
        "{said:?}"
    );
    wait_idle();
}

#[test]
fn a_second_asker_joins_the_cut_running() {
    if !runtime() {
        return;
    }
    let (dir, _) = setup("join", 0.5);
    let ask = || cut(request(dir.path(), &["cpu"], &["bass"])).1;
    let (a, b) = std::thread::scope(|s| {
        let a = s.spawn(ask);
        let b = s.spawn(ask);
        (a.join().unwrap(), b.join().unwrap())
    });
    assert_eq!(a["id"], b["id"]);
    assert_eq!(a["id"].as_str().unwrap().len(), 16);
    assert_eq!(a["files"], b["files"]);
    wait_idle();
}

#[test]
fn the_program_cuts_from_a_terminal_and_fails_with_a_status() {
    let (dir, clip) = setup("cli", 0.5);
    let program = env!("CARGO_BIN_EXE_k7-supervisor");
    let args = |providers: &str| {
        let mut c = Command::new(program);
        c.arg("--separate")
            .arg(dir.path().join("mix.wav"))
            .arg(dir.path().join("model.onnx"))
            .arg(dir.path().join("out"))
            .arg(providers);
        c.output().expect("the program runs")
    };
    let wrong = Command::new(program).arg("--separate").output().unwrap();
    assert_eq!(wrong.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&wrong.stderr).contains("--separate"));
    std::fs::write(dir.path().join("junk.wav"), "junk").unwrap();
    let failed = Command::new(program)
        .arg("--separate")
        .arg(dir.path().join("junk.wav"))
        .arg(dir.path().join("model.onnx"))
        .arg(dir.path().join("out"))
        .output()
        .unwrap();
    assert_eq!(failed.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&failed.stdout).contains("failed: "));
    if !runtime() {
        return;
    }
    let ok = args("cpu");
    let out = String::from_utf8_lossy(&ok.stdout).into_owned();
    assert_eq!(ok.status.code(), Some(0), "{out}");
    assert!(
        out.contains("done 100%") && out.contains("execution provider: cpu"),
        "{out}"
    );
    assert!(
        worst(
            &read(dir.path().join("out/drums.wav").to_str().unwrap()),
            &clip,
            1.0
        ) < 1e-4
    );
}
