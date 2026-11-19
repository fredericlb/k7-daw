//! embedded.rs — the front inside a host's own process, on a thread of its own with a small runtime: the
//! tray links this directly, the phones through `k7-embed`'s C and JNI calls (`apps/k7/backend-rs/docs/embed.md`).
//! One front per process: a second start while one runs is refused, not stacked.

use std::sync::Mutex;
use std::thread::JoinHandle;

struct Running {
    stop: tokio::sync::oneshot::Sender<()>,
    thread: JoinHandle<()>,
}

static RUNNING: Mutex<Option<Running>> = Mutex::new(None);

/// Starts the front on a thread of its own; its port, or -3 when one runs already, -4 when the runtime or the bind fails, -5 when `auth_guard` refuses its settings.
pub fn start(config: crate::Config) -> Result<u16, i32> {
    // A host that chose no sink for the front's last words gets stderr (`exit.rs`).
    crate::exit::watch(None);
    // The song sessions take an embedded host's share of memory here (docs/service.md).
    crate::routes::files::service::set_embedded();
    if let Some(refused) = crate::auth_guard::refusal(&config.env) {
        eprintln!("{refused}");
        return Err(-5);
    }
    let config = match config.settled() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("k7-front: {e}");
            return Err(-2);
        }
    };
    let mut slot = RUNNING.lock().unwrap_or_else(|p| p.into_inner());
    if slot.is_some() {
        return Err(-3);
    }
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let (bound, port) = std::sync::mpsc::channel::<Result<u16, ()>>();
    let thread = std::thread::Builder::new()
        .name("k7-front".into())
        .spawn(move || {
            // Four, not two: three families start at once and each blocks a worker, so the file family a page asks first must not queue (docs/embed.md).
            let Ok(runtime) = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .enable_all()
                .build()
            else {
                let _ = bound.send(Err(()));
                return;
            };
            let ready = bound.clone();
            let served = runtime.block_on(crate::serve(
                config,
                async {
                    let _ = stopped.await;
                },
                move |a| {
                    let _ = ready.send(Ok(a.port()));
                },
            ));
            // Said whatever ended it: a host whose front stops answering reads why here.
            match served {
                Ok(()) => crate::exit::say("k7-front: stopped"),
                Err(e) => {
                    crate::exit::say(&format!("k7-front: stopped: {e}"));
                    let _ = bound.send(Err(()));
                }
            }
        })
        .map_err(|_| -4)?;
    match port.recv() {
        Ok(Ok(p)) => {
            *slot = Some(Running { stop, thread });
            Ok(p)
        }
        _ => {
            let _ = thread.join();
            Err(-4)
        }
    }
}

/// Stops the running front and waits for it; false when none runs.
pub fn stop() -> bool {
    let Some(running) = RUNNING.lock().unwrap_or_else(|p| p.into_inner()).take() else {
        return false;
    };
    let _ = running.stop.send(());
    let _ = running.thread.join();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_once_then_stops() {
        let config = crate::Config {
            listen: "127.0.0.1:0".parse().unwrap(),
            secret: "s".into(),
            trust_hops: 0,
            env: [("STUDIO_LOCAL_USER".to_string(), "1".to_string())].into(),
        };
        let port = start(config.clone()).expect("starts");
        assert!(port > 0);
        assert_eq!(start(config), Err(-3), "a second start is refused");
        assert!(stop());
        assert!(!stop(), "nothing left to stop");
    }

    #[test]
    fn a_host_without_sign_in_is_refused() {
        let config = crate::Config {
            listen: "127.0.0.1:0".parse().unwrap(),
            secret: "s".into(),
            trust_hops: 0,
            env: Default::default(),
        };
        assert_eq!(start(config), Err(-5));
    }
}
