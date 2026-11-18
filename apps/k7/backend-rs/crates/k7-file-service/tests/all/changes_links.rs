//! A change set against links (F5, F6) and the check-only `read` op: a link in a written path, a copy source or a job's folder
//! is refused; a read writes nothing and is stale when the file moved.

use crate::support;

use k7_file_service::changes::Refusal;
use k7_file_service::changes::fresh::fresh;
use k7_file_service::text_hash::text_hash;
use serde_json::json;
use support::changes::{land, planned, refused, seed, text};
use support::entries::Fx;

// 3-B2 for 3c: a `read` op judges what a verb only read; it writes nothing, and a read of a binary or an absent file is judged too.
#[test]
fn a_read_op_is_stale_when_the_file_moved_and_writes_nothing() {
    let fx = Fx::new("c-read");
    seed(&fx);
    let set = |was: serde_json::Value| {
        planned(
            &fx,
            true,
            json!([{ "kind": "read", "file": "song.k7/a.txt", "was": was }]),
        )
        .unwrap()
    };
    assert_eq!(fresh(&set(json!(text_hash("old a"))), "t1", "t1"), Ok(()));
    assert_eq!(
        fresh(&set(json!(text_hash("older"))), "t1", "t1"),
        Err(Refusal::Stale {
            files: vec!["song.k7/a.txt".into()],
            project: false
        })
    );
    assert!(
        fresh(&set(json!(null)), "t1", "t1").is_err(),
        "it exists now"
    );
    let absent = planned(
        &fx,
        true,
        json!([{ "kind": "read", "file": "song.k7/none.txt", "was": null }]),
    )
    .unwrap();
    assert_eq!(fresh(&absent, "t1", "t1"), Ok(()));
    // A read and a write of one file are no "two ops", and a set of reads lands nothing.
    assert!(
        planned(
            &fx,
            true,
            json!([
                { "kind": "read", "file": "song.k7/a.txt", "was": text_hash("old a") },
                text("song.k7/a.txt", "new", json!(text_hash("old a"))),
            ])
        )
        .is_ok()
    );
    let before = support::changes::tree(&fx.root);
    let landed = land(
        &fx,
        json!([{ "kind": "read", "file": "song.k7/a.txt", "was": text_hash("old a") }]),
    )
    .unwrap();
    assert!(landed.touched.is_empty() && !landed.structural);
    assert_eq!(support::changes::tree(&fx.root), before);
    // The gates cover a read: a path out of the workspace is forbidden, not stale.
    let r = refused(
        &fx,
        true,
        json!([{ "kind": "read", "file": "../x", "was": null }]),
    );
    assert_eq!(r.status(), 403, "{r}");
}

#[cfg(unix)]
mod unix {
    use super::*;
    use std::os::unix::fs::symlink;

    // F6: a written path below a linked folder is refused, whatever the kind; nothing lands outside the workspace.
    #[test]
    fn a_link_in_a_written_path_is_refused_for_every_kind() {
        let fx = Fx::new("c-link-write");
        seed(&fx);
        let outside = fx.s.path("outside");
        std::fs::create_dir_all(&outside).unwrap();
        symlink(&outside, fx.at("song.k7/linked")).unwrap();
        fx.s.file("studio/.job-out/j1/a.wav", "RIFF");
        for o in [
            text("song.k7/linked/new.txt", "x", json!(null)),
            json!({ "kind": "remove", "file": "song.k7/linked/x.txt", "was": null }),
            json!({ "kind": "copy", "from": "song.k7/a.txt", "file": "song.k7/linked/c.txt" }),
            json!({ "kind": "move", "from": "song.k7/m.txt", "file": "song.k7/linked/m.txt" }),
            json!({ "kind": "adopt", "job": "j1", "out": "a.wav", "file": "song.k7/linked/a.wav" }),
        ] {
            let r = refused(&fx, true, json!([o.clone()]));
            assert_eq!(r.status(), 403, "{o}: {r}");
        }
        assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 0);
    }

    // F6: a copy source that is a link, or below one, is no source: the link's target is read by nobody.
    #[test]
    fn a_link_as_a_copy_source_is_refused() {
        let fx = Fx::new("c-link-source");
        seed(&fx);
        let outside = fx.s.file("outside/secret.txt", "secret");
        symlink(&outside, fx.at("song.k7/ln.txt")).unwrap();
        symlink(fx.s.path("outside"), fx.at("song.k7/dir")).unwrap();
        for from in ["song.k7/ln.txt", "song.k7/dir/secret.txt"] {
            let r = refused(
                &fx,
                true,
                json!([{ "kind": "copy", "from": from, "file": "song.k7/c9.txt" }]),
            );
            assert!(matches!(r.status(), 400 | 403), "{from}: {r}");
        }
        let r = refused(
            &fx,
            true,
            json!([{ "kind": "move", "from": "song.k7/ln.txt", "file": "song.k7/m9.txt" }]),
        );
        assert!(matches!(r.status(), 400 | 403), "{r}");
    }

    // F5: a job's folder or the staged-output folder that is a link is no staged output, so a job cannot name a file elsewhere.
    #[test]
    fn a_linked_job_folder_or_job_out_is_refused() {
        let fx = Fx::new("c-link-job");
        seed(&fx);
        fx.s.file("outside/a.wav", "RIFF");
        std::fs::create_dir_all(fx.s.path("studio/.job-out")).unwrap();
        symlink(fx.s.path("outside"), fx.s.path("studio/.job-out/j1")).unwrap();
        let adopt =
            json!([{ "kind": "adopt", "job": "j1", "out": "a.wav", "file": "song.k7/a.wav" }]);
        let r = refused(&fx, true, adopt.clone());
        assert_eq!(r.status(), 400, "{r}");
        std::fs::remove_file(fx.s.path("studio/.job-out/j1")).unwrap();
        std::fs::remove_dir(fx.s.path("studio/.job-out")).unwrap();
        symlink(fx.s.path("outside"), fx.s.path("studio/.job-out")).unwrap();
        fx.s.file("outside/j1/a.wav", "RIFF");
        let r = refused(&fx, true, adopt);
        assert_eq!(r.status(), 400, "{r}");
    }
}
