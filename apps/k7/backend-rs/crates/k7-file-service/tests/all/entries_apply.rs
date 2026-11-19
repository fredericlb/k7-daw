//! The disk's part of a write (1230 to 1235): the retry, the bin's names, the upload's halves, a link moved as itself,
//! the shim's two tables held equal to the crate's, and the race the TypeScript leaves open.

use crate::support;

use std::cell::Cell;
use std::io;
use std::time::Duration;

use k7_file_service::entries::apply::*;
use k7_file_service::entries::plan::plan;
use k7_file_service::entries::{EntryError, Op, Outcome};
use k7_file_service::filter::skip;
use k7_file_service::space::Space;
use support::entries::Fx;
use support::{can_link, link_file};

const MS: Duration = Duration::from_millis(1);

/// A rename that fails `n` times with `kind`, then succeeds; counts what it was asked.
struct Flaky {
    left: Cell<u32>,
    asked: Cell<u32>,
    kind: io::ErrorKind,
}

impl Flaky {
    fn new(n: u32, kind: io::ErrorKind) -> Flaky {
        Flaky {
            left: Cell::new(n),
            asked: Cell::new(0),
            kind,
        }
    }
}

impl Renamer for Flaky {
    fn rename(&self, _: &str, _: &str) -> io::Result<()> {
        self.asked.set(self.asked.get() + 1);
        if self.left.get() == 0 {
            return Ok(());
        }
        self.left.set(self.left.get() - 1);
        Err(io::Error::from(self.kind))
    }
}

#[test]
fn a_rename_is_retried_while_the_code_is_transient_and_not_a_time_longer() {
    for n in [0, 1, 24, 25] {
        let r = Flaky::new(n, io::ErrorKind::PermissionDenied);
        assert_eq!(
            rename_with_retry(&r, "a", "b", RETRY.0, MS),
            Ok(()),
            "{n} failures"
        );
        assert_eq!(r.asked.get(), n + 1);
    }
    let r = Flaky::new(26, io::ErrorKind::PermissionDenied);
    let got = rename_with_retry(&r, "a", "b", RETRY.0, MS);
    assert_eq!(
        got,
        Err(EntryError::Io {
            code: "EACCES",
            syscall: "rename",
            path: "a".into()
        })
    );
    assert_eq!(r.asked.get(), 26);
    // A code that is not transient is the first answer.
    let r = Flaky::new(5, io::ErrorKind::NotFound);
    assert!(matches!(
        rename_with_retry(&r, "a", "b", RETRY.0, MS),
        Err(EntryError::Io { code: "ENOENT", .. })
    ));
    assert_eq!(r.asked.get(), 1);
    assert_eq!(RETRY, (25, Duration::from_millis(100)));
    for c in ["EPERM", "EBUSY", "EACCES", "ENOTEMPTY"] {
        assert!(is_transient(c), "{c}");
    }
    for c in ["ENOENT", "EEXIST", "EIO", "EXDEV"] {
        assert!(!is_transient(c), "{c}");
    }
}

#[test]
fn a_rename_waits_out_a_handle_held_without_sharing() {
    let fx = Fx::new("a-handle");
    let dir = fx.s.dir("studio/workspace/songs/held");
    let inner = fx.s.file("studio/workspace/songs/held/f.txt", "x");
    let to = fx.at("moved");
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        let handle = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&inner)
            .unwrap();
        let letting_go = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            drop(handle);
        });
        assert_eq!(
            rename_with_retry(&Disk, &dir, &to, RETRY.0, RETRY.1),
            Ok(())
        );
        letting_go.join().unwrap();
    }
    #[cfg(not(windows))]
    {
        let _ = &inner;
        let r = Flaky::new(3, io::ErrorKind::ResourceBusy);
        assert_eq!(rename_with_retry(&r, &dir, &to, RETRY.0, MS), Ok(()));
        std::fs::rename(&dir, &to).unwrap();
    }
    assert!(std::fs::metadata(format!("{to}/f.txt")).is_ok());
}

