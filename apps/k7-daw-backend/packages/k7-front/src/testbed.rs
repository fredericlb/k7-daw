//! k7-engine-testbed — the front as k7-front runs it, the engine core ON and the testbed's hooks mounted under
//! `/__engine-test`, for the TypeScript suites to drive (`apps/k7-daw-frontend/packages/engine-tunnel/testCore.ts`). Never shipped:
//! it exists only in a build with the `test-hooks` feature, and refuses to start without `K7_ENGINE_TESTBED=1`.

#[tokio::main]
async fn main() {
    if std::env::var("K7_ENGINE_TESTBED").as_deref() != Ok("1") {
        eprintln!("k7-engine-testbed: set K7_ENGINE_TESTBED=1; this binary carries test hooks");
        std::process::exit(2);
    }
    let mut config = match k7_front::Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("k7-engine-testbed: {e}");
            std::process::exit(2);
        }
    };
    config.env.insert("K7_ENGINE_CORE".into(), "front".into());
    let ready = |addr| println!("k7-front on http://{addr}");
    let stop = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    if let Err(e) = k7_front::serve(config, stop, ready).await {
        eprintln!("k7-engine-testbed: {e}");
        std::process::exit(1);
    }
}
