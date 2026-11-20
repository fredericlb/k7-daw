//! The link over the real parts: the account, the song crate's own compile, a fake engine. What an open starts, the front
//! compiles itself and sends, and what a verb asks follows the document in the order asked (2216, 2247).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use k7_file_service::libraries::Home;
use serde_json::{Value, json};

use super::fake::{FakeEngine, until};
use super::real::RealHost;
use super::routes::Engine;

pub struct Account {
    pub dir: PathBuf,
    pub ws: String,
    pub acct: Arc<crate::routes::files::service::Acct>,
    pub engine: Option<Arc<Engine>>,
    pub fake: FakeEngine,
}

/// A path in the spelling the front keys its roots by: slashes only.
fn slashed(p: &std::path::Path) -> String {
    p.display().to_string().replace('\\', "/")
}

pub const SESSION: &str = "k7:a_b_c";

pub async fn account(tag: &str) -> Account {
    make(tag, false).await
}

/// The project holding one lane `gtr` and a recorded file beside `clips/`, which a take can be filed into.
pub async fn account_with_lane(tag: &str) -> Account {
    make(tag, true).await
}

async fn make(tag: &str, lane: bool) -> Account {
    let dir = crate::routes::files::service::native::tests::tmp_root()
        .join(format!("k7-real-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let project = dir.join("home/workspace/songs/p.k7");
    std::fs::create_dir_all(&project).unwrap();
    if lane {
        std::fs::create_dir_all(project.join("nodes")).unwrap();
        std::fs::create_dir_all(project.join("clips")).unwrap();
        std::fs::write(project.join("project.k7x"), "project({ tempo: 120 })\n").unwrap();
        std::fs::write(project.join("nodes/gtr.k7x"), "extends(\"@meta/track\")\n").unwrap();
        let io = dir.join("home/workspace/songs/_modules/io/IN");
        std::fs::create_dir_all(&io).unwrap();
        std::fs::write(io.join("module.k7x"), "io(\"in\")\n").unwrap();
        std::fs::write(io.join("in-1.k7x"), "channel([1])\n").unwrap();
        std::fs::write(
            project.join("graph.k7x"),
            "modules({ gtr: [\"@workspace/io/IN:in-1\"], \"@workspace/io/IN:in-1\": [\"MIX\"] })\n",
        )
        .unwrap();
        let core = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../k7-core");
        std::fs::create_dir_all(dir.join("libraries")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(core, dir.join("libraries/k7-core")).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(core, dir.join("libraries/k7-core")).unwrap();
    } else {
        // A project the song crate compiles: the native compile refuses one with no graph, no modules and no chains.
        std::fs::create_dir_all(project.join("nodes")).unwrap();
        std::fs::write(project.join("nodes/gtr.k7x"), "extends(\"@meta/track\")\n").unwrap();
        std::fs::write(project.join("graph.k7x"), "modules({ gtr: [\"MIX\"] })\n").unwrap();
        std::fs::write(project.join("project.k7x"), "project({})\n").unwrap();
    }
    let ws = slashed(&dir.join("home/workspace/songs"));
    let env: BTreeMap<String, String> = [
        ("STUDIO_HOME", slashed(&dir.join("home"))),
        ("STUDIO_LIBRARIES_HOME", slashed(&dir.join("libraries"))),
        ("STUDIO_AUTO_FREEZE", "off".to_string()),
        ("TEMP", slashed(&std::env::temp_dir())),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    let code_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../..");
    let files_root = slashed(&dir.join("home/workspace"));
    let acct = crate::routes::files::service::Acct::make(
        env.clone(),
        crate::routes::files::service::native::tests_account::HEAP,
    );
    let host = RealHost::new(
        Home::of(&env),
        code_root.display().to_string(),
        acct.clone(),
    );
    let fake = FakeEngine::start().await;
    let engine = Engine::build(
        files_root.clone(),
        "a@b.c",
        vec![files_root],
        host,
        Arc::new(|m: &str| eprintln!("{m}")),
    );
    engine
        .link
        .bind(&json!({ "url": fake.url, "engineId": "e1" }));
    Account {
        dir,
        ws,
        acct,
        engine: Some(engine),
        fake,
    }
}

impl Drop for Account {
    fn drop(&mut self) {
        if let Some(e) = &self.engine {
            e.link.close();
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Account {
    pub fn kinds(&self) -> Vec<String> {
        self.fake.kinds(SESSION)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_open_is_compiled_by_the_front_and_sent_as_the_document() {
    let a = account("open").await;
    a.open("p.k7");
    a.start().await;
    until(30, "the document", || {
        a.kinds().iter().any(|k| k == "project")
    })
    .await;
    let sent = a.fake.of(SESSION);
    let project = sent.iter().find(|f| f["type"] == "project").unwrap();
    assert!(project["doc"]["tracks"].is_array(), "{project}");
}

impl Account {
    /// The hub's binding told to the link.
    pub fn bind(&self, told: &Value) {
        self.engine.as_ref().expect("the link").link.bind(told);
    }

    /// `project` marked as the workspace's open project, as the open route does before it starts the engine.
    pub fn open(&self, project: &str) {
        let marker = PathBuf::from(&self.ws)
            .parent()
            .unwrap()
            .join(".active-projects/songs");
        std::fs::create_dir_all(marker.parent().unwrap()).unwrap();
        std::fs::write(marker, format!("{project}\n")).unwrap();
    }

    /// The engine started on the open project, as the open route asks.
    pub async fn start(&self) -> Value {
        let e = self.engine.as_ref().expect("the link");
        e.start_engine(&self.ws, true).await
    }

    /// What 1g's release does: the served project let go from the link.
    pub async fn unload(&self) {
        let e = self.engine.as_ref().expect("the link");
        e.link.unload_under(&self.ws).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_project_that_will_not_compile_is_the_findings_sentence() {
    use super::Host;
    let a = account_with_lane("compile-failure").await;
    let project = format!("{}/p.k7", a.ws);
    std::fs::write(format!("{project}/project.k7x"), "project({ tempo: \n").unwrap();
    let engine = a.engine.clone().unwrap();
    let (ws, dir) = (a.ws.clone(), project.clone());
    let ours = tokio::task::spawn_blocking(move || engine.host.compile(&ws, &dir))
        .await
        .unwrap();
    let ours = ours.expect_err("a project that will not compile has no document");
    // The sentence the compile throws with the native compiler on, as the docs routes word it (`tests_refuse.rs`).
    assert_eq!(ours, "project.k7x:2: expected a value");
}
