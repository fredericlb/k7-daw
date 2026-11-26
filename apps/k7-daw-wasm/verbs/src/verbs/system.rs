// system.rs — the `system …` verbs: the machine's plugin cache, its channel labels and pairs, the CUE pair, the separating chains; after verbs/system.ts.
// A reader would get wrong where they write: the preferences under the Env's studioHome, never the project, so they run with no project open.

use super::chain_resolve::resolve_chain;
use super::routes::{ONNX, PLUGINS, RESCAN, STEREO, call_or_throw, thrown};
use super::shared::{Done, split_words};
use crate::host::Host;
use k7x::edit::js_number;
use serde_json::{Value as Json, json};
use song_edit::js_text::{js_trim, str_to_number};
use song_edit::preferences::{ChannelDir, CueOutput, format_audio_device, format_channels, parse_channels};
use song_edit::preferences_file::{
    clear_channel_label, load_preferences, set_channel_label, set_cue_output, set_input_pair, set_instrumental_chain,
    set_stems_chain,
};

fn plus(ch: &[f64]) -> String {
    ch.iter().map(|n| js_number(*n)).collect::<Vec<_>>().join("+")
}

fn is_empty_quotes(s: &str) -> bool {
    s == "\"\"" || s == "''"
}

/// The leading `in|out` word, case-insensitive, and what follows its run of spaces.
fn direction(a: &str) -> Option<(ChannelDir, &str)> {
    let t = js_trim(a);
    let (word, rest) = t.split_once(char::is_whitespace)?;
    let dir = match word.to_lowercase().as_str() {
        "in" => ChannelDir::In,
        "out" => ChannelDir::Out,
        _ => return None,
    };
    Some((dir, rest.trim_start()))
}

fn dir_name(d: ChannelDir) -> &'static str {
    match d {
        ChannelDir::In => "in",
        ChannelDir::Out => "out",
    }
}

/// One line a plugin is listed by, as `engine plugins` words it.
fn plugin_line(p: &Json) -> String {
    let field = |k: &str| p.get(k).and_then(Json::as_str).unwrap_or("");
    let mark = match field("role") {
        "instrument" => "[inst]",
        "midiEffect" => "[midi]",
        _ => "     ",
    };
    format!(
        "  {mark} {}  [{}]  {}",
        field("name"),
        field("format"),
        field("manufacturer")
    )
}

fn plugins_of(answer: &Json) -> &[Json] {
    answer
        .get("plugins")
        .and_then(Json::as_array)
        .map_or(&[], Vec::as_slice)
}

/// `system plugins list [format]` — the machine's cache, as the engine route answers it.
pub fn plugins_list(a: &str, h: &mut Host) -> Done {
    let filter = js_trim(a);
    let args = if filter.is_empty() {
        json!({})
    } else {
        json!({ "filter": filter })
    };
    let answer = call_or_throw(h, PLUGINS, args)?;
    let plugins = plugins_of(&answer);
    let lines: Vec<String> = if plugins.is_empty() {
        vec!["no plugins cached — run: k7 engine scan".to_owned()]
    } else {
        plugins.iter().map(plugin_line).collect()
    };
    h.log(lines);
    Ok(())
}

/// `system plugins scan` — a rescan the engine route runs, then counted.
pub fn plugins_scan(_a: &str, h: &mut Host) -> Done {
    h.log(["rescanning plugins… (first scan can take a while)".to_owned()]);
    let answer = call_or_throw(h, RESCAN, json!({}))?;
    let n = plugins_of(&answer).len();
    h.log([format!("scanned {n} plugin(s)")]);
    Ok(())
}

