//! cli.rs — `k7-runtime`'s command line: which one-shot mode argv asks for, or the port to serve on, and the
//! documents `--render` and `--multi-edit-test` read, checked as the socket checks one (`docs/cli.md`).
//! What a reader gets wrong: `--settings-dir` is taken out of argv before anything else reads it, wherever it
//! sits, and a first argument that is no mode is the port, as C's `atoi` read it.

use crate::doc::{Checks, parse_project};
use crate::engine::Api;
use crate::engine::cli::Mode;
use crate::log::say;
use crate::server::msg;
use crate::wire::json;
use serde_json::{Value, json};
use std::io::Read as _;

/// The port served when argv names none.
pub const DEFAULT_PORT: i32 = 8765;

/// What argv asks for, once `--settings-dir` is taken out.
#[derive(Clone, Debug, PartialEq)]
pub enum Run {
    /// A mode, its documents still file names.
    Mode(Mode),
    Serve(i32),
    /// A usage error, already worded; the exit code is 2.
    Usage(String),
}

/// argv without its program name: the `--settings-dir` value (empty for none), and the rest.
pub fn take_settings_dir(args: &[String]) -> (String, Vec<String>) {
    let (mut dir, mut rest) = (String::new(), Vec::new());
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--settings-dir" && i + 1 < args.len() {
            dir = args[i + 1].clone();
            i += 2;
            continue;
        }
        rest.push(args[i].clone());
        i += 1;
    }
    (dir, rest)
}

/// C's `atoi`: leading blanks, a sign, the digits that follow; 0 for none.
pub fn atoi(s: &str) -> i32 {
    let t = s.trim_start();
    let (sign, digits) = match t.as_bytes().first() {
        Some(b'-') => (-1i64, &t[1..]),
        Some(b'+') => (1, &t[1..]),
        _ => (1, t),
    };
    let n = digits
        .bytes()
        .take_while(u8::is_ascii_digit)
        .fold(0i64, |n, d| {
            (n * 10 + i64::from(d - b'0')).min(i64::from(i32::MAX) + 1)
        });
    (sign * n).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

/// argv (without its program name and `--settings-dir`) read in the shape every caller spells it.
pub fn parse(args: &[String]) -> Run {
    let a = |i: usize| args.get(i).cloned().unwrap_or_default();
    let n = args.len();
    match a(0).as_str() {
        "--scan-plugin" if n >= 3 => Run::Mode(Mode::ScanChild {
            format: a(1),
            id: a(2),
        }),
        "scan" => Run::Mode(Mode::Scan {
            force: a(1) == "--force",
        }),
        "--dump-params-ext" if n >= 4 => Run::Mode(Mode::DumpParamsExt {
            format: a(1),
            id: a(2),
            state_file: a(3),
        }),
        "--dump-params" if n >= 2 => Run::Mode(Mode::DumpParams { ty: a(1) }),
        "--dump-builtin-params" => Run::Mode(Mode::DumpBuiltinParams),
        "--dump-surge-layout" => Run::Mode(Mode::DumpSurgeLayout),
        "--list-audio-devices" => Run::Mode(Mode::ListAudioDevices),
        "--multi-edit-test" => multi_edit(&args[1..]),
        "--render" => render(&args[1..]),
        "--decode" if n >= 2 => Run::Mode(Mode::Decode { path: a(1) }),
        "" => Run::Serve(DEFAULT_PORT),
        port => Run::Serve(atoi(port)),
    }
}

fn multi_edit(args: &[String]) -> Run {
    let (mut docs, mut seconds, mut device, mut device_type) =
        (Vec::new(), 5.0, String::new(), String::new());
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--seconds" if it.len() > 0 => seconds = it.next().map_or(0.0, |s| atof(s)),
            "--device" if it.len() > 0 => device = it.next().cloned().unwrap_or_default(),
            "--device-type" if it.len() > 0 => device_type = it.next().cloned().unwrap_or_default(),
            _ => docs.push(a.clone()),
        }
    }
    if docs.len() < 2 {
        return Run::Usage(
            "usage: k7-runtime --multi-edit-test <doc1.json> <doc2.json> [--seconds N]".into(),
        );
    }
    Run::Mode(Mode::MultiEdit {
        docs,
        seconds,
        device,
        device_type,
    })
}

