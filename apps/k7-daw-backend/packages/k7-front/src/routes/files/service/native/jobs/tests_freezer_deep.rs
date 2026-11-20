//! tests_freezer_deep.rs — the freezer's real compile over a project with VST modules: one, and a chain 120 deep.
//! What a reader gets wrong: no plugin is installed here, so every freeze found is `hosted: false`; the rows are about what the compile FINDS.

use std::path::Path;

use serde_json::json;

use super::freeze_run::home_of;
use super::tests_freezer::{Fixture, fixture};
use crate::routes::files::service::native::docs::{self, Asked, Docs};
use crate::routes::files::service::native::tests_docs::code_root;

/// A project `name` whose chain is `depth` nodes of a `vst3` module `Amp`, from the lane `gtr` to the master.
async fn vst_project(f: &Fixture, name: &str, depth: usize) -> String {
    let dir: &Path = &f.dir;
    let songs = dir.join("home/workspace/songs");
    let module = songs.join("_modules/audio/amp");
    std::fs::create_dir_all(&module).unwrap();
    std::fs::write(
        module.join("module.k7x"),
        "plugin({ type: \"vst3\", name: \"Amp\" })\n",
    )
    .unwrap();
    let project = songs.join(name);
    std::fs::create_dir_all(project.join("nodes")).unwrap();
    std::fs::write(project.join("project.k7x"), "project({ tempo: 120 })\n").unwrap();
    std::fs::write(project.join("nodes/gtr.k7x"), "extends(\"@meta/track\")\n").unwrap();
    let mut graph = String::from("modules({\n");
    let mut from = "gtr".to_owned();
    for i in 0..depth {
        std::fs::write(
            project.join(format!("nodes/a{i}.k7x")),
            "extends(\"@workspace/audio/amp\")\n",
        )
        .unwrap();
        graph += &format!("  {from}: [\"a{i}\"],\n");
        from = format!("a{i}");
    }
    graph += &format!("  {from}: [\"MIX\"],\n}})\n");
    std::fs::write(project.join("graph.k7x"), graph).unwrap();
    let marker = songs.parent().unwrap().join(".active-projects/songs");
    std::fs::create_dir_all(marker.parent().unwrap()).unwrap();
    std::fs::write(marker, format!("{name}\n")).unwrap();
    project.display().to_string()
}

#[tokio::test]
async fn the_real_compile_finds_the_last_vst_of_a_run() {
    let f = fixture("fzd-one", false).await;
    let project = vst_project(&f, "one.k7", 1).await;
    let found = f.freezing.compile(&project).await.unwrap();
    let names: Vec<_> = found.iter().map(|x| x.module.as_str()).collect();
    assert_eq!(names, ["a0"], "{}", json!(found.len()));
    assert!(!found[0].hosted, "no plugin is installed");
}

#[tokio::test]
async fn a_chain_of_a_hundred_and_twenty_vsts_compiles_on_its_own_stack_to_one_freeze() {
    let f = fixture("fzd-deep", false).await;
    let project = vst_project(&f, "deep.k7", 120).await;
    let found = f.freezing.compile(&project).await.unwrap();
    let names: Vec<_> = found.iter().map(|x| x.module.as_str()).collect();
    assert_eq!(names, ["a119"]);
}

/// The docs handle the freezer's compile reads through, over the fixture's jobs.
fn docs_of(f: &Fixture) -> Docs {
    Docs::of(&f._acct, &code_root()).with_thawed(f.jobs.thawed.clone())
}

#[tokio::test]
async fn the_freezers_compile_asks_with_what_the_interactive_asks_and_the_thaw_left() {
    let f = fixture("fzd-ask", false).await;
    let project = vst_project(&f, "ask.k7", 1).await;
    f.jobs.thaw(&project, "a0", true);
    let docs = docs_of(&f);
    let asked = Asked::of_project(&project, None);
    let p = docs::prepare_with(&home_of(&docs), &docs, asked).unwrap();
    assert_eq!(p.ask.thawed, ["a0"]);
}
