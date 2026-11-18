//! Hostile input to the entries (1225, 1226, 1231): names and paths at the limits through every op, a bin of 2,000
//! entries of one name, a folder of 20,000 moved with one rename, a folder that cannot be made, an upload that cannot
//! begin. Each answered fast, asserting the answer and the time on the best of three.

use crate::support;

use std::time::{Duration, Instant};

use k7_file_service::entries::apply::*;
use k7_file_service::entries::plan::plan;
use k7_file_service::entries::{EntryError, Op, Outcome};
use k7_file_service::space::{Declined, MAX_REL, Space};
use support::entries::Fx;

const BOUND: Duration = Duration::from_secs(if cfg!(target_env = "musl") { 30 } else { 5 });

/// The best of three runs of `f`, and its last answer.
fn best<T>(mut f: impl FnMut() -> T) -> (Duration, T) {
    let mut best = Duration::MAX;
    let mut last = None;
    for _ in 0..3 {
        let t0 = Instant::now();
        let v = f();
        best = best.min(t0.elapsed());
        last = Some(v);
    }
    (best, last.unwrap())
}

fn every_op<'a>(a: &'a str, b: &'a str) -> [Op<'a>; 5] {
    [
        Op::Folder { dir: a, name: b },
        Op::Rename { path: a, name: b },
        Op::Move { path: a, to_dir: b },
        Op::Delete { path: a },
        Op::Upload { dir: a, name: b },
    ]
}

