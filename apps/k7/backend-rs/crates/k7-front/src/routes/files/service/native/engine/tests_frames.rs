//! The engine link's frame traces, recorded once from node's side and replayed against the front's link alone.
//! What a reader gets wrong: a replay never writes the fixtures; they are node's frames, kept as they were.

use std::path::PathBuf;

use serde_json::Value;

use super::tests_real::account_with_lane;
use super::tests_scenarios::{
    binding_scenario, client_scenario, copy_scenario, recycle_scenario, take_scenario,
};
use super::tests_scenarios_jobs::{
    armed_take_scenario, dropped_take_scenario, edit_while_thawed_scenario, preview_scenario,
    queries_scenario, thaw_scenario,
};
use super::tests_scene::{
    Compared, Scenario, brief, save_scenario, settled, switch_scenario, unload_scenario,
};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/frames/{name}.json"))
}

fn recorded(name: &str) -> Vec<Value> {
    let text = std::fs::read_to_string(fixture(name))
        .unwrap_or_else(|e| panic!("{name}: no recorded trace ({e})"));
    serde_json::from_str(&text).unwrap()
}

/// Where a trace parts from the recorded one, in the one-line form of a frame.
pub(super) fn compare(name: &str, got: &[Value], want: &[Value]) -> Result<(), String> {
    if got == want {
        return Ok(());
    }
    let at = got
        .iter()
        .zip(want)
        .position(|(x, y)| x != y)
        .unwrap_or(got.len().min(want.len()));
    let line = |v: &[Value]| v.iter().map(brief).collect::<Vec<_>>().join(", ");
    Err(format!(
        "{name}: the frames part at {at}\n  got:      {}\n  recorded: {}",
        line(got),
        line(want)
    ))
}

/// The front's counts, as the recording bounds them: no open window, and what one dry run plays with at most.
fn bounded(name: &str, c: &Compared, opens: usize, resends: usize) {
    assert!(
        c.collapsed + c.strays <= c.dries + resends,
        "{name}: {} collapses and {} strays of {} dry runs",
        c.collapsed,
        c.strays,
        c.dries
    );
    assert!(
        !c.open_at_end,
        "{name}: a window never closed: {:?}",
        c.windows
    );
    assert!(
        c.dries >= opens,
        "{name}: dry runs {} of {opens} opens",
        c.dries
    );
}

async fn replay(name: &str, opens: usize, resends: usize, run: Scenario) {
    let a = account_with_lane(&format!("{name}-replay")).await;
    run(&a, name).await;
    let c = settled(&a).await;
    bounded(name, &c, opens, resends);
    if let Err(e) = compare(name, &c.frames, &recorded(name)) {
        panic!("{e}");
    }
}

macro_rules! replays {
    ($($fn:ident, $name:literal, $run:expr, $opens:expr, $resends:expr;)*) => {$(
        #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
        async fn $fn() {
            let run: Scenario = $run;
            replay($name, $opens, $resends, run).await;
        }
    )*};
}

replays! {
    the_save_replays, "save", save_scenario, 1, 0;
    the_unload_replays, "unload", unload_scenario, 2, 0;
    the_switch_replays, "switch", switch_scenario, 2, 0;
    the_take_replays, "take", take_scenario, 1, 0;
    the_browsers_document_replays, "client", client_scenario, 1, 0;
    the_recycle_replays, "recycle", recycle_scenario, 1, 0;
    the_binding_replays, "binding", binding_scenario, 1, 1;
    the_copy_replays, "copy", copy_scenario, 1, 0;
    the_queries_replay, "queries", queries_scenario, 0, 0;
    the_preview_replays, "preview", preview_scenario, 1, 0;
    the_thaw_replays, "thaw", thaw_scenario, 1, 2;
    the_armed_take_replays, "armedtake", armed_take_scenario, 1, 0;
    the_dropped_take_replays, "droppedtake", dropped_take_scenario, 1, 1;
    the_edit_while_thawed_replays, "jedit", edit_while_thawed_scenario, 1, 2;
}

#[test]
fn a_frame_swapped_with_the_next_fails_the_comparison() {
    let want = recorded("save");
    assert!(want.len() > 3);
    assert!(compare("save", &want, &want).is_ok());
    let mut got = want.clone();
    let at = (0..got.len() - 1).find(|&i| got[i] != got[i + 1]).unwrap();
    got.swap(at, at + 1);
    let e = compare("save", &got, &want).unwrap_err();
    assert!(e.contains(&format!("part at {at}")), "{e}");
    let mut short = want.clone();
    short.pop();
    assert!(compare("save", &short, &want).is_err());
}
