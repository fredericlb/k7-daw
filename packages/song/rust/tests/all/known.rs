// known.rs — what a bundle remembers of the compile beyond the files: the digest of a binary and the staleness of a jsdsp graph.
// A reader would get wrong that a replay reads the bytes: it holds a binary by its length, so the digest and the verdict travel in the bundle.

use song::record::{Extras, Recorder};
use song::{Cache, Entry, Kind, MemSource, Source, SourceBundle, SourceError, Span, Stamp, Stat};
use std::collections::BTreeSet;

/// A Source over one binary file.
struct Binary(Vec<u8>);

const AT: &str = "/p/preset.vstpreset";

impl Source for Binary {
    fn stat(&self, path: &str) -> Result<Stat, SourceError> {
        if path != AT {
            return Err(SourceError::NotFound);
        }
        Ok(Stat {
            kind: Kind::File,
            len: self.0.len() as u64,
            stamp: Stamp {
                mtime_ns: 1,
                len: self.0.len() as u64,
                settled: true,
            },
        })
    }

    fn list(&self, _dir: &str) -> Result<Vec<Entry>, SourceError> {
        Err(SourceError::NotFound)
    }

    fn read(&self, path: &str, _span: Span) -> Result<Vec<u8>, SourceError> {
        if path == AT {
            Ok(self.0.clone())
        } else {
            Err(SourceError::NotFound)
        }
    }
}

fn digest_of(source: &dyn Source) -> Result<String, SourceError> {
    let st = source.stat(AT)?;
    Cache::new().digest(source, AT, &st)
}

// 747: a binary's digest travels in the bundle, so the replay says what the server said without holding a byte of the file.
#[test]
fn a_digest_of_a_binary_travels_in_the_bundle_and_a_replay_answers_it_unread() {
    let bytes: Vec<u8> = (0..=255).collect();
    let inner = Binary(bytes);
    let rec = Recorder::new(&inner);
    let hex = digest_of(&rec).expect("a digest");
    let bundle = rec.into_bundle(&["/p".to_owned()]);
    assert_eq!(bundle.digests.get(AT), Some(&hex), "{:?}", bundle.digests);
    let replay = MemSource::from_bundle(bundle).expect("a source");
    assert_eq!(replay.known_digest(AT).as_deref(), Some(hex.as_str()));
    assert_eq!(digest_of(&replay).as_deref(), Ok(hex.as_str()));
}

// 747: a text the bundle holds needs no digest of its own: the replay hashes the text, which is the bytes.
#[test]
fn a_digest_of_a_text_is_not_kept_in_the_bundle() {
    let inner = Binary(b"plain text".to_vec());
    let rec = Recorder::new(&inner);
    let hex = digest_of(&rec).expect("a digest");
    let bundle = rec.into_bundle(&["/p".to_owned()]);
    assert!(bundle.digests.is_empty(), "{:?}", bundle.digests);
    let replay = MemSource::from_bundle(bundle).expect("a source");
    assert_eq!(digest_of(&replay), Ok(hex));
}

// 747: a binary with no digest in its bundle hashes as the one byte a replay reads, which is at worst a render that reads as not ready.
#[test]
fn a_binary_with_no_digest_hashes_as_one_byte() {
    let bundle = SourceBundle {
        format: 1,
        mounts: vec!["/p".to_owned()],
        files: [(
            AT.to_owned(),
            song::BundleFile {
                len: Some(256),
                binary: Some(true),
                ..Default::default()
            },
        )]
        .into(),
        lists: [(
            "/p".to_owned(),
            vec![Entry {
                name: "preset.vstpreset".to_owned(),
                kind: Kind::File,
            }],
        )]
        .into(),
        ..Default::default()
    };
    let replay = MemSource::from_bundle(bundle).expect("a source");
    assert_eq!(replay.known_digest(AT), None);
    let one = digest_of(&Binary(vec![0xFF])).expect("a digest");
    assert_eq!(digest_of(&replay), Ok(one));
}

