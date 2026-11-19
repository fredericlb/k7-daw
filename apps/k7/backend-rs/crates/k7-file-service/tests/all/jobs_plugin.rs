//! jobs_plugin.rs — `resolve_plugin` against the TypeScript's answers over a scan of 43 plugins, read through `Scan::read` (2340, fix 1 and 2).
//! A reader gets wrong that the scan is a FILE the engine wrote: entries with no name or no format are dropped by the reader, not by the resolver.

use crate::support::jobs::Tmp;
use k7_file_service::jobs::plugin::resolve_plugin;
use serde_json::{Value, json};
use song::DiskSource;
use song::modules::Scan;

fn scan_of(plugins: &[Value]) -> Scan {
    let tmp = Tmp::new("jobsplugin");
    tmp.write(
        "exts.json",
        &json!({ "entries": [{ "plugins": plugins }] }).to_string(),
    );
    let source = DiskSource::new(&[tmp.at("")]).unwrap();
    Scan::read(&source, Some(&tmp.at("exts.json"))).unwrap()
}


#[test]
fn a_ref_wins_over_a_format_and_an_id_and_a_hit_without_an_id_is_none() {
    let scan = scan_of(&[
        json!({ "name": "Café Delay", "format": "AU", "id": "au-1" }),
        json!({ "name": "Idless", "format": "VST3" }),
    ]);
    assert_eq!(
        resolve_plugin(&scan, "@engine/caf-delay", "VST3", "other"),
        Ok(("AU".into(), "au-1".into()))
    );
    assert_eq!(
        resolve_plugin(&scan, "@engine/idless", "VST3", "x"),
        Err((404, "no scanned plugin for \"@engine/idless\"".into()))
    );
    assert_eq!(
        resolve_plugin(&scan, "", "VST3", "x"),
        Ok(("VST3".into(), "x".into()))
    );
    assert_eq!(resolve_plugin(&scan, "", "", "x").unwrap_err().0, 400);
}
