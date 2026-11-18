// prefs.rs — the machine-level arms: the preferences file, the module move, the account fork and the library forks.
// A reader would get wrong that preferences answer the NORMALIZED file: keys sorted and defaults filled, as the harvest records them.

use super::args::{at, b, done, json_of, s, string_list, undefined, val_lax};
use super::{Ctx, Out};
use crate::preferences::{self, ChannelDir, CueOutput};
use crate::{account_fork, js_text, library_forks, module_move, preferences_file};
use k7x::edit::Value;
use serde_json::{Value as Json, json};

const PREFS: &[&str] = &[
    "resolveInput",
    "resolveCueOutput",
    "loadPreferences",
    "preferencesProblem",
    "savePreferences",
    "normalizePreferences",
    "parsePreferences",
    "stringifyPreferences",
    "parseChannels",
    "formatChannels",
    "formatChannel",
    "formatAudioDevice",
    "setAudioDevice",
    "setInputPair",
    "setCueOutput",
    "setStemsChain",
    "setInstrumentalChain",
    "setEditorVim",
    "setChannelLabel",
    "clearChannelLabel",
];

pub const NAMES: &[(&str, &[&str])] = &[
    ("song/preferences", PREFS),
    (
        "song/moduleMove",
        &[
            "planModuleMove",
            "refAt",
            "assertNotEngine",
            "repointDocText",
            "repointRefs",
        ],
    ),
    ("song/accountFork", &["forkIntoAccount"]),
    (
        "song/libraryForks",
        &["libraryForks", "forksWhoseBaseMoved", "libraryModuleDigest"],
    ),
];

fn prefs_of(j: &Json) -> preferences::Preferences {
    preferences::normalize_preferences(&val_lax(j))
}

fn resolved(r: &preferences::Resolved) -> Json {
    match &r.unknown_label {
        Some(l) => json!({ "channels": r.channels, "unknownLabel": l }),
        None => json!({ "channels": r.channels }),
    }
}

/// The preferences as the TypeScript's object: the normalized file read back, keys sorted and defaults filled.
pub fn prefs_json(p: &preferences::Preferences) -> Json {
    let text = preferences::stringify_preferences(p).unwrap_or_default();
    json_of(&k7x::edit::parse(&text).unwrap_or(Value::Null))
}

fn forks_json(l: &[library_forks::LibraryFork]) -> Json {
    Json::Array(
        l.iter()
            .map(|f| json!({ "module": f.module, "base": f.base }))
            .collect(),
    )
}

fn nums(j: &Json) -> Vec<f64> {
    j.as_array()
        .map(|l| l.iter().filter_map(Json::as_f64).collect())
        .unwrap_or_default()
}

fn cue_of(j: &Json) -> Option<CueOutput> {
    match j {
        Json::Array(_) => Some(CueOutput::Channels(nums(j))),
        Json::String(t) => Some(CueOutput::Label(t.clone())),
        _ => None,
    }
}

fn library(j: &Json) -> module_move::Library<'_> {
    module_move::Library {
        name: j["name"].as_str().unwrap_or(""),
        root: j["root"].as_str().unwrap_or(""),
    }
}

