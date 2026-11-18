//! The journal's bytes and its store: a round trip that keeps every byte and unknown key (1721), numbers as `String(n)`,
//! a second writer read again at the next use (1722), a torn file set aside and never overwritten (1754), atomic writes (1750).

use crate::support;

use k7_file_service::atomic::{Fs, is_temp, rename_retrying, write_atomic_on};
use k7_file_service::filter::code_of;
use k7_file_service::journal::format::{Journal, Kind, js_number, parse, print};
use k7_file_service::journal::lossy::Lossy;
use k7_file_service::journal::ring::{NewEntry, push};
use k7_file_service::journal::store::{Held, Loaded, Store};
use std::cell::Cell;
use std::io::{self, Read};
use std::time::Duration;
use support::Scratch;

fn change(p: &str, after: &str) -> k7_file_service::journal::format::Change {
    k7_file_service::journal::format::Change::new(p, None, Some(after.into()))
}

fn one(label: &str) -> NewEntry {
    NewEntry {
        label: label.into(),
        changes: vec![change("a.k7x", "A")],
        blobs: vec![],
    }
}

#[test]
fn keeps_every_byte_and_unknown_key_of_a_journal_it_read() {
    let text = concat!(
        r#"{"9":1,"nextId":3,"future":{"b":[1,2.5,"x"],"a":null},"cursor":1,"entries":[{"id":2,"ts":1.5,"kind":"process","label":"l \"","#,
        r#""changes":[{"note":1,"path":"a","before":null,"after":"A"}],"blobs":[{"path":"r.wav","before":"2-b0","after":null,"x":{"1":1}}],"#,
        r#""keptOn":{"name":"n","id":"i","z":0},"2":5}]}"#
    );
    let j = parse(text).unwrap();
    // Integer-like keys list first, as JavaScript lists them; the rest where the file had them.
    let want = text
        .replace(r#","2":5}]}"#, "}]}")
        .replace(r#"{"id":2,"#, r#"{"2":5,"id":2,"#);
    assert_eq!(print(&j), want);
}

#[test]
fn prints_numbers_as_string_of_n_does() {
    for (x, s) in [
        (0.0, "0"),
        (-0.0, "0"),
        (1.0, "1"),
        (1.5, "1.5"),
        (1e21, "1e+21"),
        (1e20, "100000000000000000000"),
        (123456789012345680000.0, "123456789012345680000"),
        (0.000001, "0.000001"),
        (1e-7, "1e-7"),
        (5e-324, "5e-324"),
        (1.7976931348623157e308, "1.7976931348623157e+308"),
        (-2.5, "-2.5"),
        (0.1 + 0.2, "0.30000000000000004"),
        (1700000000000.0, "1700000000000"),
    ] {
        assert_eq!(js_number(x), s, "{x:e}");
    }
}

#[test]
fn a_new_journal_prints_in_the_literals_order_and_a_text_step_has_no_kept_on() {
    let mut j = Journal::empty();
    push(&mut j, one("edit a"), 1, 1000, None).unwrap();
    assert_eq!(
        print(&j),
        r#"{"nextId":2,"cursor":1,"entries":[{"id":1,"ts":1,"kind":"text","label":"edit a","changes":[{"path":"a.k7x","before":null,"after":"A"}],"blobs":[]}]}"#
    );
    assert_eq!(j.entries[0].kind, Kind::Text);
    assert_eq!(j.entries[0].kept_on, None);
}

#[test]
fn reads_a_second_writers_journal_at_the_next_use() {
    let s = Scratch::new("jf-second");
    let store = Store::new(s.path("store"));
    let mut mine = Held::read(&store);
    push(&mut mine.journal, one("mine"), 1, 1000, None);
    mine.save(&store, 1).unwrap();
    assert!(!mine.refresh(&store), "nothing moved since my own save");
    // Another process writes a longer journal; the stamp tells, and this one reads it again.
    let mut other = Held::read(&store);
    push(&mut other.journal, one("theirs"), 2, 1000, None);
    other.save(&store, 2).unwrap();
    assert!(mine.refresh(&store));
    assert_eq!(
        mine.journal
            .entries
            .iter()
            .map(|e| e.label.as_str())
            .collect::<Vec<_>>(),
        ["mine", "theirs"]
    );
}

#[test]
fn sets_a_torn_journal_aside_before_the_next_step_overwrites_it() {
    let s = Scratch::new("jf-torn");
    let store = Store::new(s.path("store"));
    s.file("store/journal.json", r#"{"nextId":2,"cur"#);
    let mut held = Held::read(&store);
    assert!(held.torn.is_some() && held.journal.entries.is_empty());
    // Reading changed nothing on disk.
    assert!(std::path::Path::new(&store.journal_path()).exists());
    push(&mut held.journal, one("later"), 5, 1000, None);
    let aside = held.save(&store, 77).unwrap().expect("set aside");
    assert!(aside.ends_with("journal.torn-77.json"), "{aside}");
    assert_eq!(
        std::fs::read_to_string(aside).unwrap(),
        r#"{"nextId":2,"cur"#
    );
    assert!(matches!(store.load(), Loaded::Journal(j, _, _) if j.entries.len() == 1));
}

fn journal_with_label(label: &[u8]) -> Vec<u8> {
    let mut v =
        br#"{"nextId":2,"cursor":1,"entries":[{"id":1,"ts":1,"kind":"text","label":""#.to_vec();
    v.extend_from_slice(label);
    v.extend_from_slice(br#"","changes":[],"blobs":[]}]}"#);
    v
}

fn label_of(loaded: Loaded) -> (String, Lossy) {
    match loaded {
        Loaded::Journal(j, _, lossy) => (j.entries[0].label.clone(), lossy),
        other => panic!("read lossily, never torn: {other:?}"),
    }
}

// 1796: bad UTF-8 and a lone-surrogate escape are U+FFFD, counted, and the steps around them are kept.
#[test]
fn a_journal_with_bad_utf8_or_a_lone_surrogate_is_read_lossily_and_counted() {
    let s = Scratch::new("jf-lossy");
    let store = Store::new(s.path("store"));
    std::fs::create_dir_all(s.path("store")).unwrap();
    let put = |bytes: &[u8]| {
        std::fs::write(s.path("store/journal.json"), journal_with_label(bytes)).unwrap()
    };
    put(&[b'a', 0xff, b'b']);
    let (label, lossy) = label_of(store.load());
    assert_eq!(
        (label.as_str(), lossy.utf8, lossy.surrogate),
        ("a\u{fffd}b", true, false)
    );
    for (esc, want) in [
        (r"x\ud800y", "x\u{fffd}y"),
        (r"x\udc00y", "x\u{fffd}y"),
        (r"x\ud800\ud800y", "x\u{fffd}\u{fffd}y"),
        (r"x\uD83D\u0041", "x\u{fffd}A"),
        (r"x\ud83d\ude00y", "x\u{1f600}y"),
        (r"x\\ud800y", "x\\ud800y"),
    ] {
        put(esc.as_bytes());
        let (label, lossy) = label_of(store.load());
        assert_eq!(label, want, "{esc}");
        assert_eq!(lossy.surrogate, want.contains('\u{fffd}'), "{esc}");
    }
    // Both at once, and the held journal carries the count.
    put(&[0xff, b'\\', b'u', b'd', b'8', b'0', b'0']);
    let held = Held::read(&store);
    assert!(held.torn.is_none() && held.lossy.utf8 && held.lossy.surrogate);
    assert_eq!(held.journal.entries[0].label, "\u{fffd}\u{fffd}");
    assert!(Held::read(&Store::new(s.path("none"))).lossy == Lossy::default());
}

#[test]
fn leaves_no_temp_beside_a_journal_and_sweeps_a_crashs() {
    let s = Scratch::new("jf-sweep");
    let store = Store::new(s.path("store"));
    let mut held = Held::read(&store);
    push(&mut held.journal, one("x"), 1, 1000, None);
    held.save(&store, 1).unwrap();
    let names = || -> Vec<String> {
        std::fs::read_dir(&store.dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect()
    };
    assert!(!names().iter().any(|n| is_temp(n)), "{:?}", names());
    s.file("store/.journal.json.1.0000abcd.tmp", "half");
    s.file("store/staged/old", "x");
    assert_eq!(
        store.sweep(Duration::ZERO),
        1,
        "a fresh temp is kept, a staged file past its age goes"
    );
    assert!(
        is_temp(".a.b.12.0123abcf.tmp")
            && !is_temp("a.12.0123abcf.tmp")
            && !is_temp(".a.12.0123ABCF.tmp")
            && !is_temp(".12.0123abcf.tmp")
    );
}

#[test]
fn a_rename_refused_is_retried_on_windows_only_and_a_failed_write_leaves_no_temp() {
    struct Fake {
        refuse: Cell<u32>,
        full: bool,
        renames: Cell<u32>,
        removed: Cell<u32>,
    }
    // The Cells are touched by one thread.
    unsafe impl Sync for Fake {}
    impl Fs for Fake {
        fn write_temp(&self, _t: &str, d: &mut dyn Read, _m: Option<u32>) -> io::Result<()> {
            let mut v = Vec::new();
            d.read_to_end(&mut v)?;
            if self.full {
                Err(io::Error::from_raw_os_error(28))
            } else {
                Ok(())
            }
        }
        fn rename(&self, _f: &str, _t: &str) -> io::Result<()> {
            self.renames.set(self.renames.get() + 1);
            if self.refuse.get() > 0 {
                self.refuse.set(self.refuse.get() - 1);
                return Err(io::Error::from(io::ErrorKind::PermissionDenied));
            }
            Ok(())
        }
        fn remove(&self, _p: &str) {
            self.removed.set(self.removed.get() + 1);
        }
        fn mode_of(&self, _p: &str) -> Option<u32> {
            None
        }
    }
    let fake = |refuse, full| Fake {
        refuse: Cell::new(refuse),
        full,
        renames: Cell::new(0),
        removed: Cell::new(0),
    };
    let waited = Cell::new(Duration::ZERO);
    let sleep = |d: Duration| waited.set(waited.get() + d);
    // Windows' path: refused twice, granted the third time, waits 10 then 20 ms.
    let f = fake(2, false);
    write_atomic_on(&f, "/x/a", &mut &b"x"[..], true, &sleep).unwrap();
    assert_eq!(
        (f.renames.get(), waited.get()),
        (3, Duration::from_millis(30))
    );
    // Refused twenty times: given up after 8 retries, the temp removed.
    let f = fake(20, false);
    let e = write_atomic_on(&f, "/x/a", &mut &b"x"[..], true, &sleep).unwrap_err();
    assert_eq!(
        (code_of(&e), f.renames.get(), f.removed.get()),
        ("EACCES", 9, 1)
    );
    // Elsewhere one refusal is the answer.
    let f = fake(1, false);
    assert!(write_atomic_on(&f, "/x/a", &mut &b"x"[..], false, &sleep).is_err());
    assert_eq!(f.renames.get(), 1);
    // A full disk fails the write and removes the temp; nothing was renamed.
    let f = fake(0, true);
    assert!(write_atomic_on(&f, "/x/a", &mut &b"x"[..], true, &sleep).is_err());
    assert_eq!((f.renames.get(), f.removed.get()), (0, 1));
    let _ = rename_retrying;
}

#[cfg(unix)]
#[test]
fn keeps_the_mode_of_the_file_it_replaces() {
    use std::os::unix::fs::PermissionsExt;
    let s = Scratch::new("jf-mode");
    let p = s.file("a.json", "{}");
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o640)).unwrap();
    k7_file_service::atomic::write_atomic(&p, b"[]").unwrap();
    assert_eq!(
        std::fs::metadata(&p).unwrap().permissions().mode() & 0o7777,
        0o640
    );
    assert_eq!(std::fs::read_to_string(&p).unwrap(), "[]");
}
