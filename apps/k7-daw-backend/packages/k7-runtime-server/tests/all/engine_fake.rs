//! engine_fake.rs — `engine`'s safe API over the fake engine's table: the checks a table passes, jobs run on
//! the thread that runs the host and in order, answers come back, and nothing reaches a destroyed host.

use k7_runtime_server::engine::ffi::{K7_ENGINE_ABI, K7EngineApi};
use k7_runtime_server::engine::{AbiError, Api, Engine, Warning, fake};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;

/// A host created and run on a thread of its own, as the server's message thread; the handle comes back.
fn running() -> (Engine, thread::JoinHandle<()>) {
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let api = Api::new(fake::api()).unwrap();
        let host = api.create_host().unwrap();
        tx.send(host.engine()).unwrap();
        host.run();
    });
    (rx.recv().unwrap(), handle)
}

fn leaked(edit: impl FnOnce(&mut K7EngineApi)) -> &'static K7EngineApi {
    let real = fake::api();
    let mut copy = K7EngineApi { ..*real };
    edit(&mut copy);
    Box::leak(Box::new(copy))
}

#[test]
fn a_table_of_another_version_or_shorter_is_refused() {
    let other = leaked(|t| t.abi = K7_ENGINE_ABI + 1);
    assert_eq!(
        Api::new(other).err(),
        Some(AbiError::Version {
            engine: K7_ENGINE_ABI + 1,
            server: K7_ENGINE_ABI
        })
    );
    let short = leaked(|t| t.size -= 8);
    assert!(matches!(Api::new(short), Err(AbiError::Short { .. })));
    let none = leaked(|t| t.core = std::ptr::null());
    assert_eq!(Api::new(none).err(), Some(AbiError::NoCore));
    assert!(Api::new(fake::api()).unwrap().layout_consistent());
}

#[tokio::test]
async fn jobs_run_in_order_on_the_hosts_thread_and_answer() {
    let (engine, handle) = running();
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    for i in 0..20 {
        let seen = seen.clone();
        let engine = engine.clone();
        thread::spawn(move || assert!(engine.post(move |_| seen.lock().unwrap().push(i))))
            .join()
            .unwrap();
    }
    let ran_on = engine.call(|_| thread::current().id()).await.unwrap();
    assert_eq!(*seen.lock().unwrap(), (0..20).collect::<Vec<_>>());
    assert_eq!(
        Some(ran_on),
        fake::with_state(&engine, |s| s.run_thread).flatten()
    );
    assert_eq!(
        engine.call(|mt| mt.output_device_name()).await,
        Some(String::new())
    );
    engine.call(|mt| mt.open_audio_device()).await.unwrap();
    assert_eq!(
        fake::with_state(&engine, |s| s.calls.clone()).unwrap(),
        ["open_audio_device"]
    );
    engine.quit();
    handle.join().unwrap();
}

#[tokio::test]
async fn a_panicking_job_leaves_the_loop_running() {
    let (engine, handle) = running();
    assert!(engine.post(|_| panic!("a job that panics")));
    assert_eq!(engine.call(|_| 7).await, Some(7));
    engine.quit();
    handle.join().unwrap();
}

/// Counts its drops, so a test sees a job's closure released.
struct Drops(Arc<AtomicUsize>);

impl Drop for Drops {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn nothing_reaches_a_destroyed_host_and_queued_jobs_are_dropped() {
    let (engine, handle) = running();
    let drops = Arc::new(AtomicUsize::new(0));
    let held = Drops(drops.clone());
    engine.quit();
    handle.join().unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert!(!engine.post(move |_| drop(held)), "the host is gone");
    assert_eq!(
        drops.load(Ordering::SeqCst),
        1,
        "a refused job is released at once"
    );
    assert_eq!(engine.call(|_| 1).await, None);
    assert!(engine.session_ids().is_empty());
    assert!(!engine.has_session(""));
    assert!(engine.audio_timing().is_none());

    let (tx, rx) = mpsc::channel();
    let drops2 = drops.clone();
    thread::spawn(move || {
        let host = Api::new(fake::api()).unwrap().create_host().unwrap();
        let engine = host.engine();
        let held = Drops(drops2);
        assert!(engine.post(move |_| drop(held)));
        drop(host);
        tx.send(engine).unwrap();
    })
    .join()
    .unwrap();
    let late = rx.recv().unwrap();
    assert_eq!(
        drops.load(Ordering::SeqCst),
        2,
        "a job that never ran is dropped with its host"
    );
    assert!(!late.post(|_| {}));
}

#[tokio::test]
async fn the_any_thread_answers_read_the_host() {
    let (engine, handle) = running();
    fake::with_state(&engine, |s| {
        s.sessions = vec!["k7:a".into(), "slot".into()];
        s.timing.buffer_frames = 256;
        s.timing.sample_rate = 48000.0;
    });
    assert_eq!(engine.session_ids(), ["k7:a", "slot"]);
    assert!(engine.has_session("slot") && !engine.has_session("k7:b"));
    assert_eq!(
        engine
            .audio_timing()
            .map(|t| (t.buffer_frames, t.sample_rate)),
        Some((256, 48000.0))
    );
    assert_eq!(engine.message_thread_age_ms(), Some(0.0));
    engine.quit();
    handle.join().unwrap();
}

#[test]
fn the_process_calls_and_the_warning_sink() {
    let api = Api::new(fake::api()).unwrap();
    api.init_settings_dir("/tmp/k7-settings");
    assert_eq!(api.settings_dir(), "/tmp/k7-settings");
    assert!(!api.state_path().is_empty());
    api.set_safe_mode(true);
    assert!(fake::safe_mode());
    api.set_safe_mode(false);
    let (tx, rx) = mpsc::channel();
    let tx = std::sync::Mutex::new(tx);
    api.set_warning_sink(Some(Box::new(move |w: Warning| {
        tx.lock().unwrap().send(w).unwrap()
    })));
    fake::warn("k7:a", "chain:mod", "it broke");
    api.set_warning_sink(None);
    fake::warn("", "", "unheard");
    let got: Vec<Warning> = rx.try_iter().collect();
    assert_eq!(
        got,
        [Warning {
            session: "k7:a".into(),
            subject: "chain:mod".into(),
            message: "it broke".into()
        }]
    );
}
