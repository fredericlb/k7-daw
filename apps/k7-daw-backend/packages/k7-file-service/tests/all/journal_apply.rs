//! A step applied: text and blobs landing whole, all or nothing, confined, idempotent (1750 to 1755), the audio an engine keeps
//! (1740 to 1744), and what a failure part-way leaves. The disk is a fake where a real one cannot fail on demand.

use crate::support;

use std::cell::Cell;
use std::io::{self, Read};
use std::time::Duration;

use k7_file_service::atomic::{Fs, Real};
use k7_file_service::journal::apply::{ApplyError, Writer, apply_side};
use k7_file_service::journal::blobs::{BlobError, Side};
use k7_file_service::journal::format::{Change, EngineRef, Entry, Kind};
use k7_file_service::journal::store::Store;
use k7_file_service::journal::{Failure, Mode, Undone};
use support::journal::{Bed, ME, moved};

const A: fn() -> EngineRef = || EngineRef::new("e-a", "studio A");
const B: fn() -> EngineRef = || EngineRef::new("e-b", "studio B");

async fn bounce(b: &Bed, engine: Option<EngineRef>) {
    b.put("r.wav", "old");
    b.put("r.k7x", "v1");
    let begun =
        b.j.begin(
            &b.home,
            &b.root,
            Some(&b.dir),
            "clip process",
            Mode::Step,
            ME,
        )
        .await;
    b.j.snapshot(begun.id, &b.at("r.wav"));
    b.put("r.wav", "new");
    b.put("r.k7x", "v2");
    let ended = b.j.end(begun.id, engine).await;
    assert!(ended.failure.is_none(), "{:?}", ended.failure);
}

fn store(b: &Bed) -> Store {
    Store::new(k7_file_service::journal::store::store_for(&b.home, &b.dir))
}

#[tokio::test]
async fn refuses_an_undo_whose_audio_another_engine_keeps_naming_it_and_writes_nothing() {
    let b = Bed::new("ja-elsewhere");
    bounce(&b, Some(A())).await;
    // The engine took the copies: they are gone here.
    std::fs::remove_dir_all(store(&b).blobs_dir()).unwrap();
    let e = b.undo_with(false, Some(B()), None).await.unwrap_err();
    let Failure::Blob(blob) = e else {
        panic!("{e:?}")
    };
    assert_eq!(
        blob.to_string(),
        "The audio of proj.k7/r.wav for this step is kept by the engine \"studio A\", not by the one in use. Switch back to \"studio A\" to go past this step."
    );
    assert_eq!(
        (b.text("r.wav"), b.text("r.k7x")),
        ("new".into(), "v2".into())
    );
    // Back on the engine that keeps it, the step asks for the copies (they are fetched before the gate).
    let again = b.undo_with(false, Some(A()), None).await.unwrap();
    let Undone::Retry(missing) = again else {
        panic!("{again:?}")
    };
    assert_eq!(missing[0].name, "1-b0");
    assert_eq!(
        missing[0].gone(),
        BlobError::Gone {
            path: "proj.k7/r.wav".into(),
            kept: "studio A".into()
        }
    );
    assert_eq!(
        missing[0].gone().to_string(),
        "The engine \"studio A\" no longer has the audio of proj.k7/r.wav for this step: it keeps it for a month."
    );
}

#[tokio::test]
async fn says_a_step_with_no_engine_and_no_copy_is_no_longer_kept() {
    let b = Bed::new("ja-nolonger");
    bounce(&b, None).await;
    std::fs::remove_dir_all(store(&b).blobs_dir()).unwrap();
    let Failure::Blob(e) = b.undo_with(false, None, None).await.unwrap_err() else {
        panic!()
    };
    assert_eq!(
        e.to_string(),
        "The audio of proj.k7/r.wav for this step is no longer kept."
    );
}

#[tokio::test]
async fn a_changed_step_or_a_fetched_copy_is_applied_and_the_copy_deleted() {
    let b = Bed::new("ja-fetched");
    bounce(&b, Some(A())).await;
    let st = store(&b);
    // The engine holds the copy: it is gone here, and "fetched" back before the gate.
    let kept = std::fs::read(st.blob_path("1-b0")).unwrap();
    std::fs::remove_file(st.blob_path("1-b0")).unwrap();
    let (_, missing) =
        b.j.plan(&b.home, &b.root, &b.dir, false, Some(&A()))
            .unwrap();
    assert_eq!(missing.len(), 1);
    std::fs::write(&missing[0].file, kept).unwrap();
    // Another step on top than the one fetched for: start again.
    let confine = b.confine();
    let w = Writer {
        fs: &Real,
        retry: false,
        sleep: &std::thread::sleep,
    };
    let fetched = vec!["1-b0".to_string()];
    let mut u = k7_file_service::journal::Undo {
        home: &b.home,
        root: &b.root,
        dir: &b.dir,
        redo: false,
        expect: Some(99),
        bound: Some(A()),
        confine: &confine,
        writer: &w,
        fetched: &fetched,
    };
    assert!(matches!(b.j.undo(&u).await, Ok(Undone::Retry(_))));
    u.expect = Some(1);
    assert_eq!(moved(&b.j.undo(&u).await.unwrap()), "clip process");
    assert_eq!(b.text("r.wav"), "old");
    assert!(
        !std::path::Path::new(&st.blob_path("1-b0")).exists(),
        "the fetched copy was for that write only"
    );
}