/// `system channel list` — the device and the labels of each direction.
pub fn channel_list(_a: &str, h: &mut Host) -> Done {
    let p = load_preferences(&*h.fs, &h.env);
    let (fin, fout) = format_channels(&p);
    let or = |s: String, dir: &str| {
        if s.is_empty() {
            format!("(none — set with: channel {dir} <ch> <label>)")
        } else {
            s
        }
    };
    h.log([
        format!(
            "audio device: {}  (change: médiathèque → État du moteur)",
            format_audio_device(p.audio_device.as_ref())
        ),
        format!("in:  {}", or(fin, "in")),
        format!("out: {}", or(fout, "out")),
    ]);
    Ok(())
}

/// `system channel set <in|out> <chans> <label>`.
pub fn channel_set(a: &str, h: &mut Host) -> Done {
    let parsed = direction(a).and_then(|(d, rest)| {
        let (chans, label) = rest.split_once(char::is_whitespace)?;
        let label = js_trim(label);
        (!label.is_empty()).then_some((d, chans, label))
    });
    let Some((dir, chans, label)) = parsed else {
        h.refuse([
            "usage: system channel set <in|out> <chans> <label>   e.g. system channel set in 3+4 synth".to_owned(),
        ]);
        return Ok(());
    };
    let ch = parse_channels(chans);
    if ch.is_empty() {
        h.refuse([format!("bad channels '{chans}' — use e.g. 1 or 3+4")]);
        return Ok(());
    }
    set_channel_label(h.fs, &h.env, dir, &ch, label)?;
    h.log([format!("{} {} = {label}", dir_name(dir), plus(&ch))]);
    Ok(())
}

/// `system channel clear <in|out> <chans>`.
pub fn channel_clear(a: &str, h: &mut Host) -> Done {
    let parsed = direction(a).and_then(|(d, rest)| {
        let chans = rest.split(char::is_whitespace).next().filter(|c| !c.is_empty())?;
        Some((d, chans))
    });
    let Some((dir, chans)) = parsed else {
        h.refuse(["usage: system channel clear <in|out> <chans>".to_owned()]);
        return Ok(());
    };
    let ch = parse_channels(chans);
    if ch.is_empty() {
        h.refuse([format!("bad channels '{chans}'")]);
        return Ok(());
    }
    clear_channel_label(h.fs, &h.env, dir, &ch)?;
    h.log([format!("cleared {} {}", dir_name(dir), plus(&ch))]);
    Ok(())
}

/// `system channel type <channel> <mono|stereo>` — the preference first, then the engine is asked.
pub fn channel_type(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    let mode = w.get(1).map(|m| m.to_lowercase()).unwrap_or_default();
    if w.is_empty() || (mode != "mono" && mode != "stereo") {
        h.refuse(["usage: system channel type <channel> <mono|stereo>   e.g. system channel type 3 stereo".to_owned()]);
        return Ok(());
    }
    let channel = str_to_number(w[0]);
    if !(channel.is_finite() && channel.fract() == 0.0 && channel >= 1.0) {
        h.refuse([format!(
            "'{}' is not a channel — they count from 1, and either half names the pair",
            w[0]
        )]);
        return Ok(());
    }
    let stereo = mode == "stereo";
    let first = if channel % 2.0 == 1.0 { channel } else { channel - 1.0 };
    set_input_pair(h.fs, &h.env, channel, stereo)?;
    let answer = call_or_throw(h, STEREO, json!({ "channel": channel as i64, "stereo": stereo }))?;
    let heard = answer.get("heard").and_then(Json::as_bool) == Some(true);
    // No engine still did the durable half, which is not the same as doing nothing.
    let tail = if heard {
        ""
    } else {
        " — remembered; the engine hears it on its next connect"
    };
    h.log([format!(
        "in {}+{} is {}{tail}",
        js_number(first),
        js_number(first + 1.0),
        if stereo {
            "one stereo device"
        } else {
            "two mono devices"
        }
    )]);
    Ok(())
}

