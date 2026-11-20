//! What a host route may touch (3f): reads under the roots only, writes through `.job-out` only, and no refusal names a real folder.

use k7_file_service::verb_host::gate::{out_dir, out_file, read_path};
use k7_file_service::verb_paths::Virtual;
use song::roots::Roots;

use crate::support::jobs::Tmp;

fn world(t: &Tmp) -> Virtual {
    t.write("ws/songs/a.k7/x.txt", "x");
    t.write("code/k.txt", "k");
    t.write("studio/s.txt", "s");
    t.write("outside/secret.txt", "no");
    Virtual::new(
        t.at("ws/songs").trim_end_matches('/'),
        &[],
        &t.at("code"),
        &t.at("studio"),
        &Roots::default(),
    )
}

#[test]
fn a_virtual_path_under_a_root_is_read_and_anything_else_is_a_403() {
    let t = Tmp::new("vh-read");
    let v = world(&t);
    assert_eq!(
        read_path(&v, "/w/a.k7/x.txt").unwrap(),
        t.at("ws/songs/a.k7/x.txt")
    );
    assert_eq!(read_path(&v, "/s/s.txt").unwrap(), t.at("studio/s.txt"));
    for bad in [
        "/w/a.k7/COM1.wav",
        "/w/a.k7/nul",
        "/w/a.k7/Aux.tar.gz",
        "/w/a.k7/lpt9.txt",
        "/w/a.k7/x.txt:stream",
        "/w/a.k7/x.txt.",
        "/w/a.k7/x.txt ",
    ] {
        assert_eq!(read_path(&v, bad).unwrap_err().status, 403, "{bad}");
    }
    // A name that only starts like a device is a file.
    assert!(read_path(&v, "/w/a.k7/com10.wav").is_ok());
    for bad in [
        "/etc/passwd",
        &t.at("outside/secret.txt"),
        "/w/../../outside/secret.txt",
        "/w/a.k7/../../../outside/secret.txt",
        "w/a.k7/x.txt",
        "",
        "/w/a\0b",
        "/x/deadbeef",
    ] {
        let e = read_path(&v, bad).unwrap_err();
        assert_eq!(e.status, 403, "{bad}");
        // The sentence repeats what was sent and nothing the server added.
        assert_eq!(
            e.error,
            format!(
                "'{}' is outside the folders you read",
                bad.chars().take(80).collect::<String>()
            )
        );
    }
}

#[cfg(unix)]
#[test]
fn a_link_out_of_a_root_is_not_under_it() {
    let t = Tmp::new("vh-link");
    let v = world(&t);
    std::os::unix::fs::symlink(t.at("outside"), t.at("ws/songs/a.k7/out")).unwrap();
    assert_eq!(
        read_path(&v, "/w/a.k7/out/secret.txt").unwrap_err().status,
        403
    );
    assert!(read_path(&v, "/w/a.k7/x.txt").is_ok());
}

#[test]
fn a_job_folder_is_made_and_an_output_is_one_plain_name() {
    let t = Tmp::new("vh-out");
    t.mkdir("studio");
    let id = "0123456789abcdef0123456789abcdef";
    let dir = out_dir(&t.at("studio"), id).unwrap();
    assert!(std::path::Path::new(&dir).is_dir());
    assert!(dir.ends_with(&format!(".job-out/{id}")));
    assert_eq!(out_file(&dir, "mix.wav").unwrap(), format!("{dir}/mix.wav"));
    for bad in ["", "../x", "a/b", ".hidden", "a\\b", "a\0"] {
        assert_eq!(out_file(&dir, bad).unwrap_err().status, 400, "{bad:?}");
    }
    for bad in ["", "../x", "a/b", "a b"] {
        assert_eq!(
            out_dir(&t.at("studio"), bad).unwrap_err().status,
            400,
            "{bad:?}"
        );
    }
}

// 3d's rule: a `.job-out` or a job folder that is a link is not the job's.
#[cfg(unix)]
#[test]
fn a_job_folder_that_is_a_link_is_refused() {
    let t = Tmp::new("vh-outlink");
    t.mkdir("studio");
    t.mkdir("elsewhere");
    std::os::unix::fs::symlink(t.at("elsewhere"), t.at("studio/.job-out")).unwrap();
    assert_eq!(out_dir(&t.at("studio"), "j1").unwrap_err().status, 403);
    std::fs::remove_file(t.at("studio/.job-out")).unwrap();
    t.mkdir("studio/.job-out");
    std::os::unix::fs::symlink(t.at("elsewhere"), t.at("studio/.job-out/j2")).unwrap();
    assert_eq!(out_dir(&t.at("studio"), "j2").unwrap_err().status, 403);
    assert!(out_dir(&t.at("studio"), "j3").is_ok());
}

#[test]
fn a_backslash_never_makes_a_segment_and_a_path_not_made_yet_is_judged_by_its_folder() {
    let t = Token::new();
    let v = world(&t.0);
    for bad in ["/w/a.k7\\..\\..\\x", "/w/a.k7/a\\b"] {
        assert_eq!(read_path(&v, bad).unwrap_err().status, 403, "{bad}");
    }
    // A folder a render will make is no refusal; it is read as the folder that exists.
    assert_eq!(
        read_path(&v, "/w/a.k7/new/x.wav").unwrap(),
        t.0.at("ws/songs/a.k7/new/x.wav")
    );
}

#[cfg(unix)]
#[test]
fn a_dangling_link_is_refused_and_the_path_used_is_the_one_resolved() {
    let t = Token::new();
    let v = world(&t.0);
    std::os::unix::fs::symlink("/etc/absent-for-k7", t.0.at("ws/songs/a.k7/dangling")).unwrap();
    assert_eq!(read_path(&v, "/w/a.k7/dangling").unwrap_err().status, 403);
    std::os::unix::fs::symlink(t.0.at("ws/songs/a.k7/x.txt"), t.0.at("ws/songs/a.k7/alias"))
        .unwrap();
    // The check and the open are of one path: the link's target, not the link a swap could redirect.
    assert_eq!(
        read_path(&v, "/w/a.k7/alias").unwrap(),
        t.0.at("ws/songs/a.k7/x.txt")
    );
    std::os::unix::fs::symlink(t.0.at("outside"), t.0.at("ws/songs/a.k7/gone")).unwrap();
    assert_eq!(
        read_path(&v, "/w/a.k7/gone/new/x.wav").unwrap_err().status,
        403
    );
}

struct Token(Tmp);

impl Token {
    fn new() -> Token {
        Token(Tmp::new(&format!("vh-gate-{}", std::process::id())))
    }
}

#[cfg(unix)]
#[test]
fn a_path_the_disk_cannot_resolve_is_refused_not_let_through() {
    use std::os::unix::fs::PermissionsExt;
    let t = Token::new();
    let v = world(&t.0);
    let locked = t.0.at("ws/songs/a.k7/locked");
    std::fs::create_dir_all(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    let asked = read_path(&v, "/w/a.k7/locked/x.wav");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    // A process that may enter anyway (root) resolves it, and the folder is then a plain path not made yet.
    if std::fs::canonicalize(format!("{locked}/x.wav")).is_err() {
        assert_eq!(asked.unwrap_err().status, 403);
    }
}
