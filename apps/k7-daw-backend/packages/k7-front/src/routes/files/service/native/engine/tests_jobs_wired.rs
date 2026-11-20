//! tests_jobs_wired.rs — the account's jobs held by its engine (2306): a docs ask reads the jobs' thawed list.
//! What a reader gets wrong: without `hold_jobs` an ask's thawed list is not the jobs'.

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::tests_real::account;
use crate::routes::files::service::native::tests::Lines;
use crate::routes::files::service::native::{Req, Tag, ctx_for};

fn code_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../..")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_docs_route_reads_the_thawed_list_of_the_jobs_the_engine_holds() {
    let a = account("jobs-docs").await;
    let engine = a.engine.clone().unwrap();
    engine.hold_jobs(&BTreeMap::new(), &code_root());
    let jobs = engine.jobs.get().unwrap().clone();
    jobs.thaw("/w/p.k7", "gtr", true);
    let root = code_root();
    let env = BTreeMap::new();
    let log = crate::routes::files::service::native::tests::log_to(&Lines::default());
    let ask = |thawed| {
        let req = Req {
            env: &env,
            slug: "me",
            workspace: "songs",
            root: "/r",
            query: "",
            body: &serde_json::Value::Null,
            log: &log,
            docs: Some((&a.acct, root.as_path())),
            thawed,
        };
        ctx_for(Some(Tag::ProjectDoc), &req).unwrap().docs.unwrap()
    };
    let held = ask(Some(&jobs.thawed)).thawed.unwrap();
    assert_eq!(held.of("/w/p.k7"), ["gtr"]);
    assert!(ask(None).thawed.is_none());
}
