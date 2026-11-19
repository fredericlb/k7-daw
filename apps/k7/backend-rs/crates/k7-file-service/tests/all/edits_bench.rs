//! The bench made and kept, and a chain compiled into a slot, replayed from the TypeScript's goldens (`bench.ts`, `slotChain.ts`):
//! the answer or the refusal, then the whole tree of files compared with the one the TypeScript left.

use std::collections::BTreeMap;
use std::sync::Arc;

use k7_file_service::bench::{
    BenchError, Libs, bench_dir, copy_frozen_chain, delete_frozen_chain, ensure_bench, opened,
    save_bench_chain, slot_of,
};
use k7_file_service::libraries::Home;
use k7_file_service::rig::rig;
use serde_json::Value;
use song::session::Session;

use crate::support::Scratch;

fn cases(name: &str) -> Vec<Value> {
    let p = format!(
        "{}/tests/goldens/server/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p}: {e}"));
    serde_json::from_str::<Value>(&text).unwrap()["cases"]
        .as_array()
        .unwrap()
        .clone()
}

fn files_under(dir: &std::path::Path, base: &std::path::Path, out: &mut BTreeMap<String, String>) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            files_under(&p, base, out);
        } else {
            let rel = p
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            out.insert(rel, std::fs::read_to_string(&p).unwrap());
        }
    }
}