fn prefs_arm(cx: &mut Ctx, f: &str, a: &[Json]) -> Option<Out> {
    let env = cx.env;
    let fs = &mut *cx.fs;
    Some(match f {
        "resolveInput" => {
            let spec = match at(a, 0) {
                Json::Array(l) => Some(CueOutput::Channels(
                    l.iter().map(|x| js_text::js_to_number(Some(&val_lax(x)))).collect(),
                )),
                Json::String(t) => Some(CueOutput::Label(t.clone())),
                _ => None,
            };
            Ok(resolved(&preferences::resolve_input(
                spec.as_ref(),
                &prefs_of(at(a, 1)),
            )))
        }
        "resolveCueOutput" => Ok(resolved(&preferences::resolve_cue_output(&prefs_of(at(a, 0))))),
        "loadPreferences" => Ok(prefs_json(&preferences_file::load_preferences(fs, env))),
        "preferencesProblem" => Ok(preferences_file::preferences_problem(fs, env).map_or(Json::Null, |p| json!(p))),
        "savePreferences" => done(preferences_file::save_preferences(fs, env, &prefs_of(at(a, 0))), |()| {
            undefined()
        }),
        "normalizePreferences" => Ok(prefs_json(&prefs_of(at(a, 0)))),
        "parsePreferences" => Ok(prefs_json(&preferences::parse_preferences(&s(a, 0)))),
        "stringifyPreferences" => done(preferences::stringify_preferences(&prefs_of(at(a, 0))), |t| json!(t)),
        "parseChannels" => Ok(json!(preferences::parse_channels(&s(a, 0)))),
        "formatChannels" => {
            let (i, o) = preferences::format_channels(&prefs_of(at(a, 0)));
            Ok(json!({ "in": i, "out": o }))
        }
        "formatChannel" => {
            let p = prefs_of(&json!({ "channels": { "in": [at(a, 0).clone()] } }));
            Ok(json!(
                p.channels_in
                    .first()
                    .map_or_else(String::new, preferences::format_channel)
            ))
        }
        "formatAudioDevice" => {
            let p = prefs_of(&json!({ "audioDevice": at(a, 0).clone() }));
            Ok(json!(preferences::format_audio_device(p.audio_device.as_ref())))
        }
        _ => {
            let dir = if s(a, 0) == "in" {
                ChannelDir::In
            } else {
                ChannelDir::Out
            };
            let r = match f {
                "setAudioDevice" => preferences_file::set_audio_device(fs, env, &s(a, 0), at(a, 1).as_str()),
                "setInputPair" => preferences_file::set_input_pair(fs, env, at(a, 0).as_f64().unwrap_or(0.0), b(a, 1)),
                "setCueOutput" => preferences_file::set_cue_output(fs, env, cue_of(at(a, 0))),
                "setStemsChain" => preferences_file::set_stems_chain(fs, env, &s(a, 0)),
                "setInstrumentalChain" => preferences_file::set_instrumental_chain(fs, env, &s(a, 0)),
                "setEditorVim" => preferences_file::set_editor_vim(fs, env, b(a, 0)),
                "setChannelLabel" => preferences_file::set_channel_label(fs, env, dir, &nums(at(a, 1)), &s(a, 2)),
                "clearChannelLabel" => preferences_file::clear_channel_label(fs, env, dir, &nums(at(a, 1))),
                _ => return None,
            };
            done(r, |p| prefs_json(&p))
        }
    })
}

pub fn run(cx: &mut Ctx, module: &str, f: &str, a: &[Json]) -> Option<Out> {
    let fs = &mut *cx.fs;
    let env = cx.env;
    Some(match (module, f) {
        ("song/preferences", _) => return prefs_arm(cx, f, a),
        ("song/moduleMove", "planModuleMove") => done(
            module_move::plan_module_move(fs, library(at(a, 0)), &s(a, 1), &s(a, 2)),
            |m| json!({ "from": m.from, "to": m.to, "dest": m.dest }),
        ),
        ("song/moduleMove", "refAt") => {
            Ok(module_move::ref_at(library(at(a, 0)), &s(a, 1)).map_or(Json::Null, |r| json!(r)))
        }
        ("song/moduleMove", "assertNotEngine") => done(
            module_move::assert_not_engine(at(a, 0)["root"].as_str().unwrap_or(""), &s(a, 1)),
            |()| undefined(),
        ),
        ("song/moduleMove", "repointDocText") => {
            let mut doc = k7x::edit::Document::new(&s(a, 0));
            done(
                module_move::repoint_doc_refs(&mut doc, &s(a, 1), &s(a, 2), b(a, 3)),
                |c| json!({ "changed": c, "text": doc.text() }),
            )
        }
        ("song/moduleMove", "repointRefs") => done(
            module_move::repoint_refs(fs, &string_list(at(a, 0)), &s(a, 1), &s(a, 2), b(a, 3)),
            |mut l| {
                l.sort();
                json!(l)
            },
        ),
        ("song/accountFork", "forkIntoAccount") => done(
            account_fork::fork_into_account(fs, env, &s(a, 0), &s(a, 1)),
            |f| json!({ "ref": f.reference, "folder": f.folder, "created": f.created, "repointed": f.repointed }),
        ),
        ("song/libraryForks", "libraryForks") => {
            Ok(forks_json(&library_forks::library_forks(fs, env, &s(a, 0), &s(a, 1))))
        }
        ("song/libraryForks", "forksWhoseBaseMoved") => {
            let v = |i: usize| at(a, i).as_u64().map_or_else(|| s(a, i), |n| n.to_string());
            done(
                library_forks::forks_whose_base_moved(fs, env, &s(a, 0), &s(a, 1), &v(2), &v(3)),
                |l| forks_json(&l),
            )
        }
        ("song/libraryForks", "libraryModuleDigest") => {
            let v = at(a, 1).as_u64().map_or_else(|| s(a, 1), |n| n.to_string());
            Ok(library_forks::library_module_digest(fs, env, &s(a, 0), &v, &s(a, 2)).map_or(Json::Null, |d| json!(d)))
        }
        _ => return None,
    })
}