/// The two chain verbs: which chain separates (`stems`) or makes the instrumental (`instrumental`).
fn chain_verb(a: &str, h: &mut Host, stems: bool) -> Done {
    let r = js_trim(a);
    let prefs = load_preferences(&*h.fs, &h.env);
    let (verb, current, example, what) = if stems {
        (
            "stems",
            prefs.stems_chain,
            "@k7-core/stems",
            "nothing is set, so nothing on this machine will separate a record",
        )
    } else {
        (
            "instrumental",
            prefs.instrumental_chain,
            "@k7-core/instrumental",
            "nothing is set, so nothing on this machine will make an instrumental",
        )
    };
    if r.is_empty() {
        h.refuse([
            format!("usage: system {verb} <chaîne>   e.g. system {verb} {example}"),
            current.map_or_else(
                || what.to_owned(),
                |c| format!("currently: {c} — pass \"\" to clear it"),
            ),
        ]);
        return Ok(());
    }
    let clearing = is_empty_quotes(r);
    let checked = if clearing {
        String::new()
    } else if h.song_path.is_empty() {
        // Not a failure: `system` verbs work with no project.
        " — not checked: no project is open to resolve it against".to_owned()
    } else {
        let p = h.project()?;
        let chain = match resolve_chain(h, &p, r)? {
            Ok(c) => c,
            Err(e) => {
                h.refuse([format!("system {verb}: {e}")]);
                return Ok(());
            }
        };
        if stems {
            if chain.cold.is_empty() {
                h.refuse([format!(
                    "'{r}' does not separate — this wants a chain whose step is an offline module"
                )]);
                return Ok(());
            }
            let steps: Vec<&str> = chain.cold.iter().map(|c| c.step.as_str()).collect();
            format!(" — separates with {}", steps.join(", "))
        } else {
            // Stricter than `system stems`: four stems with one renamed is the mistake this prevents.
            let Some(step) = chain
                .cold
                .iter()
                .find(|c| c.outputs.iter().any(|o| o == "instrumental"))
            else {
                h.refuse([format!(
                    "'{r}' writes no instrumental — this wants a chain whose offline step declares an `instrumental` output"
                )]);
                return Ok(());
            };
            format!(" — written by {}", step.step)
        }
    };
    let want = if clearing { "" } else { r };
    if stems {
        set_stems_chain(h.fs, &h.env, want)?;
    } else {
        set_instrumental_chain(h.fs, &h.env, want)?;
    }
    h.log([match (clearing, stems) {
        (true, true) => "the separating chain is cleared".to_owned(),
        (true, false) => "the instrumental chain is cleared".to_owned(),
        (false, true) => format!("stems are separated with {r}{checked}"),
        (false, false) => format!("the instrumental comes from {r}{checked}"),
    }]);
    Ok(())
}

pub fn stems(a: &str, h: &mut Host) -> Done {
    chain_verb(a, h, true)
}

pub fn instrumental(a: &str, h: &mut Host) -> Done {
    chain_verb(a, h, false)
}

/// `onnxProviders`: the machine's list once `set` is sent, or None when no engine or machine holds the setting.
fn onnx_providers(h: &mut Host, set: Option<&[String]>) -> Result<Option<Vec<String>>, crate::host::Thrown> {
    let args = set.map_or_else(|| json!({}), |s| json!({ "set": s }));
    match h.call(ONNX, args)? {
        Ok(v) => Ok(v
            .get("providers")
            .and_then(Json::as_array)
            .map(|l| l.iter().filter_map(Json::as_str).map(str::to_owned).collect())),
        Err(f) if matches!(f.code.as_deref(), Some("off" | "no-machine")) => Ok(None),
        Err(f) => Err(thrown(f)),
    }
}

