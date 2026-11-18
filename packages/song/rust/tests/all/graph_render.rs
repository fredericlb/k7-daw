// graph_render.rs — Part B of 2k: whether a render exists, as the Source says it (decisions 335, 444, 458).
// A reader would get wrong that a backslash is a name under the Posix rule and a separator under the Windows one: both are asked.

use crate::support;

use song::graph::render_exists;
use song::source::SourceError;
use song::{DiskSource, Entry, Kind, MemSource, SourceBundle};
use std::collections::BTreeMap;
use support::{Tmp, spell};

fn exists(source: &MemSource, render: &str) -> Result<bool, SourceError> {
    render_exists(source, "/p/ws/proj", render, song::PathRule::Posix)
}

fn exists_windows(source: &MemSource, render: &str) -> Result<bool, SourceError> {
    render_exists(source, "/p/ws/proj", render, song::PathRule::Windows)
}

// 335: a render's stat is a stat of any kind through the Source; what it refuses, cannot name or does not hold is false.
#[test]
fn a_render_exists_when_the_source_says_so_and_never_when_it_refuses() {
    let src = support::mem(&[("ws/proj/renders/a.wav", "RIFF"), ("ws/proj/renders/x\\y.wav", "RIFF")]);
    assert_eq!(exists(&src, "renders/a.wav"), Ok(true));
    assert_eq!(exists(&src, "renders"), Ok(true));
    assert_eq!(exists(&src, "/p/ws/proj/renders/a.wav"), Ok(true));
    assert_eq!(exists(&src, "../proj/renders/a.wav"), Ok(true));
    assert_eq!(exists(&src, "renders/none.wav"), Ok(false));
    assert_eq!(exists(&src, "../../../etc/passwd"), Ok(false));
    assert_eq!(exists(&src, "/etc/passwd"), Ok(false));
    assert_eq!(exists(&src, "renders/a\0.wav"), Ok(false));
    // A bundle that lists a render and holds nothing of it cannot say: that is `Missing`, never false.
    let ghost = SourceBundle {
        format: 1,
        mounts: vec!["/p".to_owned()],
        files: BTreeMap::new(),
        lists: BTreeMap::from([(
            "/p/ws/proj/renders".to_owned(),
            vec![Entry {
                name: "ghost.wav".to_owned(),
                kind: Kind::File,
            }],
        )]),
        ..Default::default()
    };
    let ghost = MemSource::from_bundle(ghost).expect("a bundle");
    assert_eq!(exists(&ghost, "renders/ghost.wav"), Err(SourceError::Missing));
    // `spell` makes a backslash a separator under the Windows rule only: there `x\y.wav` reads as the folder `x`, which holds nothing.
    assert_eq!(exists(&src, "renders/x\\y.wav"), Ok(true));
    assert_eq!(exists_windows(&src, "renders/x\\y.wav"), Ok(false));
    // ...and there `renders\a.wav` is `renders/a.wav`, a file the source holds; under the Posix rule it is one name, which it does not.
    assert_eq!(exists(&src, "renders\\a.wav"), Ok(false));
    assert_eq!(exists_windows(&src, "renders\\a.wav"), Ok(true));
}

// 444: a name of the graph is never joined to a path in Part A; the one such path is the render, and the disk Source refuses a `:` past the drive on Windows.
#[test]
fn a_colon_in_a_render_is_a_file_on_unix_and_outside_on_windows() {
    let t = Tmp::new("handed-colon");
    let root = spell(&t.0);
    if !cfg!(windows) {
        std::fs::write(t.0.join("a:b.wav"), b"RIFF").expect("write");
    }
    let disk = DiskSource::with_files(std::slice::from_ref(&root), &Vec::<String>::new()).expect("a source");
    let got = render_exists(&disk, &root, "a:b.wav", song::PathRule::HOST);
    assert_eq!(got, Ok(!cfg!(windows)));
}
