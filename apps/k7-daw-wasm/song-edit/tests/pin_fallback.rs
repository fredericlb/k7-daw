// pin_fallback.rs — a pinned library version this machine lacks is read at the newest version it holds.
// A reader would get wrong that the pins file is rewritten: only the folder read changes.

use song_edit::confine::Env;
use song_edit::mem_fs::MemFs;
use song_edit::shared_folders::library_root;

fn fs(pins: &str) -> MemFs {
    MemFs::from_tree(
        "/studio",
        [
            ("workspace/ws/_library-pins.k7x".to_owned(), pins.as_bytes().to_vec()),
            ("workspace/ws/song/project.k7x".to_owned(), b"".to_vec()),
            ("libraries/k7-core/2/library.k7x".to_owned(), b"".to_vec()),
            ("libraries/k7-core/8/library.k7x".to_owned(), b"".to_vec()),
            ("libraries/k7-core/draft/library.k7x".to_owned(), b"".to_vec()),
        ],
    )
}

fn root(pins: &str) -> String {
    library_root(
        &fs(pins),
        &Env::at("/studio", "/meta"),
        "k7-core",
        Some("/studio/workspace/ws/song"),
    )
}

#[test]
fn a_missing_pin_reads_the_newest_number_held() {
    assert_eq!(root("k7-core(1)\n"), "/studio/libraries/k7-core/8");
    assert_eq!(root(""), "/studio/libraries/k7-core/8");
}

#[test]
fn a_held_pin_and_a_draft_pin_read_as_they_say() {
    assert_eq!(root("k7-core(2)\n"), "/studio/libraries/k7-core/2");
    assert_eq!(root("k7-core(\"draft\")\n"), "/studio/libraries/k7-core/draft");
}
