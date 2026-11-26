//! fake_engine.rs — an engine that only listens, for the supervisor's tests: `k7-fake-engine <port>` binds
//! `$STUDIO_ENGINE_HOST:<port>`, says so and the safe mode it was given on stdout, and accepts until killed.

use std::io::Write;
use std::net::TcpListener;

fn main() {
    let port = std::env::args().nth(1).unwrap_or_default();
    let host = std::env::var("STUDIO_ENGINE_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let listener = TcpListener::bind(format!("{host}:{port}")).expect("the engine's port");
    let safe = std::env::var("STUDIO_ENGINE_SAFE_MODE").unwrap_or_default();
    println!("fake engine on :{port}, safe mode {safe}");
    let _ = std::io::stdout().flush();
    for conn in listener.incoming() {
        drop(conn);
    }
}
