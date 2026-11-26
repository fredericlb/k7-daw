//! machine_socket.rs — the machine verbs over a real socket and the fake engine: sessions, drivers, the interface,
//! the meters, the instruments installed, and the audio broadcast down to its bytes. Replies are held byte for byte.

use crate::harness::{Server, Wire, eventually};
use futures_util::StreamExt;
use k7_runtime_server::engine::fake::machine::install_instrument;
use k7_runtime_server::engine::fake::{self, with_state};
use k7_runtime_server::engine::machine::{AudioDevice, InputDevice, SessionInfo};
use k7_runtime_server::server::ServeConfig;
use k7_runtime_server::wire::stream::{self, CODEC_PCM_F32, CODEC_PCM_S16, Chunk};
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

fn start() -> Server {
    Server::start(ServeConfig::new(0))
}

fn calls(s: &Server) -> Vec<String> {
    with_state(&s.engine, |st| st.calls.clone()).unwrap()
}

async fn saw(s: &Server, line: &str) {
    eventually(line, || calls(s).iter().any(|c| c == line)).await;
}

fn count(s: &Server, line: &str) -> usize {
    calls(s).iter().filter(|c| *c == line).count()
}

/// The next binary frame; text and pings are skipped.
async fn binary(w: &mut Wire) -> Vec<u8> {
    loop {
        match tokio::time::timeout(Duration::from_secs(5), w.0.next()).await {
            Ok(Some(Ok(Message::Binary(b)))) => return b.to_vec(),
            Ok(Some(Ok(_))) => {}
            other => panic!("expected a binary frame, got {other:?}"),
        }
    }
}

/// Nothing but pings arrives within a moment.
async fn quiet(w: &mut Wire) {
    let got = tokio::time::timeout(Duration::from_millis(300), w.0.next()).await;
    assert!(
        !matches!(got, Ok(Some(Ok(Message::Text(_) | Message::Binary(_))))),
        "{got:?}"
    );
}

fn session(id: &str, project: &str, loaded: bool, tracks: i32) -> SessionInfo {
    SessionInfo {
        id: id.into(),
        project: project.into(),
        loaded,
        playing: false,
        tracks,
    }
}

