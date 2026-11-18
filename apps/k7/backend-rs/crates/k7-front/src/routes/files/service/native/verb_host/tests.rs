//! The host routes' frame (3f): the gates every member passes, the poll, and a driver of the test's own behind them.

use std::sync::Arc;

use hyper::Method;
use serde_json::{Value, json};

use super::{Answer, Checked, Driver, Member, member};
use crate::routes::files::json_answer;
use crate::routes::files::service::Acct;
use crate::routes::files::service::native::tests::Lines;
use crate::routes::files::service::native::tests_account::{call, text};
use crate::routes::files::service::native::tests_docs::{answering, world};
use crate::routes::files::service::native::{Ctx, Tag};
use crate::routes::files::service::routes::{self, H};

/// Answers a peak with the real path it read, a render with a started job.
struct Echo;

impl Driver for Echo {
    fn call<'a>(&'a self, req: &'a Checked) -> Answer<'a> {
        Box::pin(async move {
            match req.member {
                Member::Peak => match req.text("file").and_then(|f| req.read(f)) {
                    Ok(real) => json_answer(200, &json!({ "db": -3.0, "real": real })),
                    Err(r) => *r,
                },
                Member::Render => match req.start() {
                    Ok((id, _)) => Checked::accepted(&id),
                    Err(r) => *r,
                },
                _ => json_answer(200, &json!({})),
            }
        })
    }
}

pub(super) struct Bed {
    pub dir: std::path::PathBuf,
    pub acct: Arc<Acct>,
    pub lines: Lines,
}

pub(super) async fn bed(name: &str) -> Bed {
    let (dir, acct) = world(name).await;
    let marker = dir.join("home/workspace/.active-projects");
    std::fs::create_dir_all(&marker).unwrap();
    std::fs::write(marker.join("songs"), "p.k7").unwrap();
    Bed {
        dir,
        acct,
        lines: Lines::default(),
    }
}

impl Bed {
    pub(super) fn ctx(&self, m: Member, driver: bool) -> Ctx {
        let mut c = answering(&self.acct, &self.dir, Tag::VerbHost(m), &self.lines);
        c.verbs = driver.then(|| Arc::new(Echo) as Arc<dyn Driver>);
        c
    }

    async fn go(&self, m: Member, account: &str, query: &str, body: Value) -> (u16, Value) {
        let mut ctx = self.ctx(m, true);
        ctx.account = account.into();
        self.send(m, query, body, Some(ctx)).await
    }

    pub(super) async fn send(
        &self,
        m: Member,
        query: &str,
        body: Value,
        ctx: Option<Ctx>,
    ) -> (u16, Value) {
        let c = call(&self.dir, query, body, ctx);
        let (status, t) = text(routes::dispatch(H::VerbHost(m), &c).await).await;
        (status, serde_json::from_str(&t).unwrap_or(Value::Null))
    }
}

#[test]
fn the_table_names_each_route_once_with_its_limit() {
    for m in member::ALL {
        assert_eq!(Member::of(&m.method(), m.path()), Some(m), "{m:?}");
        assert_eq!(m.route(), format!("{} {}", m.method(), m.path()));
        assert_eq!(Tag::VerbHost(m).route(), m.route());
        assert_eq!(Tag::VerbHost(m).piece(), super::super::Piece::Journal);
        // The limit is what the service reads the body under.
        assert_eq!(routes::body_limit(H::VerbHost(m)), m.limit());
    }
    let big = [Member::Render, Member::Picture];
    for m in member::ALL {
        let want = if big.contains(&m) { 8 << 20 } else { 64 << 10 };
        assert_eq!(m.limit(), want, "{m:?}");
    }
    assert_eq!(routes::body_limit(H::Changes), 8 << 20);
    // A wrong method or an unknown path is no member: the hooks have it, or it is a 404.
    assert_eq!(Member::of(&Method::GET, "/verb-host/render"), None);
    assert_eq!(Member::of(&Method::POST, "/verb-host/nothing"), None);
    assert_eq!(
        routes::route(&Method::POST, "/verb-host/peak"),
        routes::Route::Handler(H::VerbHost(Member::Peak))
    );
    assert_eq!(
        routes::route(&Method::GET, "/verb-host/peak"),
        routes::Route::Hooked
    );
}

