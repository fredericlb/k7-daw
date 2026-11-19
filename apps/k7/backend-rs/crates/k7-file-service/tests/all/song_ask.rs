//! `song_roots` (1117, 1513, 1514): the fields on a plain home, the libraries' order and flags, links, Windows spellings, hostile inputs.
//! The tables hold the cases no disk here can show: platform spellings and hostile inputs.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use crate::support::Scratch;
use k7_file_service::confine::Disk;
use k7_file_service::libraries::Home;
use k7_file_service::song_ask::{Input, song_roots};
use k7_file_service::space::Declined;
use song::{PathRule, Roots};

const BOUND: Duration = Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 30 });

fn roots(
    home: &Home,
    project: &str,
    code: &str,
    env: &BTreeMap<String, String>,
) -> Result<Roots, Declined> {
    song_roots(
        &Input {
            home,
            project,
            code_root: code,
            env,
        },
        &Disk,
    )
}

fn plain(s: &Scratch) -> (Home, String) {
    let project = s.dir("studio/workspace/ws/p.k7");
    (s.home(), project)
}

#[test]
fn a_plain_home_gives_every_field() {
    let s = Scratch::new("ask-plain");
    let (home, project) = plain(&s);
    let got = roots(&home, &project, "/code", &BTreeMap::new()).unwrap();
    let studio = s.path("studio");
    assert_eq!(got.project, project);
    assert_eq!(got.workspace, Some(s.path("studio/workspace/ws")));
    assert_eq!(got.workspace_namespace, None);
    assert_eq!(got.confine, Some(s.path("studio/workspace/ws")));
    assert_eq!(got.libraries.len(), 1);
    assert_eq!(
        (
            got.libraries[0].name.as_str(),
            got.libraries[0].root.as_str(),
            got.libraries[0].editable
        ),
        ("account", format!("{studio}/library").as_str(), true)
    );
    assert_eq!(got.meta.as_deref(), Some("/code/packages/song/meta"));
    assert_eq!(got.scan, Some(format!("{studio}/exts.json")));
    assert_eq!(got.renders, Some(format!("{studio}/.renders")));
    assert_eq!(got.models, Some(format!("{studio}/models")));
    assert!(got.sample_banks.is_empty());
    assert_eq!(got.also_readable, [format!("{studio}/library")]);
    assert_eq!(got.paths, PathRule::HOST);
}

#[test]
fn libraries_are_listed_by_bytes_kept_by_their_name_and_flagged_by_their_root() {
    let s = Scratch::new("ask-libs");
    let (home, project) = plain(&s);
    for n in ["b", "a-1", "a", "9z"] {
        s.dir(&format!("libraries/{n}"));
    }
    for n in ["Bad", "_x", "engine", "a b"] {
        s.dir(&format!("libraries/{n}"));
    }
    s.file("libraries/file", "not a library");
    s.dir("libraries/a/1");
    s.dir("libraries/a/draft");
    s.file(
        "studio/workspace/ws/_library-pins.k7x",
        "a(1)\nb(\"draft\")\n",
    );
    let names = |r: &Roots| {
        r.libraries
            .iter()
            .map(|l| l.name.clone())
            .collect::<Vec<_>>()
    };
    let user = roots(&home, &project, "/code", &BTreeMap::new()).unwrap();
    assert_eq!(names(&user), ["account", "9z", "a", "a-1", "b"]);
    assert_eq!(user.libraries[2].root, s.path("libraries/a/1"));
    assert!(
        user.libraries
            .iter()
            .filter(|l| l.name != "account")
            .all(|l| !l.editable)
    );
}

#[test]
fn a_family_folder_that_is_a_link_adds_its_real_path_and_a_broken_one_adds_nothing() {
    let s = Scratch::new("ask-links");
    let (home, project) = plain(&s);
    let real = s.dir("elsewhere/real");
    let file = s.file("elsewhere/file", "x");
    s.dir("libraries/a");
    let at = |f: &str| s.path(&format!("libraries/a/{f}"));
    #[cfg(unix)]
    let link = |to: &str, f: &str| std::os::unix::fs::symlink(to, at(f)).is_ok();
    #[cfg(windows)]
    let link = |to: &str, f: &str| std::os::windows::fs::symlink_dir(to, at(f)).is_ok();
    if !(link(&real, "modules")
        && link(&s.path("nowhere"), "chains")
        && link(&at("samples"), "samples")
        && link(&file, "irs"))
    {
        eprintln!("links cannot be made here: skipped");
        return;
    }
    let got = roots(&home, &project, "/code", &BTreeMap::new()).unwrap();
    assert!(got.also_readable.contains(&real), "{:?}", got.also_readable);
    assert!(
        !got.also_readable
            .iter()
            .any(|p| p.ends_with("nowhere") || p.ends_with("/samples")),
        "{:?}",
        got.also_readable
    );
}