#[tokio::test]
async fn nothing_to_undo_or_redo_is_said_so() {
    let b = Bed::new("ja-nothing");
    assert_eq!(b.undo().await, Undone::Nothing);
    assert_eq!(b.redo().await, Undone::Nothing);
}

fn entry(changes: Vec<Change>, blobs: Vec<Change>) -> Entry {
    Entry {
        id: 1,
        ts: 1.into(),
        kind: if blobs.is_empty() {
            Kind::Text
        } else {
            Kind::Process
        },
        label: "s".into(),
        changes,
        blobs,
        kept_on: None,
        order: vec![],
        rest: Default::default(),
    }
}

fn allow(root: String) -> impl Fn(&str) -> Result<String, String> {
    move |rel| Ok(format!("{root}/{rel}"))
}

#[test]
fn confines_every_path_before_the_first_write_and_refuses_a_climbing_one() {
    let b = Bed::new("ja-confine");
    let st = store(&b);
    let w = Writer {
        fs: &Real,
        retry: false,
        sleep: &std::thread::sleep,
    };
    for bad in ["../x", "a/../../x", "/etc/x", "a\\..\\x"] {
        let e = entry(
            vec![
                Change::new("ok.k7x", None, Some("fine".into())),
                Change::new(bad, None, Some("evil".into())),
            ],
            vec![],
        );
        let r = apply_side(&st, &e, Side::After, &allow(b.root.clone()), &w);
        assert_eq!(r, Err(ApplyError::Unsafe(bad.into())), "{bad}");
        assert!(
            !std::path::Path::new(&format!("{}/ok.k7x", b.root)).exists(),
            "{bad}: the first path was written"
        );
    }
}

#[test]
fn a_folder_replaced_by_a_link_cannot_carry_the_restore_outside_the_workspace() {
    let b = Bed::new("ja-link");
    let outside = b.s.dir("outside");
    std::fs::write(format!("{outside}/x.k7x"), "outside").unwrap();
    if support::link(&outside, &b.at("sub")).is_err() {
        return;
    }
    let st = store(&b);
    let w = Writer {
        fs: &Real,
        retry: false,
        sleep: &std::thread::sleep,
    };
    let e = entry(
        vec![
            Change::new("proj.k7/a.k7x", None, Some("first".into())),
            Change::new("proj.k7/sub/x.k7x", Some("o".into()), None),
        ],
        vec![],
    );
    let confine = b.confine();
    let r = apply_side(&st, &e, Side::After, &confine, &w);
    assert!(
        matches!(r, Err(ApplyError::Refused(ref m)) if m.contains("escapes")),
        "{r:?}"
    );
    assert_eq!(
        std::fs::read_to_string(format!("{outside}/x.k7x")).unwrap(),
        "outside"
    );
    assert!(!b.exists("a.k7x"), "a refusal writes nothing of the step");
}

/// A disk that fails the write of one named temp half way, as a full disk does, and refuses renames on demand.
struct Flaky {
    fail_nth_write: Cell<u32>,
    refuse_renames: Cell<u32>,
    renames: Cell<u32>,
}
unsafe impl Sync for Flaky {}

impl Fs for Flaky {
    fn write_temp(&self, tmp: &str, data: &mut dyn Read, mode: Option<u32>) -> io::Result<()> {
        let n = self.fail_nth_write.get();
        if n == 1 {
            // Half the bytes land in the temp, then the disk is full.
            let mut v = Vec::new();
            data.read_to_end(&mut v)?;
            std::fs::write(tmp, &v[..v.len() / 2])?;
            self.fail_nth_write.set(0);
            return Err(io::Error::from_raw_os_error(28));
        }
        if n > 1 {
            self.fail_nth_write.set(n - 1);
        }
        Real.write_temp(tmp, data, mode)
    }
    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        self.renames.set(self.renames.get() + 1);
        if self.refuse_renames.get() > 0 {
            self.refuse_renames.set(self.refuse_renames.get() - 1);
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        Real.rename(from, to)
    }
    fn remove(&self, p: &str) {
        Real.remove(p)
    }
    fn mode_of(&self, p: &str) -> Option<u32> {
        Real.mode_of(p)
    }
}

