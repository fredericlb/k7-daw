// clips_paths.rs — where an audio file is: the source under the project, one stat each, and a path the Source refuses is "".
// A reader would get wrong that the file is never opened: the Source is the confinement, and a missing file is 2e's to name.

use crate::clips_support;
use crate::support;

use serde_json::{Value, json};

fn clip_files(sources: &[(&str, &str)]) -> song::FamilyRead<song::Clip> {
    let items: Vec<(&str, Value)> = sources
        .iter()
        .map(|(id, src)| (*id, json!({ "source": src })))
        .collect();
    clips_support::clip_family(&items)
}

#[test]
fn an_audio_path_is_the_source_under_the_project_and_one_stat_each() {
    // decision 154: Ok and NotFound keep the path, a path outside every mount answers "", and nothing is ever read
    let src = support::mem(&[
        ("clips/kick.wav", "RIFF"),
        ("clips/kick.k7x", "source(\"clips/kick.wav\")\n"),
    ]);
    let spy = support::Spy {
        inner: &src,
        reads: std::sync::Mutex::new(Vec::new()),
    };
    let clips = clip_files(&[
        ("kick", "clips/kick.wav"),
        ("gone", "clips/gone.wav"),
        ("up", "../../x.wav"),
        ("abs", "/etc/passwd"),
        ("nul", "clips/a\0b.wav"),
        ("dots", "clips/../clips/kick.wav"),
    ]);
    let got = song::clips::audio_paths(&spy, "/p", &clips, song::PathRule::HOST).expect("paths");
    let paths: Vec<(&str, &str)> = got.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    assert_eq!(
        paths,
        [
            ("kick", "/p/clips/kick.wav"),
            ("gone", "/p/clips/gone.wav"),
            ("up", ""),
            ("abs", ""),
            ("nul", ""),
            ("dots", "/p/clips/kick.wav")
        ]
    );
    assert!(
        spy.reads.lock().expect("lock").is_empty(),
        "an audio file is never read"
    );
}

#[test]
fn a_path_the_bundle_cannot_answer_for_is_missing_not_a_wrong_answer() {
    // decision 154: `Missing` is an Err, so the compile answers needs_files
    let bare = song::SourceBundle {
        format: 1,
        mounts: vec!["/p".into()],
        files: std::collections::BTreeMap::new(),
        lists: std::collections::BTreeMap::new(),
        ..Default::default()
    };
    let src = song::MemSource::from_bundle(bare).expect("a source");
    let clips = clip_files(&[("kick", "clips/kick.wav")]);
    assert_eq!(
        song::clips::audio_paths(&src, "/p", &clips, song::PathRule::HOST),
        Err(song::SourceError::Missing)
    );
}

#[test]
fn a_backslash_is_a_character_under_the_posix_rule_and_outside_under_the_windows_one() {
    // decision 154's portability row: `a\\b.wav` names one file under the Posix rule; the Windows one reads a separator and refuses it
    let src = support::mem(&[("clips/x.k7x", "source(\"x.wav\")\n")]);
    let clips = clip_files(&[("back", "clips\\..\\..\\x.wav")]);
    for (rule, want) in [
        (song::PathRule::Windows, ""),
        (song::PathRule::Posix, "/p/clips\\..\\..\\x.wav"),
    ] {
        let got = song::clips::audio_paths(&src, "/p", &clips, rule).expect("paths");
        assert_eq!(got["back"], want, "{rule:?}");
    }
}

struct Counting<'a> {
    inner: &'a dyn song::Source,
    lists: std::sync::atomic::AtomicUsize,
}

impl song::Source for Counting<'_> {
    fn stat(&self, path: &str) -> Result<song::Stat, song::SourceError> {
        self.inner.stat(path)
    }

    fn list(&self, dir: &str) -> Result<Vec<song::Entry>, song::SourceError> {
        self.lists.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.inner.list(dir)
    }

    fn read(&self, path: &str, span: song::Span) -> Result<Vec<u8>, song::SourceError> {
        self.inner.read(path, span)
    }
}

#[test]
fn absent_audio_files_through_a_recorder_list_their_folder_once() {
    // decision 154: a Recorder proves each absent file by the listing above it, and a folder is listed once
    let src = support::mem(&[("clips/x.k7x", "source(\"x.wav\")\n")]);
    let counting = Counting {
        inner: &src,
        lists: std::sync::atomic::AtomicUsize::new(0),
    };
    let rec = song::record::Recorder::new(&counting);
    let names: Vec<String> = (0..2000).map(|i| format!("clips/gone{i}.wav")).collect();
    let items: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), n.as_str())).collect();
    let got = song::clips::audio_paths(&rec, "/p", &clip_files(&items), song::PathRule::HOST).expect("paths");
    assert_eq!(got.len(), 2000);
    assert!(counting.lists.load(std::sync::atomic::Ordering::Relaxed) <= 2);
    let bundle = rec.into_bundle(&["/p".to_owned()]);
    assert!(bundle.lists.contains_key("/p/clips"));
}
