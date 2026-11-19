//! A workspace pinned afresh (3j-1's last decline, now native): the newest version of each library this machine runs,
//! written once, named in order, never over a file that is there,.

use crate::support;

use k7_file_service::pins::{ensure_fresh, ensure_opened};
use support::Scratch;

fn manifest(version: u32, requires: u32) -> String {
    format!("name(\"lib\")\nversion({version})\nrequires({requires})\n")
}

#[test]
fn a_workspace_with_no_pins_gets_the_newest_runnable_version_of_each_library_in_name_order() {
    let s = Scratch::new("pins-fresh");
    for (name, v, req) in [
        ("zed", 1, 1),
        ("k7-core", 1, 1),
        ("k7-core", 2, 1),
        ("k7-core", 3, 99),
    ] {
        s.file(
            &format!("libraries/{name}/{v}/library.k7x"),
            &manifest(v, req),
        );
    }
    let root = s.path("home/workspace/Banc");
    std::fs::create_dir_all(&root).unwrap();
    assert_eq!(ensure_fresh(&s.home(false), &root), Ok(true));
    let text = std::fs::read_to_string(format!("{root}/_library-pins.k7x")).unwrap();
    // Version 3 asks for a level this k7 lacks, so 2 is the newest it runs.
    assert_eq!(text, "k7-core(2)\nzed(1)\n");
    // Written once: a second call leaves what is there.
    std::fs::write(format!("{root}/_library-pins.k7x"), "k7-core(1)\n").unwrap();
    assert_eq!(ensure_fresh(&s.home(false), &root), Ok(false));
    assert_eq!(
        std::fs::read_to_string(format!("{root}/_library-pins.k7x")).unwrap(),
        "k7-core(1)\n"
    );
}

#[test]
fn an_open_pins_version_one_and_an_admins_open_pins_the_draft_never_over_a_file() {
    let s = Scratch::new("pins-opened");
    s.file("libraries/k7-core/2/library.k7x", &manifest(2, 1));
    s.file("libraries/zed/1/library.k7x", &manifest(1, 1));
    let (plain, admin) = (s.path("home/workspace/A"), s.path("home/workspace/B"));
    for r in [&plain, &admin] {
        std::fs::create_dir_all(r).unwrap();
    }
    assert_eq!(ensure_opened(&s.home(false), &plain), Ok(true));
    assert_eq!(ensure_opened(&s.home(true), &admin), Ok(true));
    let read = |r: &str| std::fs::read_to_string(format!("{r}/_library-pins.k7x")).unwrap();
    // Version 1 is the workspace from before versions, whatever the newest is.
    assert_eq!(read(&plain), "k7-core(1)\nzed(1)\n");
    assert_eq!(read(&admin), "k7-core(\"draft\")\nzed(\"draft\")\n");
    assert_eq!(ensure_opened(&s.home(true), &plain), Ok(false));
    assert_eq!(read(&plain), "k7-core(1)\nzed(1)\n");
}
