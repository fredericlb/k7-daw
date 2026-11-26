//! slots.rs — the slot store in a folder of its own, as `Slots_test` held it: saved, listed without its
//! document, enabled and remembered, forgotten with its mark, nothing written for what is no slot; the files a
//! slot plays carried into it, or the slot refused for what the machine lacks; its footswitch, kept across a save.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use k7_supervisor::slots::{self, Installed};
use serde_json::{Value, json};

use crate::scratch::Scratch;

fn stage(dir: &Path, slot: &Value) -> PathBuf {
    let p = dir.join("staged.json");
    std::fs::write(&p, slot.to_string()).unwrap();
    p
}

fn a_slot(id: &str) -> Value {
    json!({"id": id, "name": "Warm", "chain": "@account/warm", "savedAt": "2026-11-01T18:00:00Z",
           "protocol": 7, "doc": {"name": "warm", "tracks": []}})
}

fn save(dir: &Path, slot: &Value) -> Result<String, String> {
    slots::save(dir, &stage(dir, slot), &Installed::default())
}

fn write(p: &Path, text: &str) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

#[test]
fn a_saved_slot_is_listed_without_its_document() {
    let t = Scratch::new("slots");
    let dir = t.path();
    assert_eq!(save(dir, &a_slot("warm")), Ok("warm".to_string()));
    let all = slots::list(dir);
    assert_eq!(
        (all.len(), all[0].chain.as_str(), all[0].protocol),
        (1, "@account/warm", 7)
    );
    assert_eq!(slots::document(dir, "warm").unwrap()["name"], "warm");
    // The staged file is consumed: a second save of it finds nothing.
    assert!(!dir.join("staged.json").exists());
}

#[test]
fn the_enabled_slot_is_remembered_and_goes_with_it() {
    let t = Scratch::new("slots");
    let dir = t.path();
    save(dir, &a_slot("warm")).unwrap();
    assert_eq!(slots::enabled(dir), None);
    slots::set_enabled(dir, Some("warm"));
    assert_eq!(slots::enabled(dir).as_deref(), Some("warm"));
    assert!(slots::remove(dir, "warm"));
    assert_eq!(slots::enabled(dir), None);
    assert!(slots::list(dir).is_empty());
    assert!(!slots::remove(dir, "warm"));
}

#[test]
fn a_mark_naming_no_slot_is_no_mark() {
    let t = Scratch::new("slots");
    slots::set_enabled(t.path(), Some("gone"));
    assert_eq!(slots::enabled(t.path()), None);
}

#[test]
fn what_is_no_slot_is_refused_and_nothing_written() {
    let t = Scratch::new("slots");
    let dir = t.path();
    assert!(save(dir, &a_slot("../escape")).is_err());
    let mut no_doc = a_slot("warm");
    no_doc.as_object_mut().unwrap().remove("doc");
    assert!(save(dir, &no_doc).is_err());
    assert!(slots::list(dir).is_empty());
    assert!(
        !slots::valid_id("Warm") && slots::valid_id("warm-2") && !slots::valid_id(&"a".repeat(65))
    );
}

#[test]
fn saving_again_replaces_the_slot() {
    let t = Scratch::new("slots");
    let dir = t.path();
    save(dir, &a_slot("warm")).unwrap();
    let mut again = a_slot("warm");
    again["savedAt"] = json!("2026-11-02T09:00:00Z");
    save(dir, &again).unwrap();
    let all = slots::list(dir);
    assert_eq!(
        (all.len(), all[0].saved_at.as_str()),
        (1, "2026-11-02T09:00:00Z")
    );
}

/// A rig whose amp names a model and an IR, whose sampler plays an SFZ with a sample beside it.
fn a_rig_naming(src: &Path) -> Value {
    write(&src.join("amp.nam"), "model");
    write(&src.join("cab.wav"), "ir");
    write(&src.join("kit/kit.sfz"), "<region> sample=snare.wav");
    write(&src.join("kit/snare.wav"), "snare");
    let mut slot = a_slot("rig");
    slot["doc"] = json!({"name": "rig", "recordDir": "/somewhere/else", "masterPlugins": [], "tracks": [{"plugins": [
        {"type": "nam", "name": "amp", "presetSave": "/somewhere/amp.vstpreset",
         "namFile": src.join("amp.nam").to_string_lossy(), "irFile": src.join("cab.wav").to_string_lossy()},
        {"type": "sfz", "name": "kit", "sfzFile": src.join("kit/kit.sfz").to_string_lossy()}]}]});
    slot
}

