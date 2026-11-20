//! k7-front's binary: the front configured from the environment (`K7_FRONT_*`), stopped by a signal or a console
//! event, each named in the log as it arrives, as is the stop's end (`k7_front::exit` for a panic or an abort).

#[tokio::main]
async fn main() {
    k7_front::exit::watch(None);
    let config = match k7_front::Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("k7-front: {e}");
            std::process::exit(2);
        }
    };
    let ready = move |addr| println!("k7-front on http://{addr}");
    if let Err(e) = k7_front::serve(config, shutdown(), ready).await {
        eprintln!("k7-front: {e}");
        std::process::exit(1);
    }
    eprintln!("k7-front: stopped");
}

/// Resolves on the first stop the process is sent, after saying which: a front gone quiet must say who stopped it.
async fn shutdown() {
    let why = signalled().await;
    eprintln!("k7-front: stopping on {why}");
}

#[cfg(unix)]
async fn signalled() -> &'static str {
    use tokio::signal::unix::{SignalKind, signal};
    let (Ok(mut term), Ok(mut hup)) = (
        signal(SignalKind::terminate()),
        signal(SignalKind::hangup()),
    ) else {
        let _ = tokio::signal::ctrl_c().await;
        return "SIGINT";
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => "SIGINT",
        _ = term.recv() => "SIGTERM",
        _ = hup.recv() => "SIGHUP",
    }
}

#[cfg(windows)]
async fn signalled() -> &'static str {
    use tokio::signal::windows::{ctrl_break, ctrl_c, ctrl_close, ctrl_logoff, ctrl_shutdown};
    let (Ok(mut c), Ok(mut b), Ok(mut close), Ok(mut off), Ok(mut down)) = (
        ctrl_c(),
        ctrl_break(),
        ctrl_close(),
        ctrl_logoff(),
        ctrl_shutdown(),
    ) else {
        let _ = tokio::signal::ctrl_c().await;
        return "Ctrl-C";
    };
    // A console's Ctrl-C reaches every process attached to it: one sent to a bed's console stops its front too.
    tokio::select! {
        _ = c.recv() => "Ctrl-C",
        _ = b.recv() => "Ctrl-Break",
        _ = close.recv() => "the console closing",
        _ = off.recv() => "a logoff",
        _ = down.recv() => "a system shutdown",
    }
}