#[test]
fn the_gate_s_hostile_paths_are_answered_fast_through_every_op() {
    let fx = Fx::new("h-gate");
    let h = fx.s.home(false);
    let space = Space::open(&h, &fx.root);
    let long = "a".repeat(MAX_REL + 1);
    let slashes = "/".repeat(8000);
    let deep = "d/".repeat(300);
    let blanks = ["\u{feff}", "\u{3000}", "  ..  ", "..\u{3000}"];
    let mut inputs: Vec<String> = [
        "",
        ".",
        "..",
        "../x",
        "a/../../x",
        "/etc/passwd",
        "C:/x",
        "\\\\server\\x",
        "a/b\0",
        "_libraries",
        "_libraries/Bad/x",
        "_libraries/k7-core/../../x",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    inputs.extend([long.clone(), slashes.clone(), deep.clone()]);
    inputs.extend(blanks.iter().map(|s| s.to_string()));
    for a in &inputs {
        for b in &inputs {
            for op in every_op(a, b) {
                let (t, _) = best(|| plan(&space, &op, &k7_file_service::confine::Disk));
                assert!(t < BOUND, "{t:?} for {:?}", &a[..a.len().min(20)]);
            }
        }
    }
    // The sentences that matter: a name past 16 KiB is declined, a name of slashes is no name.
    let op = Op::Folder {
        dir: ".",
        name: &long,
    };
    assert_eq!(
        plan(&space, &op, &k7_file_service::confine::Disk),
        Err(EntryError::Declined(Declined::LongPath))
    );
    let op = Op::Folder {
        dir: ".",
        name: &slashes,
    };
    assert_eq!(
        plan(&space, &op, &k7_file_service::confine::Disk),
        Err(EntryError::Refused(format!(
            "invalid folder name: {slashes}"
        )))
    );
    for n in blanks {
        let r = plan(
            &space,
            &Op::Rename {
                path: "song.k7/clips",
                name: n,
            },
            &k7_file_service::confine::Disk,
        );
        assert_eq!(
            r,
            Err(EntryError::Refused(format!("invalid name: {n}"))),
            "{n:?}"
        );
    }
}

#[test]
fn a_move_onto_itself_or_a_child_three_hundred_deep_is_refused() {
    let fx = Fx::new("h-self");
    let mut chain = String::from("deep");
    fx.s.dir("studio/workspace/songs/deep");
    for _ in 0..300 {
        chain.push_str("/d");
    }
    fx.s.dir(&format!("studio/workspace/songs/{chain}"));
    let (t, r) = best(|| {
        fx.user(Op::Move {
            path: "deep",
            to_dir: &chain,
        })
    });
    assert_eq!(
        r,
        Err(EntryError::Refused("can't move 'deep' into itself".into()))
    );
    assert!(t < BOUND, "{t:?}");
    assert_eq!(
        fx.user(Op::Move {
            path: "deep",
            to_dir: "deep"
        }),
        Err(EntryError::Refused("can't move 'deep' into itself".into()))
    );
}

#[test]
fn two_thousand_deletes_of_one_name_find_the_next_free_one_fast() {
    let fx = Fx::new("h-bin");
    let bin = fx.dir("studio/workspace/songs/_recycle_bin");
    std::fs::File::create(format!("{bin}/x")).unwrap();
    for n in 2..=2_000 {
        std::fs::File::create(format!("{bin}/x-{n}")).unwrap();
    }
    fx.s.file("studio/workspace/songs/x", "x");
    let (t, r) = best(|| fx.user(Op::Delete { path: "x" }));
    assert_eq!(
        r,
        Ok(Outcome::Recycle {
            from: fx.at("x"),
            to: format!("{bin}/x-2001")
        })
    );
    assert!(t < BOUND, "{t:?}");
    let (t, got) = best(|| recycle_target(&bin, "x"));
    assert_eq!(got, format!("{bin}/x-2001"));
    assert!(t < BOUND, "{t:?}");
}

trait Dir {
    fn dir(&self, rel: &str) -> String;
}
impl Dir for Fx {
    fn dir(&self, rel: &str) -> String {
        self.s.dir(rel)
    }
}

#[test]
fn a_folder_of_twenty_thousand_entries_moves_with_one_rename() {
    wide_move(20_000);
}

#[test]
#[ignore = "scale: a move that copied would show at 200,000 entries, not at 20,000"]
fn a_folder_of_two_hundred_thousand_entries_moves_with_one_rename_at_scale() {
    wide_move(200_000);
}

fn wide_move(n: u32) {
    let fx = Fx::new("h-wide");
    let wide = fx.s.dir("studio/workspace/songs/wide");
    std::thread::scope(|scope| {
        for part in 0..8u32 {
            let wide = &wide;
            scope.spawn(move || {
                for i in (part * n / 8)..((part + 1) * n / 8) {
                    std::fs::File::create(format!("{wide}/f{i:06}.txt")).unwrap();
                }
            });
        }
    });
    fx.s.dir("studio/workspace/songs/elsewhere");
    let (t, r) = best(|| {
        fx.user(Op::Move {
            path: "wide",
            to_dir: "elsewhere",
        })
    });
    assert_eq!(
        r,
        Ok(Outcome::Move {
            from: fx.at("wide"),
            to: fx.at("elsewhere/wide")
        })
    );
    assert!(t < BOUND, "{t:?}");
    let t0 = Instant::now();
    rename_once(&Disk, &fx.at("wide"), &fx.at("elsewhere/wide")).unwrap();
    assert!(t0.elapsed() < BOUND, "{:?}", t0.elapsed());
    assert!(std::fs::metadata(fx.at(&format!("elsewhere/wide/f{:06}.txt", n - 1))).is_ok());
}

#[cfg(unix)]
#[test]
fn a_folder_that_cannot_be_entered_is_an_io_failure_not_a_hang() {
    use std::os::unix::fs::PermissionsExt;
    let fx = Fx::new("h-mode");
    if support::is_root() {
        return;
    }
    let shut = fx.s.dir("studio/workspace/songs/shut");
    std::fs::set_permissions(&shut, std::fs::Permissions::from_mode(0o000)).unwrap();
    let (t, got) = best(|| make_folder(&format!("{shut}/x")));
    std::fs::set_permissions(&shut, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        got,
        Err(EntryError::Io {
            code: "EACCES",
            syscall: "mkdir",
            path: format!("{shut}/x")
        })
    );
    assert!(t < BOUND, "{t:?}");
}

#[test]
fn an_upload_of_a_thousand_segments_or_into_a_file_is_answered() {
    let fx = Fx::new("h-upload");
    let name = (0..1000)
        .map(|i| format!("s{i}"))
        .collect::<Vec<_>>()
        .join("/");
    let (t, r) = best(|| {
        fx.user(Op::Upload {
            dir: ".",
            name: &name,
        })
    });
    assert!(matches!(r, Ok(Outcome::Upload { .. })), "{r:?}");
    assert!(t < BOUND, "{t:?}");
    // A `dir` that is a file: the plan sees nothing there yet, and the first half fails to make its parents.
    fx.s.file("studio/workspace/songs/afile", "x");
    let h = fx.s.home(false);
    let space = Space::open(&h, &fx.root);
    let Ok(Outcome::Upload { abs }) = fx.user(Op::Upload {
        dir: "afile",
        name: "x.bin",
    }) else {
        panic!()
    };
    let got = upload_begin(&space, &abs);
    assert!(
        matches!(
            got,
            Err(EntryError::Io {
                syscall: "mkdir",
                ..
            })
        ),
        "{got:?}"
    );
}
