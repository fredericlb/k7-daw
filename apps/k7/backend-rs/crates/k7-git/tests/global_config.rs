//! G2: libgit2 never reads the person's own gitconfig, whose `insteadOf` could rewrite a vetted address.

mod common;

#[tokio::test]
async fn the_users_global_gitconfig_is_not_read() {
    let home = common::Dir::new("home");
    home.write(".gitconfig", b"[user]\n\tname = Leak\n[url \"https://evil.example/\"]\n\tinsteadOf = https://good.example/\n");
    // SAFETY: this binary holds one test and nothing has started libgit2 yet.
    unsafe {
        std::env::set_var("HOME", home.path());
        std::env::set_var("XDG_CONFIG_HOME", home.path().join("xdg"));
    }
    let _net = common::net(false);
    let cfg = git2::Config::open_default().unwrap();
    assert!(
        cfg.get_string("user.name").is_err(),
        "the global config was read"
    );
}
