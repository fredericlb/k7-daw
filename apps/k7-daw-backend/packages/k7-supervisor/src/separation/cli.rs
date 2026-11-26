//! cli.rs — `--separate <mix> <model> <outDir> [providers]`: one cut run from a terminal, its progress printed,
//! touching no state directory. Providers are a comma-separated list; the exit code is 1 when the cut failed.

use std::io::Write;

use super::job::{Request, run};

const USAGE: &str = "usage: k7-supervisor --separate <mix> <model> <outDir> [providers]";

/// Runs the cut the arguments after `--separate` describe, and gives the exit code.
pub fn main(args: &[String]) -> i32 {
    if !(3..=4).contains(&args.len()) {
        eprintln!("{USAGE}");
        return 2;
    }
    let r = Request {
        user: String::new(),
        source: args[0].clone(),
        model: args[1].clone(),
        out_dir: args[2].clone(),
        outputs: Vec::new(),
        providers: args
            .get(3)
            .map_or_else(Vec::new, |l| l.split(',').map(str::to_string).collect()),
    };
    let mut failed = false;
    let ran = run(r, |job| {
        let percent = job["percent"]
            .as_i64()
            .map_or(String::new(), |p| format!("{p}%"));
        println!("{} {percent}", job["phase"].as_str().unwrap_or(""));
        if job["done"] == true {
            for l in job["lines"].as_array().into_iter().flatten() {
                println!("  {}", l.as_str().unwrap_or(""));
            }
            for f in job["files"].as_array().into_iter().flatten() {
                println!("  → {}", f.as_str().unwrap_or(""));
            }
            if job["failed"] == true {
                println!("failed: {}", job["error"].as_str().unwrap_or(""));
            }
            failed = job["failed"] == true;
        }
        let _ = std::io::stdout().flush();
    });
    if let Err(e) = ran {
        println!("failed: {e}");
        return 1;
    }
    i32::from(failed)
}