// 3f-2: a host member with no driver or no compiler answers 503 `off`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_route_with_no_driver_or_no_journal_is_a_503_off() {
    let b = bed("vh-off").await;
    let no_driver = b.ctx(Member::Peak, false);
    let (s, a) = b
        .send(
            Member::Peak,
            "",
            json!({ "file": "/w/p.k7/project.k7x" }),
            Some(no_driver),
        )
        .await;
    assert_eq!((s, a["code"].as_str()), (503, Some("off")), "{a}");
    let mut no_docs = b.ctx(Member::Peak, true);
    no_docs.docs = None;
    let (s, a) = b.send(Member::Peak, "", json!({}), Some(no_docs)).await;
    assert_eq!((s, a["code"].as_str()), (503, Some("off")), "{a}");
    let _ = std::fs::remove_dir_all(&b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_read_under_the_roots_passes_and_any_other_is_a_403() {
    let b = bed("vh-403").await;
    let (s, a) = b
        .go(
            Member::Peak,
            "acct",
            "",
            json!({ "file": "/w/p.k7/project.k7x" }),
        )
        .await;
    assert_eq!(s, 200, "{a}");
    assert!(a["real"].as_str().unwrap().ends_with("p.k7/project.k7x"));
    let real = b
        .dir
        .join("home/workspace/songs/p.k7/project.k7x")
        .display()
        .to_string();
    for bad in [
        "/etc/passwd",
        real.as_str(),
        "/w/../../etc/passwd",
        "/w/p.k7/../../../../etc/passwd",
        "p.k7/project.k7x",
    ] {
        let (s, a) = b.go(Member::Peak, "acct", "", json!({ "file": bad })).await;
        assert_eq!(s, 403, "{bad}: {a}");
    }
    let (s, _) = b.go(Member::Peak, "acct", "", json!({ "file": 5 })).await;
    assert_eq!(s, 400);
    let _ = std::fs::remove_dir_all(&b.dir);
}

// `at` names the open project; another is a 409 that says which one is open, and no job starts.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn another_project_is_a_409_and_a_missing_at_a_400() {
    let b = bed("vh-409").await;
    let (s, a) = b
        .go(Member::Render, "acct-409", "", json!({ "at": "other.k7" }))
        .await;
    assert_eq!((s, a["open"].as_str()), (409, Some("p.k7")), "{a}");
    let (s, _) = b.go(Member::Render, "acct-409", "", json!({})).await;
    assert_eq!(s, 400);
    let (s, a) = b
        .go(Member::Render, "acct-409", "", json!({ "at": "p.k7" }))
        .await;
    assert_eq!(s, 202, "{a}");
    let _ = std::fs::remove_dir_all(&b.dir);
}

// A job is answered 202 and polled; its id is its account's alone, and a guess or a forgotten one is a 404.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_started_job_is_polled_by_its_account_alone() {
    let b = bed("vh-poll").await;
    let (s, a) = b
        .go(Member::Render, "ann", "", json!({ "at": "p.k7" }))
        .await;
    assert_eq!(s, 202, "{a}");
    let id = a["job"].as_str().unwrap().to_string();
    let q = format!("id={id}");
    let (s, a) = b.go(Member::Job, "ann", &q, json!({})).await;
    assert_eq!((s, a["state"].as_str()), (200, Some("running")), "{a}");
    let (s, _) = b.go(Member::Job, "bob", &q, json!({})).await;
    assert_eq!(s, 404);
    for q in ["id=00000000000000000000000000000000", "id=", ""] {
        assert_eq!(b.go(Member::Job, "ann", q, json!({})).await.0, 404, "{q}");
    }
    k7_file_service::verb_host::jobs::Table::global().finish(
        &id,
        json!({ "files": [] }),
        super::now(),
    );
    let (_, a) = b.go(Member::Job, "ann", &q, json!({})).await;
    assert_eq!(a, json!({ "state": "done", "value": { "files": [] } }));
    let _ = std::fs::remove_dir_all(&b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_account_past_its_jobs_is_a_503_busy() {
    let b = bed("vh-busy").await;
    let mut last = (0, Value::Null);
    for _ in 0..=k7_file_service::verb_host::jobs::MAX_RUNNING {
        last = b
            .go(Member::Render, "carl", "", json!({ "at": "p.k7" }))
            .await;
    }
    assert_eq!(
        (last.0, last.1["code"].as_str()),
        (503, Some("busy")),
        "{}",
        last.1
    );
    let _ = std::fs::remove_dir_all(&b.dir);
}

// A 409 names the open project under the root; one outside it is shown as nothing, never as a real path.
#[test]
fn the_open_project_is_shown_only_under_the_root() {
    assert_eq!(
        super::shown_under("/r/w/", "/r/w/p.k7").as_deref(),
        Some("p.k7")
    );
    assert_eq!(super::shown_under("/r/w", "/elsewhere/p.k7"), None);
    // A backslash root (the host's spelling) still names the project by its place.
    assert_eq!(
        super::shown_under("C:\\ws\\songs", "C:/ws/songs/p.k7").as_deref(),
        Some("p.k7")
    );
    assert_eq!(super::shown_under("/r/w", "/r/wx/p.k7"), None);
    // A Windows root is spelled with backslashes; a drive letter of another case is not shown either.
    assert_eq!(
        super::shown_under(r"C:\ws\songs", "C:/ws/songs/p.k7").as_deref(),
        Some("p.k7")
    );
    assert_eq!(super::shown_under(r"C:\ws", "c:/ws/p.k7"), None);
}

// The heartbeat's ticker belongs to no runtime: one made under a runtime that is gone is replaced.
#[test]
fn the_ticker_is_respawned_once_its_runtime_is_gone() {
    let alive = || {
        super::TICKER
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|h| !h.is_finished())
    };
    let first = tokio::runtime::Runtime::new().unwrap();
    first.block_on(async { super::ensure_ticker() });
    assert!(alive());
    drop(first);
    let second = tokio::runtime::Runtime::new().unwrap();
    second.block_on(async {
        super::ensure_ticker();
        assert!(alive(), "the first runtime's ticker died with it");
    });
}

// A member that reads no path needs no open project; one that does is told none is open.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_account_member_needs_no_open_project_but_a_path_member_does() {
    let b = bed("vh-noproject").await;
    std::fs::remove_file(b.dir.join("home/workspace/.active-projects/songs")).unwrap();
    let (s, a) = b.go(Member::Devices, "pa", "", json!({})).await;
    assert_eq!((s, a), (200, json!({})));
    let (s, a) = b
        .go(Member::Peak, "pa", "", json!({ "file": "/w/p.k7/a.wav" }))
        .await;
    assert_eq!((s, a["open"].clone()), (409, Value::Null), "{a}");
    let _ = std::fs::remove_dir_all(&b.dir);
}
