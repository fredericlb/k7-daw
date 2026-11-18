// preferences_file.rs — the preferences file read and written: found as it is, never saved over when it does not read.
// A reader would get wrong that an old `preferences.yaml` is not upgraded here: that is the native server's, so it answers as a problem.

use crate::confine::Env;
use crate::error::{Error, Res, fail, io_error};
use crate::fs::Fs;
use crate::js_text::js_trim;
use crate::paths::{dirname, join};
use crate::preferences::{
    AudioDevicePref, ChannelDir, ChannelLabel, CueOutput, LEGACY_PREFERENCES_FILE, PREFERENCES_FILE, Preferences,
    parse_strict, stringify_preferences,
};
use song::SourceError;

/// The preferences as found, and the reason when a file is there that does not read.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub prefs: Preferences,
    pub problem: Option<String>,
}

fn unreadable(file: &str, why: &str) -> Found {
    Found {
        prefs: Preferences::default(),
        problem: Some(format!(
            "{file} {why}; fix it by hand or move it away, nothing was saved"
        )),
    }
}

enum Read {
    Text(String),
    Absent,
    Error(String),
}

fn read_text(fs: &dyn Fs, path: &str) -> Read {
    match fs.read_text(path) {
        Ok(t) => Read::Text(t),
        Err(SourceError::NotFound | SourceError::Missing) => Read::Absent,
        Err(_) => Read::Error("EIO".to_owned()),
    }
}

/// Reads the file as it is: its text, or the defaults when absent. The upgrade of an old `preferences.yaml` is not in this crate.
#[must_use]
pub fn find(fs: &dyn Fs, env: &Env) -> Found {
    let path = join(&[&env.studio_home, PREFERENCES_FILE]);
    let legacy = join(&[&env.studio_home, LEGACY_PREFERENCES_FILE]);
    let blank = match read_text(fs, &path) {
        Read::Error(e) => return unreadable(&path, &format!("cannot be read ({e})")),
        Read::Text(t) if !t.is_empty() => {
            return parse_strict(&t).map_or_else(
                || unreadable(&path, "does not read"),
                |p| Found {
                    prefs: p,
                    problem: None,
                },
            );
        }
        Read::Text(_) => true,
        Read::Absent => false,
    };
    if !blank && fs.present(&path) && !fs.exists(&path) {
        return unreadable(&path, "is a link to nothing");
    }
    match read_text(fs, &legacy) {
        Read::Error(e) => unreadable(&legacy, &format!("cannot be read ({e})")),
        Read::Absent if blank => unreadable(&path, "is empty"),
        Read::Absent => Found {
            prefs: Preferences::default(),
            problem: None,
        },
        Read::Text(_) => unreadable(
            &legacy,
            "has to be upgraded to preferences.k7x, which this crate does not do",
        ),
    }
}

/// The preferences, or the defaults when no file reads: a project opens either way.
#[must_use]
pub fn load_preferences(fs: &dyn Fs, env: &Env) -> Preferences {
    find(fs, env).prefs
}

/// Why the file cannot be changed, or None when it can.
#[must_use]
pub fn preferences_problem(fs: &dyn Fs, env: &Env) -> Option<String> {
    find(fs, env).problem
}

fn write_target(fs: &dyn Fs, file: &str) -> Res<String> {
    if !fs.present(file) || fs.exists(file) && fs.real_path(file).is_none_or(|r| r == file) {
        return Ok(file.to_owned());
    }
    fs.real_path(file)
        .map_or_else(|| fail(format!("{file} is a link to nothing, nothing was saved")), Ok)
}

/// Writes the preferences whole; through a link when the file is one.
///
/// # Errors
/// A link to nothing, or the file system's.
pub fn save_preferences(fs: &mut dyn Fs, env: &Env, p: &Preferences) -> Res<()> {
    let path = join(&[&env.studio_home, PREFERENCES_FILE]);
    let dir = dirname(&path);
    fs.mkdir_all(&dir).map_err(|e| io_error(&e, "mkdir", &dir))?;
    let target = write_target(fs, &path)?;
    fs.write_atomic(&target, stringify_preferences(p)?.as_bytes())
        .map_err(|e| io_error(&e, "open", &target))
}