fn expected(c: &Value) -> BTreeMap<String, String> {
    let mut out: BTreeMap<String, String> = c["tree"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
        .collect();
    for (k, v) in c["changed"].as_object().unwrap() {
        match v.as_str() {
            Some(t) => out.insert(k.clone(), t.to_owned()),
            None => out.remove(k),
        };
    }
    out
}

fn replay(name: &str, ask: impl Fn(&Libs<'_>, &Home, &Value) -> Result<Value, BenchError>) {
    let all = cases(name);
    assert!(all.len() >= 8);
    for c in &all {
        let label = c["name"].as_str().unwrap();
        let s = Scratch::new(&format!(
            "eb-{name}-{}",
            label.replace(|c: char| !c.is_ascii_alphanumeric(), "_")
        ));
        for (k, v) in c["tree"].as_object().unwrap() {
            s.file(k, v.as_str().unwrap());
        }
        let home = Home {
            studio: s.path("home"),
            libraries: s.path("libraries"),
        };
        let env: BTreeMap<String, String> = [
            ("STUDIO_HOME".to_owned(), home.studio.clone()),
            ("STUDIO_LIBRARIES_HOME".to_owned(), home.libraries.clone()),
        ]
        .into();
        let code = format!("{}/../../../../..", env!("CARGO_MANIFEST_DIR"));
        let vocabulary = |roots: &song::Roots, disk: &song::DiskSource| {
            Session::new(roots.paths).vocabulary(disk, roots).ok()
        };
        let libs = Libs {
            home: &home,
            code_root: &code,
            env: &env,
            vocabulary: &vocabulary,
        };
        let got = ask(&libs, &home, c);
        if let Some(msg) = c["thrown"].as_str() {
            match got {
                Err(BenchError::Refused(m)) => {
                    assert_eq!(m, msg.replace("$DIR", &s.path("")), "{label}")
                }
                other => panic!("{label}: wanted the refusal {msg:?}, got {other:?}"),
            }
        } else {
            let want = serde_json::from_str::<Value>(
                &serde_json::to_string(&c["ret"])
                    .unwrap()
                    .replace("$DIR", &s.path("")),
            )
            .unwrap();
            assert_eq!(
                got.unwrap_or_else(|e| panic!("{label}: {e:?}")),
                want,
                "{label}"
            );
        }
        let mut have = BTreeMap::new();
        files_under(&s.0, &s.0, &mut have);
        assert_eq!(have, expected(c), "{label}");
    }
}

#[test]
fn a_bench_replays_the_typescript() {
    replay("bench", |libs, _, c| {
        ensure_bench(
            libs,
            c["args"]["project"].as_str().unwrap(),
            &Default::default(),
        )
        .map(|()| opened())
    });
}

#[test]
fn a_slot_replays_the_typescript_and_writes_nothing() {
    replay("slot", |libs, home, c| {
        let args = &c["args"];
        let rig = rig(home).unwrap();
        slot_of(
            libs,
            &rig,
            args["ref"].as_str().unwrap(),
            args["savedAt"].as_str().unwrap(),
            "SCRATCH",
        )
    });
}

#[test]
fn the_bench_folder_is_where_the_account_keeps_it() {
    let home = Home {
        studio: "/h".to_owned(),
        libraries: "/h/libraries".to_owned(),
    };
    assert_eq!(bench_dir(&home), "/h/workspace/Banc/live.k7");
    let _ = Arc::new(());
}

fn minted() -> impl FnMut() -> String {
    let mut n = 0;
    move || {
        n += 1;
        format!("00000000-0000-4000-8000-{n:012}")
    }
}

#[test]
fn the_accounts_chains_written_replay_the_typescript() {
    replay("frozen", |libs, home, c| {
        let args = &c["args"];
        let text = |k: &str| {
            args[k]
                .as_str()
                .map(|v| v.replace("$DIR", home.studio.trim_end_matches("/home")))
        };
        match c["fn"].as_str().unwrap() {
            "saveChainFromBench" => {
                let from = text("from").unwrap();
                let saved = save_bench_chain(
                    libs,
                    &from,
                    args["name"].as_str(),
                    &mut minted(),
                    &Default::default(),
                )?;
                Ok(
                    serde_json::json!({ "ref": saved.reference, "steps": saved.steps, "outside": saved.outside }),
                )
            }
            "copyFrozenChain" => copy_frozen_chain(
                libs,
                &text("from").unwrap(),
                &text("name").unwrap(),
                &mut minted(),
                &Default::default(),
            ),
            _ => delete_frozen_chain(libs.home, args["name"].as_str().unwrap())
                .map(|()| Value::Null)
                .map_err(BenchError::Refused),
        }
    });
}

// A step id read from `bench-source.json` names a preset file: a hostile one is replaced by a fresh id and nothing lands outside the account.
#[test]
fn a_hostile_step_id_in_the_bench_source_writes_nothing_outside_the_account() {
    let c = cases("frozen")
        .into_iter()
        .find(|c| c["name"] == "a bench with a preset")
        .unwrap();
    let s = Scratch::new("eb-hostile-id");
    for (k, v) in c["tree"].as_object().unwrap() {
        s.file(k, v.as_str().unwrap());
    }
    let source = "home/workspace/Banc/bench-source.json";
    let text = std::fs::read_to_string(s.path(source)).unwrap();
    s.file(
        source,
        &text.replace("77777777-7777-4777-8777-777777777777", "../../escape"),
    );
    let home = Home {
        studio: s.path("home"),
        libraries: s.path("libraries"),
    };
    let env: BTreeMap<String, String> = [
        ("STUDIO_HOME".to_owned(), home.studio.clone()),
        ("STUDIO_LIBRARIES_HOME".to_owned(), home.libraries.clone()),
    ]
    .into();
    let code = format!("{}/../../../../..", env!("CARGO_MANIFEST_DIR"));
    let vocabulary = |roots: &song::Roots, disk: &song::DiskSource| {
        Session::new(roots.paths).vocabulary(disk, roots).ok()
    };
    let libs = Libs {
        home: &home,
        code_root: &code,
        env: &env,
        vocabulary: &vocabulary,
    };
    let from = bench_dir(&home);
    save_bench_chain(
        &libs,
        &from,
        Some("hostile"),
        &mut minted(),
        &Default::default(),
    )
    .unwrap();
    let mut have = BTreeMap::new();
    files_under(&s.0, &s.0, &mut have);
    assert!(!have.keys().any(|k| k.contains("escape")), "{have:?}");
    let chain = &have["home/library/chains/hostile.k7x"];
    assert!(!chain.contains(".."), "{chain}");
}

// A chain that landed is never a decline: a vocabulary that fails after the write is a refusal, with the written flag set.
#[test]
fn a_decline_after_the_chain_landed_is_a_refusal_not_a_handback() {
    let c = cases("frozen")
        .into_iter()
        .find(|c| c["name"] == "a chain with a preset copied")
        .unwrap();
    let s = Scratch::new("eb-late-decline");
    for (k, v) in c["tree"].as_object().unwrap() {
        s.file(k, v.as_str().unwrap());
    }
    let home = Home {
        studio: s.path("home"),
        libraries: s.path("libraries"),
    };
    let env: BTreeMap<String, String> = [
        ("STUDIO_HOME".to_owned(), home.studio.clone()),
        ("STUDIO_LIBRARIES_HOME".to_owned(), home.libraries.clone()),
    ]
    .into();
    let code = format!("{}/../../../../..", env!("CARGO_MANIFEST_DIR"));
    let landed = s.path("home/library/chains/late.k7x");
    let vocabulary = |roots: &song::Roots, disk: &song::DiskSource| {
        if std::path::Path::new(&landed).exists() {
            return None;
        }
        Session::new(roots.paths).vocabulary(disk, roots).ok()
    };
    let libs = Libs {
        home: &home,
        code_root: &code,
        env: &env,
        vocabulary: &vocabulary,
    };
    let wrote = std::sync::atomic::AtomicBool::new(false);
    let got = copy_frozen_chain(&libs, "@k7-core/withpreset", "late", &mut minted(), &wrote);
    assert!(matches!(got, Err(BenchError::Refused(_))), "{got:?}");
    assert!(
        wrote.load(std::sync::atomic::Ordering::SeqCst),
        "the flag follows the commit"
    );
    assert!(std::path::Path::new(&s.path("home/library/chains/late.k7x")).exists());
}
