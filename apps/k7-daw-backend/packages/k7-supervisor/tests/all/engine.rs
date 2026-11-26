//! engine.rs — which binary is the engine, which origins it lets in, and a start that finds its port held
//! moving to the next one without recording the holder.

use std::path::Path;

use k7_supervisor::engine::{Engine, StartAsk, StartKind, allowed_origins, find_binary};
use k7_supervisor::frames::Frames;
use k7_supervisor::identity;
use k7_supervisor::paths::Paths;

use crate::scratch::Scratch;

#[test]
fn the_origins_are_the_studios_deduplicated() {
    let urls = [
        "https://app.k7.rocks/api",
        "http://127.0.0.1:18789",
        "https://app.k7.rocks",
        "ftp://x",
        "nope",
    ];
    let urls: Vec<String> = urls.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        allowed_origins(&urls),
        "https://app.k7.rocks,http://127.0.0.1:18789"
    );
}

#[test]
fn the_override_then_the_copy_beside_then_the_newest_build_not_debug() {
    let dir = Scratch::new("bin");
    let root = dir.path();
    let touch = |p: &Path| {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, "").unwrap();
    };
    let build = root.join("apps/k7-daw-engine/k7-runtime/build");
    touch(&build.join("macos/Debug/k7-runtime"));
    std::thread::sleep(std::time::Duration::from_millis(20));
    touch(&build.join("macos/Release/k7-runtime"));
    std::thread::sleep(std::time::Duration::from_millis(20));
    touch(&build.join("other/Debug/k7-runtime"));
    let exe = root.join("apps/x/target/k7-supervisor");
    touch(&exe);
    assert_eq!(
        find_binary(None, Some(&exe)),
        Some(build.join("macos/Release/k7-runtime"))
    );
    let beside = root.join("apps/x/target/k7-runtime");
    touch(&beside);
    assert_eq!(find_binary(None, Some(&exe)), Some(beside.clone()));
    assert_eq!(find_binary(Some(&exe), Some(&exe)), Some(exe.clone()));
    assert_eq!(
        find_binary(Some(&root.join("gone")), Some(&exe)),
        Some(beside)
    );
}

#[cfg(unix)]
#[test]
fn a_held_port_is_skipped_and_its_holder_never_recorded() {
    let dir = Scratch::new("held");
    let paths = Paths::at(dir.path(), None);
    let held = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = held.local_addr().unwrap().port();
    let id = identity::ensure(&paths.engine_id());
    let fake = env!("CARGO_BIN_EXE_k7-fake-engine").into();
    let engine = Engine::new(paths.clone(), id, Frames::silent(), Some(fake));
    let fixed = StartAsk {
        first_port: port,
        port_fixed: true,
        origins: String::new(),
        safe_mode_crashes: 0,
        lan: false,
    };
    assert!(matches!(engine.start(&fixed).kind, StartKind::Error(why) if why.contains("--port")));
    assert!(!paths.engine_pid().exists());
    let started = engine.start(&StartAsk {
        port_fixed: false,
        ..fixed
    });
    assert_eq!(started.kind, StartKind::Spawned);
    assert!(started.port > port);
    assert!(engine.running());
    engine.stop();
    assert!(!paths.engine_pid().exists());
}