/// Load, change, save: never over a file that did not read.
fn update(fs: &mut dyn Fs, env: &Env, change: impl FnOnce(&mut Preferences)) -> Res<Preferences> {
    let mut found = find(fs, env);
    if let Some(problem) = found.problem {
        return Err(Error::Msg(problem));
    }
    change(&mut found.prefs);
    save_preferences(fs, env, &found.prefs)?;
    Ok(found.prefs)
}

/// # Errors
/// A file that did not read, or the write's.
pub fn set_audio_device(fs: &mut dyn Fs, env: &Env, name: &str, kind: Option<&str>) -> Res<Preferences> {
    update(fs, env, |p| {
        p.audio_device = (!name.is_empty()).then(|| AudioDevicePref {
            name: name.to_owned(),
            kind: kind.filter(|t| !t.is_empty()).map(str::to_owned),
        });
    })
}

fn first_of_pair(channel: f64) -> f64 {
    if channel % 2.0 == 1.0 { channel } else { channel - 1.0 }
}

/// # Errors
/// A file that did not read, or the write's.
pub fn set_input_pair(fs: &mut dyn Fs, env: &Env, channel: f64, stereo: bool) -> Res<Preferences> {
    let first = first_of_pair(channel);
    update(fs, env, |p| {
        let mut rest: Vec<f64> = p.input_pairs.iter().copied().filter(|n| *n != first).collect();
        if stereo {
            rest.push(first);
            rest.sort_by(f64::total_cmp);
        }
        p.input_pairs = rest;
    })
}

/// # Errors
/// A file that did not read, or the write's.
pub fn set_cue_output(fs: &mut dyn Fs, env: &Env, spec: Option<CueOutput>) -> Res<Preferences> {
    let want = match spec {
        Some(CueOutput::Label(s)) => CueOutput::Label(js_trim(&s).to_owned()),
        Some(c) => c,
        None => CueOutput::Label(String::new()),
    };
    update(fs, env, |p| {
        p.cue_output = match want {
            CueOutput::Label(s) if s.is_empty() => None,
            CueOutput::Channels(c) if c.is_empty() => None,
            other => Some(other),
        };
    })
}

/// # Errors
/// A file that did not read, or the write's.
pub fn set_stems_chain(fs: &mut dyn Fs, env: &Env, r: &str) -> Res<Preferences> {
    let want = js_trim(r);
    update(fs, env, |p| p.stems_chain = (!want.is_empty()).then(|| want.to_owned()))
}

/// # Errors
/// A file that did not read, or the write's.
pub fn set_instrumental_chain(fs: &mut dyn Fs, env: &Env, r: &str) -> Res<Preferences> {
    let want = js_trim(r);
    update(fs, env, |p| {
        p.instrumental_chain = (!want.is_empty()).then(|| want.to_owned())
    })
}

/// # Errors
/// A file that did not read, or the write's.
pub fn set_editor_vim(fs: &mut dyn Fs, env: &Env, on: bool) -> Res<Preferences> {
    update(fs, env, |p| p.editor_vim = on)
}

fn same_set(a: &[f64], b: &[f64]) -> bool {
    let sorted = |x: &[f64]| {
        let mut v = x.to_vec();
        v.sort_by(f64::total_cmp);
        v
    };
    a.len() == b.len() && sorted(a) == sorted(b)
}

/// # Errors
/// A file that did not read, or the write's.
pub fn set_channel_label(fs: &mut dyn Fs, env: &Env, dir: ChannelDir, ch: &[f64], label: &str) -> Res<Preferences> {
    update(fs, env, |p| {
        let mut list: Vec<ChannelLabel> = p.list(dir).iter().filter(|e| !same_set(&e.ch, ch)).cloned().collect();
        let mut sorted = ch.to_vec();
        sorted.sort_by(f64::total_cmp);
        list.push(ChannelLabel {
            ch: sorted,
            label: label.to_owned(),
        });
        list.sort_by(|a, b| a.ch[0].total_cmp(&b.ch[0]));
        *p.list_mut(dir) = list;
    })
}

/// # Errors
/// A file that did not read, or the write's.
pub fn clear_channel_label(fs: &mut dyn Fs, env: &Env, dir: ChannelDir, ch: &[f64]) -> Res<Preferences> {
    update(fs, env, |p| p.list_mut(dir).retain(|e| !same_set(&e.ch, ch)))
}
