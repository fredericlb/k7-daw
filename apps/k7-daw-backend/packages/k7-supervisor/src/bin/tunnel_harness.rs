//! tunnel_harness.rs — the tunnel agent alone, for `engine-tunnel`'s suites to drive against the real hub:
//! `tunnel_harness <registryUrl> <id> <token> <enginePort> <cacheDir>`. It answers the two test jobs the
//! TypeScript suite gives its own agent (`where`, `upper`) and exits when stdin closes, the test's way of stopping it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use k7_supervisor::tunnel::{Options, TunnelAgent};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, BufReader};

fn text(req: &Value, key: &str) -> String {
    req[key].as_str().unwrap_or("").to_string()
}

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
    // Answers the same path twice: once mapped back to the server's, once under `raw` as the machine's.
    o.jobs.add("where", |req, emit, _| async move {
        let source = text(&req, "source");
        emit.send(json!({"type": "where", "path": source, "raw": {"path": source}}));
        Ok(())
    });
    // Reads the source it was handed, writes one "stem" beside it, reporting progress first.
    o.jobs.add("upper", |req, emit, _| async move {
        let (source, out_dir) = (text(&req, "source"), text(&req, "outDir"));
        let upper = std::fs::read_to_string(&source)
            .map_err(|e| e.to_string())?
            .to_uppercase();
        emit.send(json!({"type": "progress", "percent": 50}));
        std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
        let name = Path::new(&source)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let out = Path::new(&out_dir).join(format!("{name}.up"));
        std::fs::write(&out, upper).map_err(|e| e.to_string())?;
        emit.send(json!({"type": "done", "files": [out.to_string_lossy()]}));
        Ok(())
    });
    let agent = TunnelAgent::start(o);
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    while let Ok(Some(_)) = lines.next_line().await {}
    agent.stop();
    std::process::exit(0);
}