#[test]
fn the_bin_hands_out_the_first_free_name() {
    let fx = Fx::new("a-bin");
    let bin = fx.at("_recycle_bin");
    assert_eq!(recycle_target(&bin, "a"), format!("{bin}/a"));
    fx.s.file("studio/workspace/songs/_recycle_bin/a", "");
    assert_eq!(recycle_target(&bin, "a"), format!("{bin}/a-2"));
    fx.s.file("studio/workspace/songs/_recycle_bin/a-3", "");
    assert_eq!(
        recycle_target(&bin, "a"),
        format!("{bin}/a-2"),
        "the first free, not the last taken"
    );
    fx.s.file("studio/workspace/songs/_recycle_bin/a-2", "");
    assert_eq!(recycle_target(&bin, "a"), format!("{bin}/a-4"));
}

#[test]
fn a_folder_is_made_whole_and_a_file_in_the_way_is_an_io_failure() {
    let fx = Fx::new("a-mkdir");
    let deep = fx.at("x/y/z");
    assert_eq!(make_folder(&deep), Ok(()));
    assert!(std::fs::metadata(&deep).unwrap().is_dir());
    assert_eq!(
        make_folder(&deep),
        Ok(()),
        "recursive: an existing folder is no failure"
    );
    fx.s.file("studio/workspace/songs/file", "x");
    let got = make_folder(&fx.at("file/sub"));
    let Err(EntryError::Io {
        code,
        syscall,
        path,
    }) = got
    else {
        panic!("{got:?}")
    };
    assert_eq!(
        (syscall, path.as_str()),
        ("mkdir", fx.at("file/sub").as_str())
    );
    // The shim's own mkdir is std's: below a file Windows answers EEXIST, the others ENOTDIR.
    assert_eq!(code, if cfg!(windows) { "EEXIST" } else { "ENOTDIR" });
}

#[test]
fn the_upload_s_halves_make_the_parents_then_rename_a_hidden_temp() {
    let fx = Fx::new("a-upload");
    let h = fx.s.home();
    let space = Space::open(&h, &fx.root);
    let abs = fx.at("media/take/one.bin");
    let tmp = upload_begin(&space, &abs).unwrap();
    assert!(
        std::fs::metadata(fx.at("media/take")).unwrap().is_dir(),
        "the parents are made by the first half"
    );
    let name = tmp.rsplit('/').next().unwrap();
    assert!(skip(name), "{name} is hidden by every listing");
    assert_eq!(tmp.rsplit_once('/').unwrap().0, fx.at("media/take"));
    std::fs::write(&tmp, "bytes").unwrap();
    assert_eq!(upload_end(&space, &abs, &tmp), Ok(()));
    assert_eq!(std::fs::read_to_string(&abs).unwrap(), "bytes");
    assert!(std::fs::metadata(&tmp).is_err(), "renamed away");
    // Another upload of the name landed meanwhile: refused, and the temp is the caller's to remove.
    let tmp2 = upload_begin(&space, &abs).unwrap();
    std::fs::write(&tmp2, "late").unwrap();
    assert_eq!(
        upload_end(&space, &abs, &tmp2),
        Err(EntryError::Exists {
            rel: "media/take/one.bin".into()
        })
    );
    assert_eq!(std::fs::read_to_string(&abs).unwrap(), "bytes");
    assert_ne!(tmp, tmp2, "two halves never share a temp");
    // A temp that is not beside its target is no upload of this workspace's.
    let stray = fx.at("stray.tmp");
    std::fs::write(&stray, "").unwrap();
    assert!(matches!(
        upload_end(&space, &fx.at("media/other"), &stray),
        Err(EntryError::Refused(_))
    ));
    // …nor a target outside the workspace and the libraries.
    let out = fx.s.path("elsewhere/x");
    assert!(matches!(
        upload_end(
            &space,
            &out,
            &format!("{}/.x.1.00000000.tmp", fx.s.path("elsewhere"))
        ),
        Err(EntryError::Refused(_))
    ));
}

