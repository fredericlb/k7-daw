//! A library module moved or renamed, replayed from the TypeScript's goldens (`workspaceEntries.ts` through a real Workspace):
//! the plan, the folder's rename, the repoint, then the whole tree of files compared with the one the TypeScript left.

// Only the link test below uses them, and it is unix-only.
#[cfg(unix)]
use k7_file_service::edits::module_make::clone_module;
#[cfg(unix)]
use k7_file_service::libraries::Home;
#[cfg(unix)]
use k7_file_service::space::Space;

#[cfg(unix)]
use crate::support::Scratch;

// A clone reads the workspace alone: a link to another library's file, or to `..`, is refused and nothing lands.
#[cfg(unix)]
#[test]
fn a_clone_refuses_links() {
    let code = format!("{}/../../../..", env!("CARGO_MANIFEST_DIR"));
    for (label, target) in [("file", "libraries/private/x.wav"), ("up", "..")] {
        let s = Scratch::new(&format!("em-clone-link-{label}"));
        s.file("libraries/private/x.wav", "secret");
        s.file(
            "home/workspace/songs/_modules/audio/verb/module.k7x",
            "extends(\"@engine/reverb\")\n",
        );
        s.file(
            "home/workspace/songs/p.k7/project.k7x",
            "project({ name: \"p\" })\n",
        );
        s.file("home/workspace/.active-projects/songs", "p.k7");
        let module = s.path("home/workspace/songs/_modules/audio/verb");
        std::os::unix::fs::symlink(s.path(target), format!("{module}/ir")).unwrap();
        let home = Home {
            studio: s.path("home"),
            libraries: s.path("libraries"),
        };
        let space = Space::open(&home, &s.path("home/workspace/songs"));
        let before = std::fs::read_dir(s.path("home/workspace/songs/_modules/audio"))
            .unwrap()
            .count();
        let made = clone_module(&home, &code, &space, "_modules/audio/verb", "copy");
        let err = made.map(|m| m.answer).expect_err(label).to_string();
        assert!(err.contains("a link"), "{label}: {err}");
        let after = std::fs::read_dir(s.path("home/workspace/songs/_modules/audio"))
            .unwrap()
            .count();
        assert_eq!(before, after, "{label}: a clone landed");
    }
}
