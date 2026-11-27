// writes.rs — which command lines write the workspace: the class a merge's gate asks of a line.
// A reader would get wrong that a verb is listed as a write: only the reads and the device's are, and any other verb of the catalogue writes.

use crate::palette::catalogue;
use serde::Serialize;

/// What a verb does to the workspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum VerbClass {
    /// It changes the workspace's files, or feeds the take that will.
    Write,
    /// It only reads, or plays.
    Read,
    /// It writes this machine's settings, which a locked workspace still runs.
    Device,
}

/// The verbs that read, or play, and change no file of the workspace.
const READS: [&str; 14] = [
    "timeline play",
    "project reload",
    "sequence list",
    "library versions",
    "system plugins list",
    "system channel list",
    "node external-ui",
    "transport play",
    "transport stop",
    "transport toggle",
    "transport rewind",
    "transport panic",
    // They hand a film or a cut to the person's own folder and touch no file of the workspace.
    "picture interchange",
    "picture render",
];

/// The verbs that write this machine's preferences, not the workspace.
const DEVICE: [&str; 8] = [
    "system channel set",
    "system channel clear",
    "system channel type",
    "system stems",
    "system instrumental",
    "system onnx",
    "system cue",
    "system plugins scan",
];

/// Verbs that read when they carry at most this many arguments: a bare `project console` lists them.
const READS_UP_TO: [(&str, usize); 2] = [("project console", 0), ("node engine", 1)];

/// How the longest verb of the catalogue that begins `line` is classed, and how many arguments follow it.
fn verb_of(line: &str) -> Option<(String, usize)> {
    let words: Vec<&str> = line
        .split(song_edit::js_text::is_js_space)
        .filter(|w| !w.is_empty())
        .collect();
    let known = catalogue();
    (1..=words.len().min(4)).rev().find_map(|n| {
        let verb = words[..n].join(" ");
        known
            .iter()
            .any(|c| c.command == verb)
            .then_some((verb, words.len() - n))
    })
}

/// What a command line does to the workspace; a line that names no known verb is a write: it fails closed.
#[must_use]
pub fn class_of_line(line: &str) -> VerbClass {
    let Some((verb, args)) = verb_of(line) else {
        return VerbClass::Write;
    };
    if READS_UP_TO.iter().any(|(v, up_to)| *v == verb && args <= *up_to) {
        return VerbClass::Read;
    }
    if READS.contains(&verb.as_str()) {
        VerbClass::Read
    } else if DEVICE.contains(&verb.as_str()) {
        VerbClass::Device
    } else {
        VerbClass::Write
    }
}

/// Every verb of the catalogue, which the gate classes: what is neither a read nor the device's is a write.
#[must_use]
pub fn classed_verbs() -> Vec<String> {
    catalogue().iter().map(|c| c.command.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_named_read_and_device_verb_is_in_the_catalogue() {
        let known = classed_verbs();
        for v in READS
            .iter()
            .chain(DEVICE.iter())
            .chain(READS_UP_TO.iter().map(|(v, _)| v))
        {
            assert!(known.iter().any(|k| k == v), "{v} is named here and is not a verb");
        }
        assert!(known.len() > 150);
    }

    #[test]
    fn a_locked_workspace_keeps_every_read_and_its_own_devices_settings() {
        for line in [
            "sequence list",
            "library versions",
            "system channel list",
            "system plugins list vst",
            "project reload",
            "timeline play verse",
            "transport stop",
            "project console",
            "node engine vox-comp",
            "system channel type 3 stereo",
            "system cue 3+4",
            "system plugins scan",
        ] {
            assert_ne!(class_of_line(line), VerbClass::Write, "{line}");
        }
    }

    #[test]
    fn what_writes_a_longer_verb_over_its_shorter_one_and_the_unknown_are_writes() {
        for line in [
            "project console @k7-core/console/console-6",
            "node engine vox-comp vari-mu",
            "library upgrade k7",
            "transport metronome",
            "transport arm gtr",
            "track duplicate empty a",
            "no such verb",
            "",
        ] {
            assert_eq!(class_of_line(line), VerbClass::Write, "{line}");
        }
    }

    #[test]
    fn the_two_classes_are_told_apart() {
        assert_eq!(class_of_line("transport play"), VerbClass::Read);
        assert_eq!(class_of_line("system cue 3+4"), VerbClass::Device);
        assert_eq!(class_of_line("track new drums"), VerbClass::Write);
    }
}
