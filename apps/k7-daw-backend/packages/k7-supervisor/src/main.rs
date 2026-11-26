//! main.rs — the `k7-supervisor` program: driven by an app over stdio with no argument, the command-line
//! engine with `--config <file> [--port <n>]`, or one of the diagnostics. Rust ignores SIGPIPE already, so
//! a frame written after the app died fails as a write rather than ending us before the deregister.

use std::path::PathBuf;
use std::process::ExitCode;

use k7_supervisor::diagnostics;
use k7_supervisor::run::{self, Mode};

/// `--port`'s value: a whole number from 1 to 65535.
fn parse_port(text: &str) -> Option<u16> {
    (!text.is_empty() && text.len() <= 5 && text.bytes().all(|b| b.is_ascii_digit()))
        .then(|| text.parse::<u16>().ok())
        .flatten()
        .filter(|&p| p > 0)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.as_slice() {
        [] => run::main(Mode::Stdio),
        [flag, file] if flag == "--config" => run::main(Mode::Config {
            file: PathBuf::from(file),
            port: None,
        }),
        [flag, file, p, n] if flag == "--config" && p == "--port" => match parse_port(n) {
            Some(port) => run::main(Mode::Config {
                file: PathBuf::from(file),
                port: Some(port),
            }),
            None => {
                eprintln!("--port takes a port from 1 to 65535, not {n}");
                2
            }
        },
        [flag, rest @ ..] => match diagnostics::ALL.iter().find(|d| d.flag == flag) {
            Some(d) => (d.run)(rest),
            None => {
                eprint!("{}", diagnostics::usage());
                2
            }
        },
    };
    ExitCode::from(code.clamp(0, 255) as u8)
}
