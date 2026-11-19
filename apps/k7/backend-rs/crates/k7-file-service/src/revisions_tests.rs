//! revisions_tests.rs — what only the memo's insides can show: a walk in flight seen from an ask that follows a bump.

use std::sync::mpsc;
use std::time::Duration;

use super::*;

struct Still;

impl Feed for Still {
    fn generation(&self, _root: &str) -> Option<u64> {
        Some(0)
    }
}

#[test]
fn an_ask_after_a_bump_never_waits_for_a_walk_begun_before_it() {
    // 1346: the flight below never ends; joining it would answer the state before the write, or hang
    let base = std::env::temp_dir().join(format!("k7-revs-flight-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(base.join("ws/p.k7")).unwrap();
    std::fs::write(base.join("ws/p.k7/a.k7x"), "a").unwrap();
    let at = |rel: &str| base.join(rel).to_string_lossy().replace('\\', "/");
    let (root, dir) = (at("ws"), at("ws/p.k7"));
    let home = Home {
        studio: at("studio"),
        libraries: at("libraries"),
    };
    let m: &'static Revisions = Box::leak(Box::new(Revisions::with_feed(
        Box::new(Still),
        Instant::now,
    )));
    let before = m.token(&home, &root, Some(&dir), true);
    {
        let mut roots = m.roots.lock().unwrap();
        let r = roots.get_mut(&root).unwrap();
        r.flight = Some(Arc::new(Flight {
            dir: dir.clone(),
            mutations: r.mutations,
            done: Mutex::new(None),
            cv: Condvar::new(),
        }));
    }
    std::fs::write(base.join("ws/p.k7/b.k7x"), "b").unwrap();
    m.bump(&root);
    let (tx, rx) = mpsc::channel();
    let (h, r, d) = (home.clone(), root.clone(), dir.clone());
    std::thread::spawn(move || {
        let _ = tx.send(m.token(&h, &r, Some(&d), false));
    });
    let after = rx
        .recv_timeout(Duration::from_secs(30))
        .expect("the ask joined a walk begun before the bump");
    assert_ne!(after, before);
    let _ = std::fs::remove_dir_all(&base);
}