fn render(args: &[String]) -> Run {
    let (mut doc, mut out, mut base, mut stems) =
        (String::new(), String::new(), String::new(), false);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--out" if it.len() > 0 => out = it.next().cloned().unwrap_or_default(),
            "--base" if it.len() > 0 => base = it.next().cloned().unwrap_or_default(),
            "--stems" => stems = true,
            _ if doc.is_empty() => doc = a.clone(),
            _ => {}
        }
    }
    if doc.is_empty() || out.is_empty() {
        return Run::Usage(
            "usage: k7-runtime --render <doc.json|-> --out <dir> [--stems] [--base <dir>]".into(),
        );
    }
    Run::Mode(Mode::Render {
        doc,
        base,
        out,
        stems,
    })
}

/// C's `atof` for the plain numbers a flag carries; 0 for none.
fn atof(s: &str) -> f64 {
    s.trim().parse().unwrap_or(0.0)
}

/// The engine's roots and warnings, without a host: what the offline modes' documents are held to.
fn checks(api: Api) -> Checks<'static> {
    let allowed = move |p: &str, write: bool| {
        api.project().is_some_and(|t| unsafe {
            (t.path_allowed)(crate::engine::ffi::K7Str::of(p), i32::from(write)) != 0
        })
    };
    Checks::new(
        move |p| allowed(p, false),
        move |p| allowed(p, true),
        move |message, subject| {
            if let Some(t) = api.project() {
                use crate::engine::ffi::K7Str;
                unsafe { (t.warn)(K7Str::of(""), K7Str::of(subject), K7Str::of(message)) }
            }
        },
    )
}

/// A file's text, or stdin's for `-`; `Err` is already worded.
fn text_of(input: &str) -> Result<String, String> {
    let mut text = String::new();
    let read = if input == "-" {
        std::io::stdin().read_to_string(&mut text).map(|_| ())
    } else {
        std::fs::read_to_string(input).map(|t| text = t)
    };
    read.map(|()| text)
        .map_err(|_| format!("cannot open {input}"))
}

/// A document file (or stdin for `-`), bare or in a `{ "doc": … }` envelope, printed as the C++ reader reads it.
pub fn validated(api: Api, input: &str) -> Result<String, String> {
    let doc = validated_text(api, &text_of(input)?)?;
    Ok(json::dump(&doc))
}

/// The mode with its documents read; `Err` is the exit code, its reason already on stderr.
fn with_documents(api: Api, mode: Mode) -> Result<Mode, i32> {
    let read = |input: &str| {
        validated(api, input).map_err(|why| {
            say!("{why}");
            2
        })
    };
    Ok(match mode {
        Mode::Render {
            doc,
            base,
            out,
            stems,
        } => Mode::Render {
            doc: read(&doc)?,
            base,
            out,
            stems,
        },
        Mode::MultiEdit {
            docs,
            seconds,
            device,
            device_type,
        } => Mode::MultiEdit {
            docs: docs.iter().map(|d| read(d)).collect::<Result<_, _>>()?,
            seconds,
            device,
            device_type,
        },
        other => other,
    })
}

/// The whole process past its settings folder and layout check: a mode, or the server.
pub fn run(api: Api, args: &[String], serve: impl FnOnce(i32) -> i32) -> i32 {
    match parse(args) {
        Run::Usage(text) => {
            say!("{text}");
            2
        }
        Run::Serve(port) => serve(port),
        Run::Mode(mode) => match with_documents(api, mode) {
            Ok(mode) => api.run_mode(&mode),
            Err(code) => code,
        },
    }
}

/// A document's text checked against `api`'s roots, bare or in a `{ "doc": … }` envelope.
pub fn validated_text(api: Api, text: &str) -> Result<Value, String> {
    let mut j = msg::parse(text).map_err(|e| format!("bad json: {e}"))?;
    if j.get("doc").is_none() {
        j = json!({"doc": j});
    }
    parse_project(&j, "", &checks(api))
        .map(|p| p.doc)
        .map_err(|e| format!("bad project: {}", e.0))
}