fn jsdsp_world(stale: &[&str]) -> MemSource {
    let graph = "{\"params\":[],\"nodes\":[]}\n";
    let mut bundle = SourceBundle {
        format: 1,
        mounts: vec!["/p".to_owned()],
        ..Default::default()
    };
    for dir in ["a", "b"] {
        for (name, text) in [("jsdsp.ts", "export {}\n"), ("jsdsp.json", graph)] {
            bundle.files.insert(
                format!("/p/{dir}/{name}"),
                song::BundleFile {
                    text: Some(text.to_owned()),
                    ..Default::default()
                },
            );
        }
        bundle.lists.insert(
            format!("/p/{dir}"),
            ["jsdsp.json", "jsdsp.ts"]
                .iter()
                .map(|n| Entry {
                    name: (*n).to_owned(),
                    kind: Kind::File,
                })
                .collect(),
        );
    }
    bundle.jsdsp_stale = stale.iter().map(|s| (*s).to_owned()).collect::<BTreeSet<_>>();
    MemSource::from_bundle(bundle).expect("a source")
}

// 748: the verdict of the server is the replay's, since a memory stamp is a hash and orders nothing.
#[test]
fn a_stale_graph_is_stale_in_the_replay_because_the_bundle_says_so() {
    let src = jsdsp_world(&["/p/a/jsdsp.ts"]);
    assert!(!src.stamps_are_times());
    assert_eq!(src.known_stale("/p/a/jsdsp.ts"), Some(true));
    assert_eq!(src.known_stale("/p/b/jsdsp.ts"), Some(false));
}

// 748: a Recorder carries the verdict a compile made, and a disk-less Source says none.
#[test]
fn a_recorder_writes_the_verdict_it_is_given_and_a_plain_source_says_none() {
    assert_eq!(Binary(Vec::new()).known_stale("/p/a/jsdsp.ts"), None);
    let inner = Binary(Vec::new());
    let rec = Recorder::new(&inner);
    let extras = Extras {
        jsdsp_stale: ["/p/a/jsdsp.ts".to_owned()].into(),
        ..Default::default()
    };
    let bundle = rec.into_bundle_with(&["/p".to_owned()], &[], extras);
    assert_eq!(bundle.jsdsp_stale.len(), 1);
}

// A compile watches its source to know what it read; the watcher must pass a bundle's measured lengths on, or a replay asks for the header bytes it never held.
#[test]
fn the_watcher_passes_a_bundles_measured_seconds_on() {
    let mut bundle = SourceBundle {
        format: 1,
        mounts: vec!["/p".to_owned()],
        ..Default::default()
    };
    bundle.lengths.insert(AT.to_owned(), 0.5);
    let replay = MemSource::from_bundle(bundle).expect("a source");
    let watch = song::readset::Watch::new(&replay);
    assert_eq!(watch.known_seconds(AT), Some(0.5));
}

// A replay measures from its table: an audio file it lacks is no wav (a mov, an mp3), and its header is not asked for.
#[test]
fn a_replay_does_not_ask_the_header_of_a_file_its_table_lacks() {
    use song::views::lengths::{ByHeader, Measure};
    let mut bundle = SourceBundle {
        format: 1,
        measured: true,
        mounts: vec!["/p".to_owned()],
        ..Default::default()
    };
    bundle.files.insert(
        "/p/prep.mov".to_owned(),
        song::source::BundleFile {
            len: Some(10),
            binary: Some(true),
            ..Default::default()
        },
    );
    let replay = MemSource::from_bundle(bundle).expect("a source");
    let watch = song::readset::Watch::new(&replay);
    let mut cache = Cache::new();
    let mut by = ByHeader::new(&watch, &mut cache);
    assert_eq!(by.seconds("/p/prep.mov"), Ok(None));
}