#[test]
fn a_disk_full_mid_blob_leaves_the_songs_file_whole_and_the_next_undo_finishes_the_step() {
    let b = Bed::new("ja-full");
    let st = store(&b);
    std::fs::create_dir_all(st.blobs_dir()).unwrap();
    std::fs::write(st.blob_path("1-b0"), "old audio").unwrap();
    b.put("a.k7x", "v2");
    b.put("r.wav", "new audio");
    let blob = Change {
        path: "proj.k7/r.wav".into(),
        before: Some("1-b0".into()),
        after: None,
        order: vec![],
        rest: Default::default(),
    };
    let e = entry(
        vec![Change::new(
            "proj.k7/a.k7x",
            Some("v1".into()),
            Some("v2".into()),
        )],
        vec![blob],
    );
    let confine = b.confine();
    let flaky = Flaky {
        fail_nth_write: Cell::new(2),
        refuse_renames: Cell::new(0),
        renames: Cell::new(0),
    };
    let w = Writer {
        fs: &flaky,
        retry: false,
        sleep: &std::thread::sleep,
    };
    let r = apply_side(&st, &e, Side::Before, &confine, &w);
    assert!(
        matches!(
            r,
            Err(ApplyError::Io { code: "EIO", .. }) | Err(ApplyError::Io { .. })
        ),
        "{r:?}"
    );
    assert_eq!(
        b.text("r.wav"),
        "new audio",
        "a torn copy never replaced the file"
    );
    assert_eq!(b.text("a.k7x"), "v1", "the text before it did land");
    let leftovers = std::fs::read_dir(&b.dir)
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        })
        .count();
    assert_eq!(leftovers, 0);
    // The side is idempotent: the same side again writes it all.
    let w = Writer {
        fs: &Real,
        retry: false,
        sleep: &std::thread::sleep,
    };
    apply_side(&st, &e, Side::Before, &confine, &w).unwrap();
    assert_eq!(
        (b.text("a.k7x"), b.text("r.wav")),
        ("v1".into(), "old audio".into())
    );
}

#[tokio::test]
async fn the_cursor_stays_where_it_was_when_the_files_failed() {
    let b = Bed::new("ja-cursor");
    b.put("a.k7x", "v1");
    b.step("edit", || b.put("a.k7x", "v2")).await;
    let confine = b.confine();
    let flaky = Flaky {
        fail_nth_write: Cell::new(1),
        refuse_renames: Cell::new(0),
        renames: Cell::new(0),
    };
    let w = Writer {
        fs: &flaky,
        retry: false,
        sleep: &std::thread::sleep,
    };
    let u = k7_file_service::journal::Undo {
        home: &b.home,
        root: &b.root,
        dir: &b.dir,
        redo: false,
        expect: None,
        bound: None,
        confine: &confine,
        writer: &w,
        fetched: &[],
    };
    assert!(b.j.undo(&u).await.is_err());
    assert_eq!(
        b.j.state_of(&b.home, &b.root, &b.dir)["canUndo"],
        serde_json::json!(true),
        "the cursor did not move"
    );
    assert_eq!(moved(&b.undo().await), "edit");
    assert_eq!(b.text("a.k7x"), "v1");
}

#[test]
fn a_rename_refused_twenty_times_is_given_up_on_and_a_few_times_waited_out_on_windows_rules() {
    let b = Bed::new("ja-rename");
    let st = store(&b);
    b.put("a.k7x", "v2");
    let e = entry(
        vec![Change::new(
            "proj.k7/a.k7x",
            Some("v1".into()),
            Some("v2".into()),
        )],
        vec![],
    );
    let confine = b.confine();
    let waited = std::sync::Mutex::new(Duration::ZERO);
    let sleep = |d: Duration| *waited.lock().unwrap() += d;
    let flaky = Flaky {
        fail_nth_write: Cell::new(0),
        refuse_renames: Cell::new(3),
        renames: Cell::new(0),
    };
    let w = Writer {
        fs: &flaky,
        retry: true,
        sleep: &sleep,
    };
    apply_side(&st, &e, Side::Before, &confine, &w).unwrap();
    assert_eq!(
        (
            b.text("a.k7x"),
            flaky.renames.get(),
            *waited.lock().unwrap()
        ),
        ("v1".into(), 4, Duration::from_millis(70))
    );
    let flaky = Flaky {
        fail_nth_write: Cell::new(0),
        refuse_renames: Cell::new(20),
        renames: Cell::new(0),
    };
    let w = Writer {
        fs: &flaky,
        retry: true,
        sleep: &sleep,
    };
    let r = apply_side(&st, &e, Side::After, &confine, &w);
    assert!(
        matches!(r, Err(ApplyError::Io { code: "EACCES", .. })),
        "{r:?}"
    );
    assert_eq!((flaky.renames.get(), b.text("a.k7x")), (9, "v1".into()));
}
