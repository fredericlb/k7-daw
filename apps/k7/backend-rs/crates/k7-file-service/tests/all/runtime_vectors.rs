//! 2604 replayed: every case of `vectors/runtime.json`, made by `js/makeRuntimeVectors.mts` from the TypeScript's own functions.
//! Ids are compared by order of first sight in the background section, since its scenarios share one module there; `-0` prints as 0.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use k7_file_service::runtime::background::{Jobs, tag_label};
use k7_file_service::runtime::list::Table;
use k7_file_service::runtime::logs::{num, tail_of};
use k7_file_service::runtime::ring::{Level, Ring};
use serde_json::{Value, json};

fn vectors() -> Value {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/vectors/runtime.json"
    ))
    .unwrap();
    serde_json::from_str(&text).unwrap()
}

#[test]
fn num_and_tail_read_a_query_as_javascript_does() {
    let all = vectors();
    for (key, n) in [("num", 49), ("tail", 49)] {
        let rows = all[key].as_array().unwrap();
        assert!(rows.len() >= n, "{key}: {}", rows.len());
        for r in rows {
            let q = r["q"].as_str();
            let got = if key == "num" {
                num(q).map(|v| json!(v)).unwrap_or(Value::Null)
            } else {
                json!(tail_of(q))
            };
            // JSON has one zero and one number type: compare as numbers.
            assert_eq!(got.as_f64(), r["out"].as_f64(), "{key} {q:?}");
        }
    }
}

fn pushes(n: usize) -> Ring {
    let texts = [
        "boot",
        "",
        "two\nlines",
        "quo\"te",
        "é e\u{301}",
        "😀 astral",
        "[cmd] ok",
    ];
    let long = "x".repeat(40);
    let mut ring = Ring::new();
    for i in 0..n {
        let level = [Level::Info, Level::Warn, Level::Error][i % 3];
        let t = if i % 8 == 7 { &long } else { texts[i % 8] };
        ring.push(
            level,
            &format!("{i}: {t}"),
            1_700_000_000_000 + i as u64 * 1000 + (i as u64 % 3),
        );
    }
    ring
}

fn replay_since(section: &str, whole: bool) {
    let all = vectors();
    let rows = all[section].as_array().unwrap();
    assert!(rows.len() >= 45, "{section}: {}", rows.len());
    let mut rings: HashMap<u64, Ring> = HashMap::new();
    for r in rows {
        let n = r["pushes"].as_u64().unwrap();
        let ring = rings.entry(n).or_insert_with(|| pushes(n as usize));
        let after = r["after"].as_f64();
        let (seq, lines) = ring.since(after, r["tail"].as_f64().unwrap().trunc() as usize);
        let want = &r["out"];
        if whole {
            let got = json!({ "seq": seq, "lines": lines.iter().map(|l| l.to_json()).collect::<Vec<_>>() });
            assert_eq!(&got, want, "{r}");
        } else {
            let first = lines.first().map(|l| l.seq);
            let last = lines.last().map(|l| l.seq);
            assert_eq!(
                json!([seq, lines.len(), first, last]),
                json!([want["seq"], want["count"], want["first"], want["last"]]),
                "{r}"
            );
        }
    }
}

#[test]
fn a_small_ring_answers_whole_as_the_service_log_does() {
    replay_since("since_small", true);
}

#[test]
fn a_big_ring_answers_its_count_and_ends() {
    replay_since("since_big", false);
}

#[test]
fn labels_shorten_the_path_as_the_regex_does() {
    let all = vectors();
    let rows = all["label"].as_array().unwrap();
    assert!(rows.len() >= 30);
    for r in rows {
        let tag = r["tag"].as_str().unwrap();
        assert_eq!(tag_label(tag), r["out"].as_str().unwrap(), "{tag:?}");
    }
}

#[test]
fn the_process_table_lists_as_list_processes_does() {
    let all = vectors();
    for sc in all["processes"].as_array().unwrap() {
        let mut t = Table::new();
        let mut ids: HashMap<u64, String> = HashMap::new();
        for s in sc["steps"].as_array().unwrap() {
            let at = s["at"].as_u64().unwrap();
            let p = s["p"].as_u64().unwrap_or(0);
            match s["op"].as_str().unwrap() {
                "start" => {
                    let id = t.start(
                        s["label"].as_str().unwrap(),
                        s["showAfter"].as_u64().unwrap(),
                        at,
                    );
                    ids.insert(p, id);
                }
                "progress" => t.progress(&ids[&p], s["fraction"].as_f64(), s["detail"].as_str()),
                "end" => {
                    let refused = s["result"].as_str().filter(|r| r.starts_with("refused"));
                    let err = s["error"].as_str().or(s["thrown"].as_str()).or(refused);
                    t.end(&ids[&p], err, at);
                }
                _ => {
                    let got: Vec<Value> = t.list(at).iter().map(|e| e.to_json()).collect();
                    assert_eq!(Value::Array(got), s["out"], "{} at {at}", sc["name"]);
                }
            }
        }
    }
}

#[test]
fn background_jobs_list_and_say_as_run_in_background_does_but_a_late_progress_is_dropped() {
    let all = vectors();
    for (k, sc) in all["background"].as_array().unwrap().iter().enumerate() {
        let name = sc["name"].as_str().unwrap();
        let origin = Instant::now();
        // Each scenario's clock starts at (k + 1) million, which the list's `startedAt` counts from.
        let mut jobs = Jobs::new(origin, (k as u64 + 1) * 1_000_000);
        let mut tags: HashMap<u64, String> = HashMap::new();
        let mut order: HashMap<String, String> = HashMap::new();
        for s in sc["steps"].as_array().unwrap() {
            let now = origin + Duration::from_millis(s["at"].as_u64().unwrap());
            let p = s["p"].as_u64().unwrap_or(0);
            let mut said: Vec<String> = Vec::new();
            match s["op"].as_str().unwrap() {
                "bg" => {
                    let tag = s["tag"].as_str().unwrap();
                    tags.insert(p, tag.into());
                    assert_eq!(json!(jobs.start(tag, now)), s["out"], "{name}");
                }
                "progress" => {
                    jobs.progress(&tags[&p], s["fraction"].as_f64(), s["detail"].as_str(), now)
                }
                "finish" => {
                    said.extend(jobs.end(&tags[&p], Ok(s["line"].as_str().unwrap().into()), now))
                }
                "reject" => {
                    said.extend(jobs.end(&tags[&p], Err(s["thrown"].as_str().unwrap().into()), now))
                }
                _ => {
                    let got: Vec<Value> = jobs
                        .list(now)
                        .iter()
                        .map(|e| {
                            let n = order.len();
                            let mut v = e.to_json();
                            let name = order
                                .entry(e.id.clone())
                                .or_insert_with(|| format!("#{}", n + 1))
                                .clone();
                            v["id"] = json!(name);
                            v
                        })
                        .collect();
                    let mut want = s["out"].clone();
                    if name.starts_with("a progress after the end") {
                        for e in want.as_array_mut().unwrap() {
                            let o = e.as_object_mut().unwrap();
                            o.remove("progress");
                            o.remove("detail");
                        }
                    }
                    assert_eq!(Value::Array(got), want, "{name} at {}", s["at"]);
                }
            }
            assert_eq!(json!(said), s["lines"], "{name} at {}", s["at"]);
        }
    }
}
