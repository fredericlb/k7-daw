//! Confinement, as `hubConfinement.test.ts` and `@studio/web-protocol/paths` hold it.

use super::*;

/// A directory link anyone may make: a junction on Windows, a symlink elsewhere.
fn link_dir(target: &Path, at: &Path) {
    #[cfg(windows)]
    {
        let ok = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(at)
            .arg(target)
            .output()
            .expect("cmd")
            .status
            .success();
        assert!(ok, "mklink /J");
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, at).unwrap();
}

fn base(name: &str) -> PathBuf {
    let b = std::env::temp_dir().join(format!("k7-hubfiles-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&b);
    std::fs::create_dir_all(b.join("checkout").join("wavetables")).unwrap();
    std::fs::write(
        b.join("checkout").join("wavetables").join("basic.wav"),
        "RIFF",
    )
    .unwrap();
    std::fs::create_dir_all(b.join("secret")).unwrap();
    std::fs::write(b.join("secret").join("key"), "x").unwrap();
    link_dir(&b.join("secret"), &b.join("checkout").join("escape"));
    std::fs::create_dir_all(b.join("edits")).unwrap();
    link_dir(&b.join("checkout"), &b.join("edits").join("k7"));
    b
}

fn root(b: &Path, per_child: bool) -> Root {
    Root {
        remote: b.join("edits").to_string_lossy().into_owned(),
        name: "edits-x".into(),
        writable: false,
        per_child: per_child.then_some(true),
    }
}

fn s(p: PathBuf) -> String {
    p.to_string_lossy().into_owned()
}

#[tokio::test]
async fn a_per_child_root_serves_a_linked_clone_but_not_a_link_out_of_it() {
    let b = base("perchild");
    let file = s(b
        .join("edits")
        .join("k7")
        .join("wavetables")
        .join("basic.wav"));
    let escape = s(b.join("edits").join("k7").join("escape").join("key"));
    assert!(matches!(
        stat_paths(&[root(&b, true)], std::slice::from_ref(&file)).await[0],
        RemoteStat::File { .. }
    ));
    assert!(matches!(
        stat_paths(&[root(&b, true)], &[escape]).await[0],
        RemoteStat::Missing { .. }
    ));
    assert!(matches!(
        stat_paths(&[root(&b, false)], &[file]).await[0],
        RemoteStat::Missing { .. }
    ));
    let _ = std::fs::remove_dir_all(&b);
}

#[test]
fn lexical_checks_refuse_what_could_climb() {
    if cfg!(windows) {
        assert!(is_plain(r"C:\srv\ws\a.wav") && is_plain("C:/srv/ws/a.wav"));
        for bad in [
            r"C:\srv\ws\..\x",
            r"C:\srv\ws\.\x",
            "srv/ws",
            r"C:srv\x",
            "C:/srv/ws/../x",
        ] {
            assert!(!is_plain(bad), "{bad}");
        }
        assert!(is_within(r"C:\Srv\WS", r"c:\srv\ws\a") && is_within(r"C:\srv\ws", r"C:\srv\ws/a"));
    } else {
        assert!(is_plain("/srv/ws/a.wav") && is_plain("/srv/ws/") && is_plain("/"));
        for bad in [
            "/srv/ws/../x",
            "/srv/./ws",
            "/srv//ws",
            "srv/ws",
            "",
            "//srv",
        ] {
            assert!(!is_plain(bad), "{bad}");
        }
        assert!(is_plain("/srv/ws/..\\..\\x"));
    }
    assert!(!is_within("/srv/ws", "/srv/wsx/a"));
    assert!(is_within("/srv/ws", "/srv/ws") && is_within("/srv/ws", "/srv/ws/a"));
    assert!(!is_within("/srv/ws", "/SRV/ws/a"));
}
