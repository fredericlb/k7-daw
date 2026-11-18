//! The guards every write stands on (1221, 1224): each refusal's sentence and the order they run in, a library's
//! write rule for an admin and not, and the refs a library path answers to.

use crate::support;

use k7_file_service::entries::guards::*;
use k7_file_service::libraries::{Library, library_of};
use k7_file_service::rel::to_rel;
use k7_file_service::space::Space;
use support::entries::{Fx, drive_home};

fn refused(r: Result<(), k7_file_service::entries::EntryError>) -> String {
    r.unwrap_err().to_string()
}

#[test]
fn a_yaml_name_is_refused_whatever_its_case_and_a_k7x_is_not() {
    for bad in ["a.yml", "a.YAML", "dir/b.Yml", "x.yaml"] {
        assert_eq!(
            refuse_yaml(bad).unwrap_err().to_string(),
            format!("{bad}: a workspace file is .k7x, not YAML")
        );
    }
    for fine in ["a.k7x", "a.yml.k7x", "yaml", "a.ymll", "a.yaml/b"] {
        assert!(refuse_yaml(fine).is_ok(), "{fine}");
    }
}

#[test]
fn shared_modules_is_a_segment_by_either_separator() {
    assert!(in_shared_modules("_modules/a"));
    assert!(in_shared_modules("p\\_modules\\a"));
    assert!(in_shared_modules("../_modules"));
    assert!(!in_shared_modules("my_modules/a"));
    assert!(!in_shared_modules("_modules2"));
}

#[test]
fn only_an_admin_s_draft_and_the_account_library_are_writable() {
    let fx = Fx::new("g-writable");
    let user = fx.s.home(false);
    let admin = fx.s.home(true);
    let at = fx.lib("modules/audio/verb");
    let (u, a) = (Space::open(&user, &fx.root), Space::open(&admin, &fx.root));
    let lib = library_of(&user, &at).unwrap();
    assert!(!library_writable(&u, &lib));
    assert!(library_writable(&a, &library_of(&admin, &at).unwrap()));
    let account = Library {
        name: "account".into(),
        root: user.account_home(),
    };
    assert!(library_writable(&u, &account));
    assert_eq!(
        refused(assert_library_writable(&u, &at, "rename")),
        "can't rename '_libraries/k7-core/modules/audio/verb': only the origin's admins edit a library"
    );
    assert!(assert_library_writable(&a, &at, "rename").is_ok());
    // A workspace path is no library's.
    assert!(assert_library_writable(&u, &fx.at("song.k7/clips"), "rename").is_ok());
}

#[test]
fn a_published_version_never_changes_even_for_an_admin() {
    let fx = Fx::new("g-version");
    fx.s.file("libraries/drums/1/modules/a/module.k7x", "x");
    fx.s.dir("libraries/drums/draft/modules");
    let admin = fx.s.home(true);
    let space = Space::open(&admin, &fx.root);
    let published = fx.s.path("libraries/drums/1/modules/a");
    assert_eq!(
        refused(assert_library_writable(&space, &published, "delete")),
        "can't delete '_libraries/drums/modules/a': a published version never changes — edit the draft"
    );
    assert!(
        assert_library_writable(
            &space,
            &fx.s.path("libraries/drums/draft/modules"),
            "delete"
        )
        .is_ok()
    );
}

#[test]
fn the_shared_modules_folder_is_refused_after_the_library_rule() {
    let fx = Fx::new("g-shared");
    let h = fx.s.home(false);
    let space = Space::open(&h, &fx.root);
    assert_eq!(
        refused(guard_shared(
            &space,
            &fx.at("_modules/audio"),
            "create a folder in"
        )),
        "can't create a folder in '_modules/audio': a shared module is edited in place — change its files directly, or run 'module populate' to rebuild it"
    );
    assert!(guard_shared(&space, &fx.at("my_modules/audio"), "x").is_ok());
}

#[test]
fn the_bin_is_the_workspace_s_own_and_a_drive_root_folds_its_case() {
    let fx = Fx::new("g-bin");
    let h = fx.s.home(false);
    let space = Space::open(&h, &fx.root);
    assert!(is_recycled(&space, &fx.at("_recycle_bin")));
    assert!(is_recycled(&space, &fx.at("_recycle_bin/a/b")));
    // A POSIX-rooted home keeps case whatever the host, so this is not the temp folder's rule.
    let posix = k7_file_service::libraries::Home {
        studio: "/srv/studio".into(),
        libraries: "/srv/libs".into(),
        admin: false,
    };
    let p = Space::open(&posix, "/srv/songs");
    assert!(is_recycled(&p, "/srv/songs/_recycle_bin/a"));
    assert!(
        !is_recycled(&p, "/srv/songs/_Recycle_Bin/a"),
        "a POSIX root keeps case"
    );
    assert!(!is_recycled(&space, &fx.at("my_recycle_bin")));
    // A library's bin is not "recycled" in the TypeScript either.
    assert!(!is_recycled(&space, &fx.lib("_recycle_bin/x")));
    let drive = drive_home(false);
    let c = Space::open(&drive, "C:/Work/songs");
    assert!(is_recycled(&c, "c:/work/songs/_RECYCLE_BIN/x"));
    assert!(!is_recycled(&c, "C:/Work/songs/_recycle_bins"));
}

