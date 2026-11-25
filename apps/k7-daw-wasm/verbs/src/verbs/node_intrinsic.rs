// node_intrinsic.rs — what `node param` writes when the name is no plugin parameter, and `module cc`, after intrinsics.ts and nodeParam.ts.
// A reader would get wrong where an intrinsic lands: the module's file, which may be the workspace's SHARED one.

use super::shared::Done;
use crate::host::Host;
use k7x::edit::{PathSeg, StringifyOptions, Value, js_number};
use song::modules::panels::{module_folder_for_ref, module_own_refs};
use song_edit::Fs;
use song_edit::confine::Env;
use song_edit::js_text::str_to_number;
use song_edit::loader::{load_module_refs, load_modules};
use song_edit::paths::{climbs_out, relative};
use song_edit::project_edit::{
    set_module_base, set_module_camera, set_module_channel, set_module_device, set_module_input_gain, set_module_mix,
    set_module_monitor, set_module_shader, set_module_volume,
};
use song_edit::workspace_doc::{ws_data, ws_write_data};
use song_edit::workspace_file::ws_exists;

const INTRINSICS: [&str; 10] = [
    "channel",
    "monitor",
    "gain",
    "mix",
    "input-volume",
    "output-volume",
    "shader",
    "camera",
    "device",
    "base",
];

fn bad(msg: String) -> song_edit::Error {
    song_edit::Error::Msg(msg)
}

/// `none`-like words clear a field, written as the empty name.
fn cleared<'a>(raw: &'a str, word: &str) -> &'a str {
    if raw.trim() == word { "" } else { raw.trim() }
}

fn volume(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    module: &str,
    side: &str,
    raw: &str,
) -> song_edit::Res<(String, String)> {
    let db = str_to_number(raw);
    if !db.is_finite() || !(-60.0..=12.0).contains(&db) {
        return Err(bad(format!("'{raw}' is not a volume between -60 and +12 dB")));
    }
    let sign = if db > 0.0 { "+" } else { "" };
    Ok((
        set_module_volume(fs, env, dir, module, side, db)?,
        format!("{side} volume {sign}{} dB", js_number(db)),
    ))
}

/// One intrinsic written: the file it landed in and what to say, or None for a field no module has.
fn write(h: &mut Host, module: &str, field: &str, raw: &str) -> Option<song_edit::Res<(String, String)>> {
    let (fs, env, dir) = (&mut *h.fs, &h.env, h.song_path.as_str());
    let named = |path: song_edit::Res<String>, said: String| path.map(|p| (p, said));
    Some(match field {
        "channel" => {
            let ch: Vec<f64> = raw.split(',').map(str_to_number).collect();
            if ch.iter().any(|n| !n.is_finite() || n.fract() != 0.0 || *n < 1.0) {
                Err(bad(format!("'{raw}' is not a channel or a pair of them")))
            } else {
                let said: Vec<String> = ch.iter().map(|n| js_number(*n)).collect();
                named(
                    set_module_channel(fs, env, dir, module, &ch),
                    format!("channel {}", said.join(", ")),
                )
            }
        }
        "monitor" if matches!(raw, "off" | "auto" | "on") => {
            named(set_module_monitor(fs, env, dir, module, raw), format!("monitor {raw}"))
        }
        "monitor" => Err(bad(format!("'{raw}' is not off, auto or on"))),
        "gain" => {
            let db = str_to_number(raw);
            if db.is_finite() {
                named(
                    set_module_input_gain(fs, env, dir, module, db),
                    format!("gain {} dB", js_number(db)),
                )
            } else {
                Err(bad(format!("'{raw}' is not a number of dB")))
            }
        }
        "mix" => {
            let mix = str_to_number(raw);
            if mix.is_finite() && (0.0..=1.0).contains(&mix) {
                let pct = song_edit::start_time::js_round(mix * 100.0);
                named(
                    set_module_mix(fs, env, dir, module, mix),
                    format!("mix {}% wet", js_number(pct)),
                )
            } else {
                Err(bad(format!("'{raw}' is not a mix between 0 and 1")))
            }
        }
        "input-volume" => volume(fs, env, dir, module, "input", raw),
        "output-volume" => volume(fs, env, dir, module, "output", raw),
        "shader" => {
            let n = cleared(raw, "none");
            let said = if n.is_empty() {
                "no shader".to_owned()
            } else {
                format!("shader {n}")
            };
            named(set_module_shader(fs, env, dir, module, n), said)
        }
        "camera" => {
            let n = cleared(raw, "default");
            let said = if n.is_empty() {
                "the default camera".to_owned()
            } else {
                format!("camera {n}")
            };
            named(set_module_camera(fs, env, dir, module, n), said)
        }
        "device" => {
            let n = cleared(raw, "any");
            let said = if n.is_empty() {
                "any MIDI device".to_owned()
            } else {
                format!("MIDI device {n}")
            };
            named(set_module_device(fs, env, dir, module, n), said)
        }
        "base" => {
            let n = cleared(raw, "none");
            let said = if n.is_empty() {
                "no base".to_owned()
            } else {
                format!("base {n}")
            };
            named(set_module_base(fs, env, dir, module, n), said)
        }
        _ => return None,
    })
}