#[test]
fn a_slot_carries_the_files_it_plays() {
    let t = Scratch::new("slots");
    let dir = t.path();
    let src = dir.join("source");
    assert_eq!(save(dir, &a_rig_naming(&src)), Ok("rig".to_string()));
    std::fs::remove_dir_all(&src).unwrap();
    let doc = slots::document(dir, "rig").unwrap();
    let (amp, kit) = (
        &doc["tracks"][0]["plugins"][0],
        &doc["tracks"][0]["plugins"][1],
    );
    // Every path now under the slot's own folder, and what it names there, the source gone.
    let under = dir.join("rig/files");
    let nam = PathBuf::from(amp["namFile"].as_str().unwrap());
    assert!(nam.starts_with(&under) && nam.exists());
    assert!(Path::new(amp["irFile"].as_str().unwrap()).exists());
    let sfz = PathBuf::from(kit["sfzFile"].as_str().unwrap());
    assert!(sfz.exists() && sfz.parent().unwrap().join("snare.wav").exists());
    // Nothing written back, and takes in the slot.
    assert_eq!(amp["presetSave"], "");
    assert_eq!(
        doc["recordDir"],
        dir.join("rig/takes").to_string_lossy().as_ref()
    );
    assert!(!dir.join("rig.new").exists());
}

#[test]
fn what_the_machine_lacks_refuses_the_slot_and_keeps_the_old_one() {
    let t = Scratch::new("slots");
    let dir = t.path();
    save(dir, &a_slot("rig")).unwrap();
    let mut slot = a_slot("rig");
    slot["doc"]["tracks"] = json!([{"plugins": [
        {"type": "VST3", "name": "Diva"},
        {"type": "sfz", "name": "wurli", "sfzFile": "@instruments/nbo-wurt/wurli.sfz"}]}]);
    let has = Installed {
        plugin: Some(Arc::new(|f, n| f == "vst3" && n == "Pro-Q")),
        instrument: Some(Arc::new(|_| false)),
        readable: None,
    };
    let why = slots::save(dir, &stage(dir, &slot), &has).unwrap_err();
    assert!(why.contains("Diva") && why.contains("nbo-wurt"), "{why}");
    // The slot it would have replaced is still the one there.
    assert_eq!(slots::document(dir, "rig").unwrap()["name"], "warm");
    assert!(!dir.join("rig.new").exists());
}

#[test]
fn a_file_outside_the_servers_copies_is_refused_when_only_those_may_be_copied() {
    let t = Scratch::new("slots");
    let dir = t.path();
    let rig = a_rig_naming(&dir.join("source"));
    let only = dir.join("source/kit");
    let has = Installed {
        readable: Some(Arc::new(move |p| p.starts_with(&only))),
        ..Installed::default()
    };
    let why = slots::save(dir, &stage(dir, &rig), &has).unwrap_err();
    assert!(why.contains("outside this machine's copies"), "{why}");
    assert!(slots::list(dir).is_empty());
}

#[test]
fn the_footswitch_is_the_machines_and_rides_with_the_document() {
    let t = Scratch::new("slots");
    let dir = t.path();
    let mut slot = a_slot("warm");
    slot["chainPlugins"] = json!(["IN:amp"]);
    save(dir, &slot).unwrap();
    assert!(
        slots::document(dir, "warm")
            .unwrap()
            .get("footswitch")
            .is_none()
    );
    assert!(!slots::set_footswitch(dir, "warm", Some(128)));
    assert!(!slots::set_footswitch(dir, "gone", Some(64)));
    assert!(slots::set_footswitch(dir, "warm", Some(64)));
    assert_eq!(slots::list(dir)[0].footswitch, Some(64));
    let doc = slots::document(dir, "warm").unwrap();
    assert_eq!(doc["footswitch"], json!({"cc": 64, "plugins": ["IN:amp"]}));
    // Saved again from the chain, it keeps its pedal; let go, it has none.
    save(dir, &slot).unwrap();
    assert_eq!(slots::list(dir)[0].footswitch, Some(64));
    assert!(slots::set_footswitch(dir, "warm", None));
    assert_eq!(slots::list(dir)[0].footswitch, None);
}