/// `system onnx <providers>` — what the engine's machine separates on, set THERE by the route.
pub fn onnx(a: &str, h: &mut Host) -> Done {
    const NO_MACHINE: &str = "system onnx: no engine is chosen — the setting lives on its machine";
    let spec = js_trim(a);
    if spec.is_empty() {
        let Some(current) = onnx_providers(h, None)? else {
            h.refuse([NO_MACHINE.to_owned()]);
            return Ok(());
        };
        h.refuse([
            "usage: system onnx <providers>   e.g. system onnx dml,cpu".to_owned(),
            if current.is_empty() {
                "nothing is set, so this machine separates on the CPU, which cannot be unavailable".to_owned()
            } else {
                format!("currently: {} — pass \"\" to go back to cpu", current.join(", "))
            },
        ]);
        return Ok(());
    }
    if is_empty_quotes(spec) {
        if onnx_providers(h, Some(&[]))?.is_none() {
            h.refuse([NO_MACHINE.to_owned()]);
            return Ok(());
        }
        h.log(["separation goes back to the CPU".to_owned()]);
        return Ok(());
    }
    let spaced = spec.replace(',', " ");
    let eps: Vec<String> = split_words(&spaced).iter().map(|e| e.to_lowercase()).collect();
    let known = ["cpu", "dml", "cuda", "coreml"];
    let bad: Vec<&str> = eps.iter().map(String::as_str).filter(|e| !known.contains(e)).collect();
    if !bad.is_empty() {
        h.refuse([
            format!("unknown execution provider(s): {}", bad.join(", ")),
            format!("known here: {}", known.join(", ")),
        ]);
        return Ok(());
    }
    if onnx_providers(h, Some(&eps))?.is_none() {
        h.refuse([NO_MACHINE.to_owned()]);
        return Ok(());
    }
    let mut lines = vec![format!("separation tries {}, in that order", eps.join(", "))];
    if eps.iter().any(|e| e == "dml") {
        lines.push(
            "note: `dml` HANGS on some rigs — un-interruptible, and only the job's hour ends it. \
             Measure a song before trusting it (apps/k7-daw-backend/packages/k7-supervisor/docs/stemSplit.md)."
                .to_owned(),
        );
    }
    h.log(lines);
    Ok(())
}

/// `system cue <channels|label>` — the interface pair the CUE bus leaves by.
pub fn cue(a: &str, h: &mut Host) -> Done {
    let spec = js_trim(a);
    let prefs = load_preferences(&*h.fs, &h.env);
    if spec.is_empty() {
        let now = match &prefs.cue_output {
            Some(CueOutput::Channels(c)) => format!("currently: {} — pass \"\" to clear it", plus(c)),
            Some(CueOutput::Label(l)) => format!("currently: {l} — pass \"\" to clear it"),
            None => "nothing is set, so a CUE bus on this machine plays nowhere at all".to_owned(),
        };
        h.refuse([
            "usage: system cue <canaux>   e.g. system cue 3+4, or a label from `system channels`".to_owned(),
            now,
        ]);
        return Ok(());
    }
    if is_empty_quotes(spec) {
        set_cue_output(h.fs, &h.env, None)?;
        h.log(["the headphone pair is cleared — CUE now plays nowhere".to_owned()]);
        return Ok(());
    }
    let channels = parse_channels(spec);
    if !channels.is_empty() {
        let said = plus(&channels);
        set_cue_output(h.fs, &h.env, Some(CueOutput::Channels(channels)))?;
        h.log([format!("CUE leaves by channel(s) {said}")]);
        return Ok(());
    }
    let Some(hit) = prefs.channels_out.iter().find(|c| c.label == spec) else {
        let known = if prefs.channels_out.is_empty() {
            "no output is labelled here — give the numbers instead, e.g. 3+4".to_owned()
        } else {
            let labels: Vec<&str> = prefs.channels_out.iter().map(|c| c.label.as_str()).collect();
            format!("known: {}", labels.join(", "))
        };
        h.refuse([format!("no output labelled '{spec}' on this machine"), known]);
        return Ok(());
    };
    let said = plus(&hit.ch);
    set_cue_output(h.fs, &h.env, Some(CueOutput::Label(spec.to_owned())))?;
    h.log([format!("CUE leaves by '{spec}' (channel(s) {said})")]);
    Ok(())
}
