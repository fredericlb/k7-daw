//! tunnel_harness.rs — the tunnel agent alone, for the conformance suite's stand-in engine (`e2e/src/standInEngine.ts`):
//! `tunnel_harness <registryUrl> <id> <token> <enginePort> <cacheDir>`. The agent's log goes to stderr as `tunnel | …`,
//! each `prefetch` job as one `prefetch [files]` line, and the process exits when stdin closes, the suite's way of stopping it.

use std::path::PathBuf;
use std::sync::Arc;

use k7_supervisor::tunnel::{Options, TunnelAgent};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, BufReader};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 6 {
        eprintln!("usage: tunnel_harness <registryUrl> <id> <token> <enginePort> <cacheDir>");
        std::process::exit(2);
    }
    let port: u16 = args[4].parse().unwrap_or(0);
    let mut o = Options::new(&args[1], &args[2], &args[3], PathBuf::from(&args[5]));
    o.engine_port = Arc::new(move || port);
    o.log = Arc::new(|m| eprintln!("tunnel | {m}"));
    // Says the file list it was asked for on stderr and answers `prefetched`, as a machine that has them all does.
    o.jobs.add("prefetch", |req, emit, _| async move {
        let files = req["files"].as_array().cloned().unwrap_or_default();
        eprintln!("prefetch {}", Value::Array(files.clone()));
        emit.send(json!({"type": "prefetched", "count": files.len()}));
        Ok(())
    });
    let agent = TunnelAgent::start(o);
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    while let Ok(Some(_)) = lines.next_line().await {}
    agent.stop();
    std::process::exit(0);
}