#[test]
fn an_upload_into_a_library_is_refused() {
    let fx = Fx::new("a-upload-lib");
    let abs = fx.lib("modules/audio/verb/ir.wav");
    let user = fx.s.home();
    let u = Space::open(&user, &fx.root);
    let tmp = upload_begin(&u, &abs).unwrap();
    std::fs::write(&tmp, "w").unwrap();
    assert!(
        matches!(upload_end(&u, &abs, &tmp), Err(EntryError::Refused(_))),
        "1290: a library is not written from a workspace"
    );
}

#[test]
fn a_link_is_moved_as_itself() {
    let fx = Fx::new("a-link");
    if !can_link(&fx.s) {
        return;
    }
    fx.s.file("studio/workspace/songs/target.txt", "t");
    link_file(&fx.at("target.txt"), &fx.at("lnk")).unwrap();
    let h = fx.s.home();
    let space = Space::open(&h, &fx.root);
    let done = plan(
        &space,
        &Op::Rename {
            path: "lnk",
            name: "lnk2",
        },
        &k7_file_service::confine::Disk,
    )
    .unwrap();
    let Outcome::Rename { from, to, .. } = done else {
        panic!()
    };
    rename_once(&Disk, &from, &to).unwrap();
    assert!(
        std::fs::symlink_metadata(&to)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        std::fs::read_to_string(fx.at("target.txt")).unwrap(),
        "t",
        "its target is untouched"
    );
}

#[test]
fn a_target_made_between_the_plan_and_the_rename_is_replaced_as_the_referee_does() {
    let fx = Fx::new("a-race");
    fx.s.file("studio/workspace/songs/a.txt", "from");
    let h = fx.s.home();
    let space = Space::open(&h, &fx.root);
    let done = plan(
        &space,
        &Op::Rename {
            path: "a.txt",
            name: "b.txt",
        },
        &k7_file_service::confine::Disk,
    )
    .unwrap();
    fx.s.file("studio/workspace/songs/b.txt", "landed meanwhile");
    let Outcome::Rename { from, to, .. } = done else {
        panic!()
    };
    assert_eq!(rename_once(&Disk, &from, &to), Ok(()));
    assert_eq!(std::fs::read_to_string(fx.at("b.txt")).unwrap(), "from");
}

#[test]
fn no_answer_carries_a_verbatim_prefix() {
    let fx = Fx::new("a-prefix");
    let h = fx.s.home();
    let space = Space::open(&h, &fx.root);
    fx.s.file("studio/workspace/songs/a.txt", "x");
    for op in [
        Op::Rename {
            path: "a.txt",
            name: "b.txt",
        },
        Op::Folder {
            dir: ".",
            name: "new",
        },
        Op::Delete { path: "a.txt" },
    ] {
        let done = plan(&space, &op, &k7_file_service::confine::Disk).unwrap();
        let body = k7_file_service::entries::plan::answer_of(&space, &done).to_string();
        assert!(!body.contains(r"\\?\") && !body.contains("//?/"), "{body}");
    }
    let io = EntryError::Io {
        code: "EACCES",
        syscall: "mkdir",
        path: "/ws/x".into(),
    };
    assert_eq!(io.to_string(), "EACCES: permission denied, mkdir '/ws/x'");
}

#[test]
fn the_temp_matches_the_typescript_s_shape() {
    let ts = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../..",
        "/packages/song/src/atomicWrite.ts"
    ))
    .unwrap();
    assert!(ts.contains(r"const TEMP_NAME = /^\..+\.\d+\.[0-9a-f]{8}\.tmp$/;"));
    let t = temp_for("/ws/dir/take.bin", 4242, 0xab);
    assert_eq!(t, "/ws/dir/.take.bin.4242.000000ab.tmp");
    let name = t.rsplit('/').next().unwrap();
    let parts: Vec<&str> = name.rsplitn(4, '.').collect();
    // [tmp, <8 hex>, <pid>, ".take.bin"]
    assert_eq!(
        (parts[0], parts[1].len(), parts[2].parse::<u32>().is_ok()),
        ("tmp", 8, true)
    );
    assert!(
        parts[1]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
    assert!(name.starts_with('.') && skip(name));
}
