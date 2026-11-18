// bundle_absent.rs — what a disk answered NotFound, Outside or a binary length, the bundle answers the same (795, 743, 745).
// A reader would get wrong that the fakes give Windows' answers on purpose: a Mac cannot be made to answer them, a Source can.

use crate::support;

use song::record::Recorder;
use song::{DiskSource, Entry, Kind, MemSource, Source, SourceBundle, SourceError, Span, Stamp, Stat};
use std::collections::BTreeMap;

/// A Source with a fixed answer per path and call; whatever is not named is NotFound.
#[derive(Default)]
struct Fake {
    stats: BTreeMap<&'static str, Result<(Kind, u64), SourceError>>,
    lists: BTreeMap<&'static str, Result<Vec<&'static str>, SourceError>>,
    reads: BTreeMap<&'static str, Result<Vec<u8>, SourceError>>,
}

impl Source for Fake {
    fn stat(&self, path: &str) -> Result<Stat, SourceError> {
        match self.stats.get(path) {
            Some(Ok((kind, len))) => Ok(Stat {
                kind: *kind,
                len: *len,
                stamp: Stamp {
                    mtime_ns: 1,
                    len: *len,
                    settled: true,
                },
            }),
            Some(Err(e)) => Err(e.clone()),
            None => Err(SourceError::NotFound),
        }
    }

    fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError> {
        match self.lists.get(dir) {
            Some(Ok(names)) => Ok(names
                .iter()
                .map(|n| Entry {
                    name: (*n).to_owned(),
                    kind: Kind::File,
                })
                .collect()),
            Some(Err(e)) => Err(e.clone()),
            None => Err(SourceError::NotFound),
        }
    }

    fn read(&self, path: &str, _span: Span) -> Result<Vec<u8>, SourceError> {
        self.reads.get(path).cloned().unwrap_or(Err(SourceError::NotFound))
    }
}

fn replay(b: SourceBundle) -> MemSource {
    let json = serde_json::to_string(&b).expect("json");
    MemSource::from_bundle(serde_json::from_str(&json).expect("back")).expect("a source")
}

// Decision 795: Windows answers NotFound for a name below a file and Io for the file as a folder.
#[test]
fn a_name_below_a_file_as_windows_answers_it_replays_as_not_found() {
    let sub = "/p/clips/a.wav/sub.wav";
    let mut fake = Fake::default();
    fake.stats.insert("/p/clips/a.wav", Ok((Kind::File, 4)));
    fake.lists.insert("/p/clips", Ok(vec!["a.wav"]));
    fake.lists
        .insert("/p/clips/a.wav", Err(SourceError::Io("not a folder".into())));
    let rec = Recorder::new(&fake);
    assert_eq!(rec.stat(sub), Err(SourceError::NotFound));
    assert_eq!(rec.read(sub, Span::All), Err(SourceError::NotFound));
    assert_eq!(rec.list(sub), Err(SourceError::NotFound));
    let b = rec.into_bundle(&["/p".to_owned()]);
    assert!(b.absent.contains(sub), "{:?}", b.absent);
    let mem = replay(b);
    assert_eq!(mem.stat(sub).err(), Some(SourceError::NotFound));
    assert_eq!(mem.read(sub, Span::All), Err(SourceError::NotFound));
    assert_eq!(mem.list(sub), Err(SourceError::NotFound));
}

// Decision 795: a listing held above that omits the name already proves it, so the bundle does not repeat it.
#[test]
fn an_absent_path_a_held_listing_proves_is_not_repeated() {
    let mut fake = Fake::default();
    fake.lists.insert("/p/clips", Ok(vec!["a.wav"]));
    let rec = Recorder::new(&fake);
    assert_eq!(rec.stat("/p/clips/b.wav"), Err(SourceError::NotFound));
    let b = rec.into_bundle(&["/p".to_owned()]);
    assert!(b.absent.is_empty() && b.lists.contains_key("/p/clips"), "{b:?}");
    assert_eq!(replay(b).stat("/p/clips/b.wav").err(), Some(SourceError::NotFound));
}