#[test]
fn guard_mutable_refuses_in_the_typescript_s_order() {
    let fx = Fx::new("g-mutable");
    let h = fx.s.home(false);
    let space = Space::open(&h, &fx.root);
    let active = fx.at("song.k7");
    let run = |abs: &str, verb: &str| refused(guard_mutable(&space, Some(&active), abs, verb));
    assert_eq!(run(&fx.root, "rename"), "can't rename the workspace root");
    let lib_say = "the libraries are listed by the admins, not moved from a workspace";
    assert_eq!(
        run(&h.libraries, "move"),
        format!("can't move '../../../libraries': {lib_say}")
    );
    assert_eq!(
        run(&fx.s.path("libraries/k7-core"), "delete"),
        format!("can't delete '_libraries/k7-core': {lib_say}")
    );
    // A library entry the account may not write: the library rule before the bin's and the project's.
    assert_eq!(
        run(&fx.lib("modules"), "rename"),
        "can't rename '_libraries/k7-core/modules': only the origin's admins edit a library"
    );
    assert_eq!(
        run(&fx.at("_modules/x"), "move"),
        "can't move '_modules/x': a shared module is edited in place — change its files directly, or run 'module populate' to rebuild it"
    );
    assert_eq!(
        run(&fx.at("_recycle_bin/x"), "delete"),
        "'_recycle_bin/x' is already in the recycle bin"
    );
    let stop = "the active project's folder — stop it first, then";
    assert_eq!(
        run(&active, "rename"),
        format!("can't rename {stop} rename it")
    );
    // The project's ancestor is refused too: the workspace's own folder is the root, so use a parent.
    let nested = fx.at("band");
    let inner = format!("{nested}/song.k7");
    assert_eq!(
        refused(guard_mutable(&space, Some(&inner), &nested, "move")),
        format!("can't move {stop} move it")
    );
    // Anything INSIDE the project is fair game, and so is everything with no open project.
    assert!(guard_mutable(&space, Some(&active), &fx.at("song.k7/clips"), "move").is_ok());
    assert!(guard_mutable(&space, None, &active, "move").is_ok());
}

#[test]
fn a_drive_root_guards_as_the_shim_does() {
    let drive = drive_home(false);
    let space = Space::open(&drive, "C:/Work/songs");
    let active = "C:/Work/songs/Song.k7";
    assert_eq!(
        refused(guard_mutable(
            &space,
            Some(active),
            "c:/work/songs/song.K7",
            "delete"
        )),
        "can't delete the active project's folder — stop it first, then delete it"
    );
    assert_eq!(
        refused(guard_mutable(
            &space,
            Some(active),
            "C:/Work/songs/_RECYCLE_BIN/a",
            "rename"
        )),
        "'_RECYCLE_BIN/a' is already in the recycle bin"
    );
    assert_eq!(
        refused(guard_mutable(&space, None, "C:/Work/songs", "rename")),
        "can't rename the workspace root"
    );
    assert_eq!(
        refused(guard_mutable(&space, None, "C:/Work/libs", "move")),
        "can't move '../libs': the libraries are listed by the admins, not moved from a workspace"
    );
}

#[test]
fn a_library_path_answers_to_its_virtual_folder_and_a_module_to_its_ref() {
    let fx = Fx::new("g-ref");
    let h = fx.s.home(false);
    let space = Space::open(&h, &fx.root);
    let verb = fx.lib("modules/audio/verb");
    let lib = library_of(&h, &verb).unwrap();
    assert_eq!(
        to_rel(&space, &verb),
        "_libraries/k7-core/modules/audio/verb"
    );
    assert_eq!(
        to_rel(&space, &fx.s.path("libraries/k7-core")),
        "_libraries/k7-core"
    );
    assert_eq!(to_rel(&space, &fx.root), ".");
    assert_eq!(to_rel(&space, &fx.at("song.k7/clips")), "song.k7/clips");
    assert_eq!(ref_at(&lib, &verb), Some("@k7-core/audio/verb".to_string()));
    assert_eq!(
        ref_at(&lib, &fx.lib("modules/audio")),
        Some("@k7-core/audio".to_string())
    );
    assert_eq!(ref_at(&lib, &fx.lib("modules")), None);
    assert_eq!(ref_at(&lib, &fx.lib("chains/a")), None);
    assert!(not_engine(&lib, &verb).is_ok());
    assert_eq!(
        refused(not_engine(
            &lib,
            &fx.lib("modules/audio/verb/engines/plate")
        )),
        "an engine is picked by its name (`engine: <name>` in every node using it), which renaming would strand — make a new engine instead"
    );
}