/// `node param <node> @<field> <value>` — one of the module's own fields, saying when it is a shared file.
pub fn set_intrinsic(h: &mut Host, module: &str, field: &str, raw: &str) -> Done {
    let Some(done) = write(h, module, field, raw) else {
        let all: Vec<String> = INTRINSICS.iter().map(|f| format!("@{f}")).collect();
        h.refuse([format!(
            "no intrinsic '@{field}' — a module's own fields are {}",
            all.join(", ")
        )]);
        return Ok(());
    };
    match done {
        Ok((path, said)) => {
            let shared = if climbs_out(&relative(&h.song_path, &path)) {
                " — in the shared library, so every project beside this one sees it"
            } else {
                ""
            };
            h.log([format!("{module}: {said}{shared}")]);
            h.reload();
        }
        Err(e) => h.refuse([format!("node param @{field} failed: {e}")]),
    }
    Ok(())
}

/// A control: its MIDI channel and CC number.
type Binding = (f64, f64);

const CC_USAGE: &str = "usage: module cc <module> <param> <channel 1-16> <cc 0-127>  (e.g. module cc synth Cutoff 1 74)\n       module cc <module> <param> none            — unbind";

/// The line as `<module> <param…> none`, or `<module> <param…> <ch> <cc>`.
fn cc_parse(a: &str) -> Option<(String, String, Option<Binding>)> {
    let w: Vec<&str> = a.split_whitespace().collect();
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let unquote = |s: String| {
        s.strip_prefix('"')
            .and_then(|x| x.strip_suffix('"'))
            .map_or(s.clone(), str::to_owned)
    };
    if w.len() >= 3 && w[w.len() - 1].eq_ignore_ascii_case("none") {
        let param = w[1..w.len() - 1].join(" ");
        return Some((w[0].to_owned(), unquote(param).trim().to_owned(), None));
    }
    if w.len() >= 4 && digits(w[w.len() - 1]) && digits(w[w.len() - 2]) {
        let param = w[1..w.len() - 2].join(" ");
        let b = (str_to_number(w[w.len() - 2]), str_to_number(w[w.len() - 1]));
        return Some((w[0].to_owned(), unquote(param).trim().to_owned(), Some(b)));
    }
    None
}

/// `module cc <module> <param> <ch> <cc>|none` — the MODULE's `controller.k7x`, shared by every node of it.
pub fn module_cc(a: &str, h: &mut Host) -> Done {
    let Some((module, param, binding)) = cc_parse(a.trim()) else {
        h.refuse([CC_USAGE.to_owned()]);
        return Ok(());
    };
    if let Some((ch, cc)) = binding {
        if !(1.0..=16.0).contains(&ch) {
            h.refuse([format!("channel {} is not 1-16", js_number(ch)), CC_USAGE.to_owned()]);
            return Ok(());
        }
        if !(0.0..=127.0).contains(&cc) {
            h.refuse([format!("cc {} is not 0-127", js_number(cc)), CC_USAGE.to_owned()]);
            return Ok(());
        }
    }
    match bind_cc(h, &module, &param, binding) {
        Ok((rel, r)) => {
            let first = match binding {
                Some((ch, cc)) => format!("{module}: {param} <- ch{} cc{}  ({rel})", js_number(ch), js_number(cc)),
                None => format!("{module}: {param} unbound  ({rel})"),
            };
            h.log([first, format!("{r}'s map — shared by every node of it")]);
        }
        Err(e) => h.refuse([format!("module cc failed: {e}")]),
    }
    Ok(())
}

/// `bindModuleCc`: the first own ref of the node that has a folder holds the map.
fn bind_cc(h: &mut Host, module: &str, param: &str, binding: Option<(f64, f64)>) -> song_edit::Res<(String, String)> {
    let dir = h.song_path.clone();
    let modules = load_modules(&*h.fs, &h.env, &dir)?;
    let Some(m) = modules.get(module) else {
        return Err(bad(format!("no module \"{module}\" in this project")));
    };
    let vocab = load_module_refs(&*h.fs, &h.env, &dir)?;
    let mut found = None;
    for r in module_own_refs(m) {
        if let Some(folder) = module_folder_for_ref(&*h.fs, &vocab.folders, &r).map_err(|e| bad(e.to_string()))? {
            found = Some((r, folder));
            break;
        }
    }
    let Some((r, folder)) = found else {
        return Err(bad(format!(
            "module '{module}' has no folder to hold a controller map — run `module own {module}` first"
        )));
    };
    if param.trim().is_empty() {
        return Err(bad("a binding needs a parameter name".to_owned()));
    }
    let path = format!("{folder}/controller.k7x");
    let mut map = match ws_exists(&*h.fs, &path).then(|| ws_data(&*h.fs, &path)).transpose()? {
        Some(Value::Map(m)) => m,
        _ => Vec::new(),
    };
    match binding {
        Some((ch, cc)) => {
            let b = Value::Map(vec![
                ("ch".to_owned(), Value::Num(ch)),
                ("cc".to_owned(), Value::Num(cc)),
            ]);
            match map.iter_mut().find(|(k, _)| k == param) {
                Some(slot) => slot.1 = b,
                None => map.push((param.to_owned(), b)),
            }
        }
        None => map.retain(|(k, _)| k != param),
    }
    let header = [format!(
        "MIDI map for {r} — <param>: {{ ch, cc }}. Shared by every node of this module."
    )];
    let inline = |p: &[PathSeg], _: &Value| p.len() == 1;
    let opts = StringifyOptions {
        header: &header,
        inline: Some(&inline),
    };
    ws_write_data(h.fs, &path, &Value::Map(map), &opts)?;
    Ok((relative(&dir, &path), r))
}