#[tokio::test]
async fn list_sessions_counts_each_sessions_listeners_and_hides_an_unloaded_name() {
    let s = start();
    with_state(&s.engine, |st| {
        st.machine.infos = vec![
            session("k7:a", "song", true, 3),
            session("k7:b", "stale", false, 0),
        ];
    });
    let mut w = s.connect().await;
    w.send(r#"{"type":"broadcast","on":true,"session":"k7:a"}"#)
        .await;
    saw(&s, "enable_broadcast k7:a 1").await;
    assert_eq!(
        w.ask(r#"{"type":"list-sessions"}"#).await,
        r#"{"sessions":[{"id":"k7:a","listeners":1,"loaded":true,"playing":false,"project":"song","tracks":3},{"id":"k7:b","listeners":0,"loaded":false,"playing":false,"project":"","tracks":0}],"type":"sessions"}"#
    );
}

#[tokio::test]
async fn drop_session_forgets_the_session_it_names() {
    let s = start();
    with_state(&s.engine, |st| {
        st.machine.infos = vec![session("k7:a", "x", true, 1)]
    });
    let mut w = s.connect().await;
    w.send(r#"{"type":"drop-session","session":"k7:a"}"#).await;
    saw(&s, "drop_session k7:a").await;
    assert!(with_state(&s.engine, |st| st.machine.infos.is_empty()).unwrap());
}

#[tokio::test]
async fn audio_cost_answers_in_milliseconds_and_reset_clears_the_window() {
    let s = start();
    with_state(&s.engine, |st| {
        st.machine.cost.mean_seconds = 0.001;
        st.machine.cost.max_seconds = 0.002;
        st.machine.cost.min_seconds = 0.0005;
        st.machine.cost.runs = 42;
        st.machine.cost.deadline_seconds = 0.01;
    });
    let mut w = s.connect().await;
    assert_eq!(
        w.ask(r#"{"type":"audio-cost"}"#).await,
        r#"{"deadlineMs":10.0,"maxMs":2.0,"meanMs":1.0,"minMs":0.5,"runs":42,"type":"audio-cost"}"#
    );
    let after = w.ask(r#"{"type":"audio-cost","reset":true}"#).await;
    assert_eq!(
        after,
        r#"{"deadlineMs":10.0,"maxMs":0.0,"meanMs":0.0,"minMs":0.0,"runs":0,"type":"audio-cost"}"#
    );
    assert_eq!(count(&s, "reset_callback_cost"), 1);
}

fn device(name: &str, ty: &str) -> AudioDevice {
    AudioDevice {
        name: name.into(),
        ty: ty.into(),
        inputs: 2,
        outputs: 2,
        current: false,
        current_input: false,
        is_default: false,
    }
}

#[tokio::test]
async fn a_device_list_goes_to_everyone_only_when_it_moved() {
    let s = start();
    with_state(&s.engine, |st| {
        st.machine.devices = vec![device("Built-in", "CoreAudio"), device("USB", "CoreAudio")]
    });
    let (mut a, mut b) = (s.connect().await, s.connect().await);
    // A socket hears the machine's broadcasts once the gate has seen it speak.
    b.ask(r#"{"type":"whoami"}"#).await;
    let first = a.ask(r#"{"type":"list-audio-devices"}"#).await;
    assert_eq!(
        first,
        r#"{"devices":[{"current":false,"currentInput":false,"inputs":2,"isDefault":false,"name":"Built-in","outputs":2,"type":"CoreAudio"},{"current":false,"currentInput":false,"inputs":2,"isDefault":false,"name":"USB","outputs":2,"type":"CoreAudio"}],"type":"audio-devices"}"#
    );
    // The first list moved from nothing, so the other socket hears it too.
    assert_eq!(b.text().await, first);
    // An unchanged list answers its asker alone.
    assert_eq!(a.ask(r#"{"type":"list-audio-devices"}"#).await, first);
    quiet(&mut b).await;
}

#[tokio::test]
async fn setting_a_device_tells_everyone_and_a_missing_one_only_the_asker() {
    let s = start();
    with_state(&s.engine, |st| {
        st.machine.devices = vec![device("Built-in", "CoreAudio"), device("USB", "ASIO")]
    });
    let (mut a, mut b) = (s.connect().await, s.connect().await);
    // A socket hears the machine's broadcasts once the gate has seen it speak.
    b.ask(r#"{"type":"whoami"}"#).await;
    a.send(r#"{"type":"set-audio-device","name":"USB","deviceType":"ASIO"}"#)
        .await;
    let told = a.text().await;
    assert!(
        told.contains(
            r#""current":true,"currentInput":true,"inputs":2,"isDefault":false,"name":"USB""#
        ),
        "{told}"
    );
    assert_eq!(b.text().await, told);
    assert!(calls(&s).contains(&"set_audio_device USB [ASIO]".to_string()));
    let refused = a
        .ask(r#"{"type":"set-audio-device","name":"Nope","session":"k7:a"}"#)
        .await;
    assert_eq!(
        refused,
        r#"{"message":"audio device not available: Nope","session":"k7:a","type":"error"}"#
    );
}

#[tokio::test]
async fn io_levels_meters_each_channel_set_and_stamps_the_session() {
    let s = start();
    with_state(&s.engine, |st| {
        st.machine.levels = vec![(-12.5, true)];
        st.machine.pitches = vec![(440.0, 0.75)];
        st.machine.master_db.insert("k7:a".into(), -6.0);
        st.machine.inputs = vec![InputDevice {
            name: "Scarlett".into(),
            channels: vec![1, 2],
            enabled: true,
        }];
    });
    let mut w = s.connect().await;
    let reply = w
        .ask(r#"{"type":"io-levels","session":"k7:a","channels":[[1,2],[9],"x",[3,"y",4.9]]}"#)
        .await;
    assert_eq!(
        reply,
        r#"{"clarity":[0.75,0.0,0.0],"devices":[{"channels":[1,2],"enabled":true,"name":"Scarlett"}],"found":[true,false,false],"inputs":[-12.5,-100.0,-100.0],"master":-6.0,"pitches":[440.0,0.0,0.0],"session":"k7:a","type":"io-levels"}"#
    );
    let sets = with_state(&s.engine, |st| st.machine.last_sets.clone()).unwrap();
    assert_eq!(sets, vec![vec![1, 2], vec![9], vec![3, 4]]);
}

#[tokio::test]
async fn grouping_inputs_without_an_interface_is_an_error_for_that_session() {
    let s = start();
    let mut w = s.connect().await;
    let reply = w
        .ask(r#"{"type":"set-input-stereo","channel":3,"stereo":true,"session":"k7:a"}"#)
        .await;
    assert_eq!(
        reply,
        r#"{"message":"no audio interface open to group inputs on","session":"k7:a","type":"error"}"#
    );
    with_state(&s.engine, |st| st.machine.interface_open = true);
    w.send(r#"{"type":"set-input-stereo","channel":3,"stereo":false}"#)
        .await;
    saw(&s, "set_input_stereo 3 0").await;
    quiet(&mut w).await;
}

#[tokio::test]
async fn instrument_status_names_each_ref_for_the_asker_alone() {
    install_instrument("@instruments/m8/piano.sfz");
    let s = start();
    let mut w = s.connect().await;
    let reply = w
        .ask(r#"{"type":"instrument-status","refs":["@instruments/m8/piano.sfz","@instruments/m8/none.sfz",7]}"#)
        .await;
    assert_eq!(
        reply,
        r#"{"status":{"@instruments/m8/none.sfz":false,"@instruments/m8/piano.sfz":true},"type":"instrument-status"}"#
    );
    assert_eq!(
        w.ask(r#"{"type":"instrument-status"}"#).await,
        r#"{"status":{},"type":"instrument-status"}"#
    );
}

#[tokio::test]
async fn a_driver_is_counted_once_told_the_standing_warnings_and_released_when_it_leaves() {
    fake::project::set_standing("k7:m8drv", &[("chain:fx", "slow")]);
    let s = start();
    let mut w = s.connect().await;
    let hello = r#"{"type":"hello","role":"driver","session":"k7:m8drv"}"#;
    w.send(hello).await;
    assert!(w.text().await.starts_with(r#"{"audioToBrowser":false"#));
    assert_eq!(
        w.text().await,
        r#"{"message":"slow","session":"k7:m8drv","subject":"chain:fx","type":"engine-warning"}"#
    );
    // The keepalive sends it again: still one driver, and no second warning.
    w.send(hello).await;
    assert!(w.text().await.contains("whoami"));
    saw(&s, "add_driver k7:m8drv").await;
    drop(w);
    saw(&s, "remove_driver k7:m8drv").await;
    assert_eq!(count(&s, "add_driver k7:m8drv"), 1);
}

/// `n` frames of a ramp, interleaved stereo.
fn ramp(n: usize) -> Vec<f32> {
    (0..n * 2).map(|i| (i as f32 - 3.0) / 8.0).collect()
}

fn push_tap(s: &Server, session: &str, samples: &[f32], rate: f64, lost: usize) {
    with_state(&s.engine, |st| {
        let tap = st.machine.taps.entry(session.into()).or_default();
        tap.samples.extend_from_slice(samples);
        tap.sample_rate = rate;
        tap.lost = lost;
    });
}

#[tokio::test]
async fn a_listener_hears_the_lstr_frame_byte_for_byte_with_a_sequence_that_jumps_over_a_hole() {
    let s = start();
    let mut w = s.connect().await;
    w.send(r#"{"type":"broadcast","on":true,"session":"k7:a"}"#)
        .await;
    saw(&s, "enable_broadcast k7:a 1").await;
    // The sender starts a first listener from live: the arrival is honoured before audio is pushed.
    saw(&s, "skip_broadcast_to_live k7:a").await;
    let samples = ramp(3);
    push_tap(&s, "k7:a", &samples, 48000.0, 0);
    let got = binary(&mut w).await;
    let want = stream::encode(
        &Chunk {
            samples: &samples,
            frames: 3,
            sample_rate: 48000.0,
        },
        1,
        0.0,
        CODEC_PCM_F32,
    );
    // Everything but the capture time, which counts from the engine's boot.
    assert_eq!(got[..24], want[..24]);
    assert_eq!(got[32..], want[32..]);
    let capture = f64::from_le_bytes(got[24..32].try_into().unwrap());
    assert!(capture > 0.0 && capture < 60_000.0, "{capture}");
    // A hole of 1000 lost frames rounds up to 3 blocks of 480: the next frame is 4 further on.
    push_tap(&s, "k7:a", &samples, 48000.0, 1000);
    let got = binary(&mut w).await;
    assert_eq!(
        u32::from_le_bytes(got[16..20].try_into().unwrap()),
        1 + 1 + 3
    );
}

#[tokio::test]
async fn a_pcm16_listener_hears_half_the_bytes_and_only_its_own_session() {
    let s = start();
    let (mut a, mut b) = (s.connect().await, s.connect().await);
    a.send(r#"{"type":"broadcast","on":true,"session":"k7:a","codec":"pcm16"}"#)
        .await;
    b.send(r#"{"type":"broadcast","on":true,"session":"k7:b"}"#)
        .await;
    saw(&s, "skip_broadcast_to_live k7:b").await;
    saw(&s, "skip_broadcast_to_live k7:a").await;
    let samples = ramp(2);
    push_tap(&s, "k7:a", &samples, 44100.0, 0);
    let got = binary(&mut a).await;
    let want = stream::encode(
        &Chunk {
            samples: &samples,
            frames: 2,
            sample_rate: 44100.0,
        },
        1,
        0.0,
        CODEC_PCM_S16,
    );
    assert_eq!(got.len(), 32 + 8);
    assert_eq!(got[..24], want[..24]);
    assert_eq!(got[32..], want[32..]);
    quiet(&mut b).await;
}

#[tokio::test]
async fn a_tap_with_no_sample_rate_is_withheld() {
    let s = start();
    let mut w = s.connect().await;
    w.send(r#"{"type":"broadcast","on":true,"session":"k7:a"}"#)
        .await;
    saw(&s, "skip_broadcast_to_live k7:a").await;
    push_tap(&s, "k7:a", &ramp(2), 0.0, 0);
    quiet(&mut w).await;
}

#[tokio::test]
async fn the_tap_goes_off_after_the_last_listener_and_a_silent_one_is_remembered() {
    let s = start();
    let (mut a, mut b) = (s.connect().await, s.connect().await);
    a.send(r#"{"type":"broadcast","on":true,"session":"k7:a","silent":true}"#)
        .await;
    saw(&s, "silence_output k7:a 1").await;
    b.send(r#"{"type":"broadcast","on":true,"session":"k7:a"}"#)
        .await;
    eventually("two ears", || count(&s, "enable_broadcast k7:a 1") == 2).await;
    // One ear left, and it is silent: the tap stays on.
    drop(b);
    eventually("b gone", || s.ctx.bindings().listeners.len() == 1).await;
    assert_eq!(count(&s, "enable_broadcast k7:a 0"), 0);
    // A closed recorder tab sends no goodbye: the interface plays again once no silent ear is left.
    drop(a);
    saw(&s, "enable_broadcast k7:a 0").await;
    eventually("silence lifted", || {
        calls(&s)
            .iter()
            .rev()
            .find(|c| c.starts_with("silence_output k7:a"))
            == Some(&"silence_output k7:a 0".to_string())
    })
    .await;
}

#[tokio::test]
async fn off_retires_the_sockets_own_subscription_and_moving_leaves_the_old_session() {
    let s = start();
    let mut w = s.connect().await;
    w.send(r#"{"type":"broadcast","on":true,"session":"k7:a"}"#)
        .await;
    w.send(r#"{"type":"broadcast","on":true,"session":"k7:b"}"#)
        .await;
    // Moving leaves k7:a, whose tap goes off with nobody else on it.
    saw(&s, "enable_broadcast k7:a 0").await;
    saw(&s, "enable_broadcast k7:b 1").await;
    // Off names another session and still ends the socket's own.
    w.send(r#"{"type":"broadcast","on":false,"session":"k7:zzz"}"#)
        .await;
    saw(&s, "enable_broadcast k7:b 0").await;
    assert!(s.ctx.bindings().listeners.is_empty());
}