// Decision 795: a folder that can be entered but not read, and a file below it that is not there.
#[cfg(unix)]
#[test]
fn a_file_in_a_folder_that_will_not_list_replays_as_the_disk_answered() {
    use std::os::unix::fs::PermissionsExt;
    let t = support::Tmp::new("absent-closed");
    t.mkdir("p/closed");
    let dir = t.0.join("p/closed");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o311)).expect("chmod");
    let readable = std::fs::read_dir(&dir).is_ok();
    let project = t.at("p");
    let path = t.at("p/closed/absent.k7x");
    let disk = DiskSource::new(std::slice::from_ref(&project)).expect("a source");
    let rec = Recorder::new(&disk);
    let on_disk = (rec.stat(&path).err(), rec.read(&path, Span::All).err());
    let b = rec.into_bundle(std::slice::from_ref(&project));
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    if readable {
        support::skip(
            "a_file_in_a_folder_that_will_not_list",
            "this user can list a mode-0311 folder",
        );
        return;
    }
    assert_eq!(on_disk, (Some(SourceError::NotFound), Some(SourceError::NotFound)));
    assert!(b.absent.contains(&path), "{:?}", b.absent);
    let mem = replay(b);
    assert_eq!((mem.stat(&path).err(), mem.read(&path, Span::All).err()), on_disk);
}

// Decision 795: a mount whose folder is not there answers NotFound to all three calls, not an empty folder.
#[test]
fn a_folder_mount_that_is_not_there_replays_as_not_found_for_every_call() {
    let t = support::Tmp::new("absent-mount");
    t.mkdir("ws/proj");
    let (project, gone) = (t.at("ws/proj"), t.at("ws/gone"));
    let mounts = [project, gone.clone()];
    let disk = DiskSource::new(&mounts).expect("a source");
    let rec = Recorder::new(&disk);
    let on_disk = (
        rec.stat(&gone).err(),
        rec.list(&gone).err(),
        rec.read(&gone, Span::All).err(),
    );
    assert_eq!(
        on_disk,
        (
            Some(SourceError::NotFound),
            Some(SourceError::NotFound),
            Some(SourceError::NotFound)
        )
    );
    let b = rec.into_bundle(&mounts);
    assert!(b.absent.contains(&gone), "{:?}", b.absent);
    let mem = replay(b);
    let again = (
        mem.stat(&gone).err(),
        mem.list(&gone).err(),
        mem.read(&gone, Span::All).err(),
    );
    assert_eq!(again, on_disk);
}

// Decision 743: a refused folder with a mount below it is kept for itself and answers nothing for the mount.
#[test]
fn a_refusal_with_a_mount_below_it_replays_for_the_folder_alone() {
    let mut fake = Fake::default();
    fake.stats.insert("/ws/out", Err(SourceError::Outside));
    fake.lists.insert("/ws/out", Err(SourceError::Outside));
    fake.reads.insert("/ws/out", Err(SourceError::Outside));
    let mounts = ["/ws".to_owned(), "/ws/out/proj".to_owned()];
    let rec = Recorder::new(&fake);
    assert_eq!(rec.stat("/ws/out").err(), Some(SourceError::Outside));
    assert_eq!(rec.list("/ws/out").err(), Some(SourceError::Outside));
    assert_eq!(rec.read("/ws/out", Span::All).err(), Some(SourceError::Outside));
    let b = rec.into_bundle(&mounts);
    assert!(b.refused.contains("/ws/out"), "{:?}", b.refused);
    let mem = replay(b);
    assert_eq!(mem.stat("/ws/out").err(), Some(SourceError::Outside));
    assert_eq!(mem.list("/ws/out").err(), Some(SourceError::Outside));
    assert_eq!(mem.read("/ws/out", Span::All).err(), Some(SourceError::Outside));
    // The mount below is not refused by it, and a path with no mount below still is.
    assert_ne!(mem.stat("/ws/out/proj").err(), Some(SourceError::Outside));
}

// Decision 745: a file seen at length 0 and then read as bytes that are not UTF-8 keeps the length of the bytes.
#[test]
fn a_file_stat_at_length_zero_and_read_as_binary_keeps_the_length_it_was_read_at() {
    let mut fake = Fake::default();
    fake.stats.insert("/p/a.bin", Ok((Kind::File, 0)));
    fake.reads.insert("/p/a.bin", Ok(vec![0xff, 0xfe, 0x00]));
    let rec = Recorder::new(&fake);
    rec.stat("/p/a.bin").expect("a stat");
    rec.read("/p/a.bin", Span::All).expect("bytes");
    let b = rec.into_bundle(&["/p".to_owned()]);
    let f = &b.files["/p/a.bin"];
    assert_eq!((f.len, f.binary), (Some(3), Some(true)));
    let mem = replay(b);
    assert_eq!(mem.stat("/p/a.bin").expect("a stat").len, 3);
    assert_eq!(mem.read("/p/a.bin", Span::All), Ok(vec![0xff]));
}