#[test]
fn the_environment_names_the_models_and_the_banks_as_javascript_reads_it() {
    let s = Scratch::new("ask-env");
    let (home, project) = plain(&s);
    let env = |pairs: &[(&str, &str)]| {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect::<BTreeMap<_, _>>()
    };
    let got = roots(
        &home,
        &project,
        "/code",
        &env(&[
            ("STUDIO_MODELS", "rel/m"),
            ("DIRT_SAMPLES", "rel/d"),
            ("STRUDEL_SAMPLES", "/abs/s"),
        ]),
    )
    .unwrap();
    assert_eq!(got.models.as_deref(), Some("/code/rel/m"));
    assert_eq!(got.sample_banks, ["rel/d", "/abs/s"]);
    let none = roots(
        &home,
        &project,
        "/code",
        &env(&[
            ("STUDIO_MODELS", ""),
            ("DIRT_SAMPLES", ""),
            ("STRUDEL_SAMPLES", ""),
        ]),
    )
    .unwrap();
    assert_eq!(none.models, Some(s.path("studio/models")));
    assert!(none.sample_banks.is_empty());
}

#[test]
fn a_project_inside_a_library_is_at_that_librarys_level() {
    let s = Scratch::new("ask-inlib");
    let project = s.dir("libraries/lib/p.k7");
    let got = roots(&s.home(), &project, "/code", &BTreeMap::new()).unwrap();
    assert_eq!(got.workspace_namespace.as_deref(), Some("lib"));
    assert_eq!(got.confine, Some(s.path("libraries/lib")));
}

// 1514: a home spelled with a drive keeps its case, every path is `/`-separated, and the mounts hold under the Windows rule.
#[test]
fn windows_spelled_homes_and_code_roots_keep_their_drive_and_slashes() {
    let env: BTreeMap<String, String> = [("HOME", "C:\\Users\\a"), ("STUDIO_MODELS", "D:\\models")]
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    let home = Home::of(&env);
    let project = "C:/Users/a/.k7/workspace/ws/p.k7";
    let got = roots(&home, project, r"C:\k7", &env).unwrap();
    assert_eq!(got.project, project);
    assert_eq!(
        got.workspace.as_deref(),
        Some("C:/Users/a/.k7/workspace/ws")
    );
    assert_eq!(got.confine.as_deref(), Some("C:/Users/a/.k7/workspace/ws"));
    assert_eq!(got.meta.as_deref(), Some("C:/k7/packages/song/meta"));
    assert_eq!(got.scan.as_deref(), Some("C:/Users/a/.k7/exts.json"));
    assert_eq!(got.models.as_deref(), Some("D:/models"));
    assert_eq!(got.libraries[0].root, "C:/Users/a/.k7/library");
    let lowered = |r: &Roots| {
        r.libraries
            .iter()
            .map(|l| l.root.clone())
            .chain(r.meta.clone())
            .chain(r.scan.clone())
            .any(|p| p.starts_with("c:"))
    };
    assert!(!lowered(&got));
    let mut win = got.clone();
    win.paths = PathRule::Windows;
    let (dirs, files) = win.mounts().unwrap();
    assert_eq!(dirs[0], project);
    assert_eq!(files, ["C:/Users/a/.k7/exts.json"]);
    assert!(
        dirs.iter()
            .all(|d| d.starts_with("C:/") || d.starts_with("D:/")),
        "{dirs:?}"
    );
}

// 1514: the mounts of a POSIX home hold under the host's rule forced, each inside the roots that name it.
#[test]
fn the_mounts_hold_under_the_host_rule() {
    let s = Scratch::new("ask-mounts");
    let (home, project) = plain(&s);
    s.dir("libraries/a");
    let mut got = roots(&home, &project, "/code", &BTreeMap::new()).unwrap();
    got.paths = PathRule::HOST;
    let (dirs, files) = got.mounts().unwrap();
    assert!(dirs.contains(&project) && files.len() == 1, "{dirs:?}");
}

#[test]
fn an_unreadable_pins_file_is_no_pins() {
    let s = Scratch::new("ask-pins");
    let (home, project) = plain(&s);
    s.dir("libraries/a");
    for text in ["this does not parse (".to_owned(), "x".repeat(100_000)] {
        s.file("studio/workspace/ws/_library-pins.k7x", &text);
        assert!(roots(&home, &project, "/code", &BTreeMap::new()).is_ok());
    }
}

#[test]
fn two_hundred_libraries_and_a_libraries_home_that_is_a_file_are_answered_in_time() {
    let s = Scratch::new("ask-hostile");
    let (home, project) = plain(&s);
    for i in 0..200 {
        s.dir(&format!("libraries/lib-{i:03}"));
    }
    let t = Instant::now();
    let got = roots(&home, &project, "/code", &BTreeMap::new()).unwrap();
    assert_eq!(got.libraries.len(), 201);
    assert_eq!(got.libraries[1].name, "lib-000");
    assert!(t.elapsed() < BOUND, "{:?}", t.elapsed());
    std::fs::remove_dir_all(s.path("libraries")).unwrap();
    s.file("libraries", "a file");
    assert_eq!(
        roots(&home, &project, "/code", &BTreeMap::new())
            .unwrap()
            .libraries
            .len(),
        1
    );
}
