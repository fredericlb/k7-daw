//! `project_roots` of confine.rs against `projectRoots` of song's `confine.ts`: the list and its order.
//! What a reader gets wrong: the libraries come before every overlay, not each beside its own.

use crate::support::Scratch;
use k7_file_service::confine::project_roots;

#[test]
fn a_home_with_no_library_gives_the_project_the_workspace_and_the_fixed_roots() {
    let s = Scratch::new("roots-plain");
    let project = s.dir("studio/workspace/ws/p.k7");
    let got = project_roots(&s.home(false), &project, "/code/meta", "/models");
    let studio = s.path("studio");
    assert_eq!(
        got,
        [
            project,
            s.path("studio/workspace/ws"),
            format!("{studio}/library"),
            "/code/meta".to_owned(),
            format!("{studio}/.renders"),
            "/models".to_owned(),
        ]
    );
}

#[test]
fn the_libraries_come_first_and_then_the_overlays_an_admin_writes() {
    let s = Scratch::new("roots-order");
    let project = s.dir("studio/workspace/ws/p.k7");
    s.dir("libraries/b/1");
    s.dir("libraries/b/draft");
    s.dir("libraries/a");
    s.file("libraries/not-a-folder", "x");
    let at = |home| project_roots(home, &project, "/m", "/o")[3..7].to_vec();
    assert_eq!(
        at(&s.home(false)),
        [
            s.path("libraries/a"),
            s.path("libraries/b"),
            "/m".into(),
            s.path("studio/.renders")
        ]
    );
    assert_eq!(
        project_roots(&s.home(true), &project, "/m", "/o")[3..7],
        [
            s.path("libraries/a"),
            s.path("libraries/b"),
            s.path("libraries/a"),
            s.path("libraries/b/draft")
        ]
    );
}

#[test]
fn a_project_outside_the_workspaces_stands_in_its_parent_folder() {
    let s = Scratch::new("roots-outside");
    let project = s.dir("elsewhere/p.k7");
    let got = project_roots(&s.home(false), &project, "/m", "/o");
    assert_eq!(got[..2], [project, s.path("elsewhere")]);
}
