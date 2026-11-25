// node_param.rs — `node param <node> <param> <value>`: a plugin's parameter, a shader's knob, OUT's volume or a file, after nodeParam.ts.
// A reader would get wrong the push: the file is written first, then the mix is pushed as values, never a reload.

use super::node_intrinsic::set_intrinsic;
use super::node_shared::{library_files, plugin_module};
use super::shared::{Done, join};
use crate::host::Host;
use k7x::edit::{PathSeg, Value, js_number};
use song::lanes::{CUE_NODE, OUT_NODE, VIDEO_OUT_NODE, is_builtin_output};
use song::modules::files::{FILE_PARAMS, library_family_of, shared_family_of};
use song::routing::{MIX_NODE, RENDER_NODE};
use song_edit::js_text::str_to_number;
use song_edit::list_files::{ListOpts, list_files_under};
use song_edit::loader::{load_module_refs, resolve_node};
use song_edit::project_edit::{set_module_asset, set_module_param};
use song_edit::yaml_edit::{delete_file_value, set_file_value};

const SHADER_PARAMS: [&str; 4] = ["P1", "P2", "P3", "P4"];
const ASSET_EXTS: [&str; 3] = [".nam", ".json", ".onnx"];
const MASTER_MIN_DB: f64 = -100.0;
const MASTER_MAX_DB: f64 = 12.0;
const USAGE: &str = r#"usage: node param <node> <param> <value>  (e.g. node param synth "Filter Freq" 69)"#;

/// The first word, or a `"quoted"` one with its quotes, and the rest after its spaces.
fn head(s: &str) -> Option<(&str, &str)> {
    let end = if let Some(body) = s.strip_prefix('"') {
        body.find('"').map(|i| i + 2)?
    } else {
        s.find(char::is_whitespace)?
    };
    let rest = s[end..].trim_start();
    (end < s.len() && s[end..].starts_with(char::is_whitespace) && !rest.is_empty()).then_some((&s[..end], rest))
}

/// The value at the end of `rest`: a `"quoted"` one whole when it is, else the last word; the param is what is left.
fn tail(rest: &str) -> Option<(&str, &str)> {
    if rest.len() > 1 && rest.ends_with('"') {
        let open = rest[..rest.len() - 1].rfind('"');
        if let Some(o) = open
            && rest[..o].ends_with(char::is_whitespace)
            && !rest[..o].trim().is_empty()
        {
            return Some((rest[..o].trim_end(), &rest[o..]));
        }
    }
    let at = rest.rfind(char::is_whitespace)?;
    let param = rest[..at].trim_end();
    (!param.is_empty()).then_some((param, &rest[at + 1..]))
}

fn unquote(s: &str) -> &str {
    s.strip_prefix('"')
        .and_then(|x| x.strip_suffix('"'))
        .unwrap_or(s)
        .trim()
}

/// The line as node, param and value, as the two TypeScript patterns read it: an `@field` takes the REST as its value.
fn parse(a: &str) -> Option<(String, String, String)> {
    let (node, rest) = head(a.trim())?;
    if rest.starts_with('@') {
        let end = rest.find(char::is_whitespace)?;
        let value = rest[end..].trim_start();
        if !value.is_empty() {
            return Some((
                unquote(node).to_owned(),
                rest[..end].to_owned(),
                unquote(value).to_owned(),
            ));
        }
    }
    let (param, value) = tail(rest)?;
    Some((
        unquote(node).to_owned(),
        unquote(param).to_owned(),
        unquote(value).to_owned(),
    ))
}

/// `node param <node> <param> <value>` — node, param and value may each be quoted.
pub fn node_param(a: &str, h: &mut Host) -> Done {
    let Some((module, param, raw)) = parse(a) else {
        h.refuse([USAGE.to_owned()]);
        return Ok(());
    };
    if let Some(field) = param.strip_prefix('@') {
        return set_intrinsic(h, &module, field, &raw);
    }
    if FILE_PARAMS.contains(&param.as_str()) || library_family_of(&param).is_some() {
        return set_file_param(h, &module, &param, &raw);
    }
    let value = str_to_number(&raw);
    if !value.is_finite() {
        h.refuse([format!("\"{raw}\" is not a number — a param value is numeric")]);
        return Ok(());
    }
    if is_builtin_output(&module) {
        return builtin_out(h, &module, &param, value);
    }
    let dir = h.song_path.clone();
    if resolve_node(&*h.fs, &h.env, &dir, &module)?.is_some_and(|n| n.authored.shader.is_some()) {
        return shader_param(h, &module, &param, value);
    }
    let p = h.project()?;
    let all: Vec<&str> = p
        .tracks
        .iter()
        .flat_map(|t| t.plugins.iter())
        .chain(p.master_plugins.iter())
        .map(|pl| plugin_module(&pl.id))
        .collect();
    if !all.contains(&module.as_str()) {
        let mut seen: Vec<&str> = Vec::new();
        for m in &all {
            if !seen.contains(m) {
                seen.push(m);
            }
        }
        let hint = if seen.is_empty() {
            String::new()
        } else {
            format!(" — try: {}", seen.join(", "))
        };
        h.refuse([format!("no plugin module '{module}'{hint}")]);
        return Ok(());
    }
    if let Err(e) = set_module_param(h.fs, &h.env, &dir, &module, &param, value) {
        h.refuse([format!("node param failed: {e}")]);
        return Ok(());
    }
    h.apply_project();
    h.log([format!("{module}: {param} = {}", js_number(value))]);
    Ok(())
}

/// A shader's knob, P1..P4 clamped to 0..1, read by the picture's compile; the file is the whole write.
fn shader_param(h: &mut Host, module: &str, param: &str, value: f64) -> Done {
    if !SHADER_PARAMS.contains(&param) {
        h.refuse([format!(
            "{module} is a shader: its knobs are {}, not '{param}'",
            SHADER_PARAMS.join(", ")
        )]);
        return Ok(());
    }
    let v = value.clamp(0.0, 1.0);
    let dir = h.song_path.clone();
    match set_module_param(h.fs, &h.env, &dir, module, param, v) {
        Ok(_) => {
            h.log([format!("{module}: {param} = {}", js_number(v))]);
            h.reload();
        }
        Err(e) => h.refuse([format!("node param failed: {e}")]),
    }
    Ok(())
}

/// The built-in outputs: only OUT carries a param, its Volume, written as `project.masterVolume`.
fn builtin_out(h: &mut Host, node: &str, param: &str, value: f64) -> Done {
    let v = js_number(value);
    if node == MIX_NODE {
        h.refuse([
            format!("{MIX_NODE} is the mixer's sum and carries no level — the output level is {OUT_NODE}'s"),
            format!("try: node param \"{OUT_NODE}\" Volume {v}"),
        ]);
        return Ok(());
    }
    if node == RENDER_NODE || node == VIDEO_OUT_NODE {
        h.refuse([
            format!("{node} carries no param — the picture's outputs are routing only, and a frame has no level"),
            format!("the one built-in param is {OUT_NODE}'s Volume, the song's output level in dB"),
        ]);
        return Ok(());
    }
    if node == CUE_NODE {
        h.refuse([
            format!("{CUE_NODE} carries no level — it is the pair the headphones are plugged into, and that amp has its own knob"),
            format!("set what a feed CONTRIBUTES instead, on the module wired into {CUE_NODE}"),
        ]);
        return Ok(());
    }
    if param != "Volume" {
        h.refuse([format!(
            "{OUT_NODE} has one param, Volume — the song's output level in dB"
        )]);
        return Ok(());
    }
    if !(MASTER_MIN_DB..=MASTER_MAX_DB).contains(&value) {
        let mut why = vec![format!(
            "{v} dB is outside the fader — it runs {}..+{} dB",
            js_number(MASTER_MIN_DB),
            js_number(MASTER_MAX_DB)
        )];
        if value > MASTER_MAX_DB {
            why.push("a mix that needs more than +12 on its output has its headroom in the STRIPS — see apps/k7-daw-frontend/packages/song/docs/masterVolume.md".to_owned());
        }
        h.refuse(why);
        return Ok(());
    }
    let file = join(&h.song_path, "project.k7x");
    let at = [
        PathSeg::Key("project".to_owned()),
        PathSeg::Key("masterVolume".to_owned()),
    ];
    if value == 0.0 {
        delete_file_value(h.fs, &file, &at)?;
    } else {
        set_file_value(h.fs, &file, &at, &Value::Num(value))?;
    }
    h.apply_project();
    h.log([format!("{OUT_NODE}: Volume = {v}")]);
    Ok(())
}

/// Every asset `module` may name: its own folder's `assets/`, then each module it extends.
fn asset_choices(h: &Host, module: &str) -> Result<(Vec<String>, String), song_edit::Error> {
    let node = resolve_node(&*h.fs, &h.env, &h.song_path, module)?;
    let vocab = load_module_refs(&*h.fs, &h.env, &h.song_path)?;
    let opts = ListOpts {
        max_depth: 6,
        ..ListOpts::default()
    };
    let mut out: Vec<String> = Vec::new();
    let mut add = |folder: Option<&String>| {
        let Some(f) = folder else { return };
        let mut files: Vec<String> = list_files_under(&*h.fs, &format!("{f}/assets"), &opts)
            .into_iter()
            .filter(|p| {
                p.rfind('.')
                    .is_some_and(|i| ASSET_EXTS.contains(&p[i..].to_lowercase().as_str()))
            })
            .collect();
        files.sort();
        for x in files {
            if !out.contains(&x) {
                out.push(x);
            }
        }
    };
    add(node.as_ref().and_then(|n| n.folder_abs.as_ref()));
    let mut parent = node.as_ref().and_then(|n| n.authored.extends.clone());
    for _ in 0..16 {
        let Some(m) = parent.as_ref().and_then(|r| vocab.modules.get(r)) else {
            break;
        };
        add(m.folder_abs.as_ref());
        parent = m.authored.extends.clone();
    }
    let file = node.map_or_else(|| module.to_owned(), |n| n.file.clone());
    Ok((out, file))
}

/// A file-valued param pointed at one of the module's assets or a library's file, or cleared with `none`.
fn set_file_param(h: &mut Host, module: &str, param: &str, file: &str) -> Done {
    let clearing = file == "none";
    let family = library_family_of(param).or_else(|| file.starts_with('@').then(|| shared_family_of(param)).flatten());
    if !clearing && let Some(fam) = family {
        let shared = library_files(h, fam);
        if !shared.iter().any(|f| f == file) {
            h.refuse([if shared.is_empty() {
                format!("no library has a {fam}/ folder with a file in it yet")
            } else {
                let more = if shared.len() > 8 { ", …" } else { "" };
                format!(
                    "no '{file}' in the libraries' {fam}/ — they have: {}{more}",
                    shared[..shared.len().min(8)].join(", ")
                )
            }]);
            return Ok(());
        }
    } else if !clearing {
        let (have, folder) = asset_choices(h, module)?;
        if !have.iter().any(|f| f == file) {
            h.refuse([if have.is_empty() {
                format!("{module} carries no assets — drop the file in modules/<category>/{folder}/assets/")
            } else {
                format!("no '{file}' in {module}'s assets — it has: {}", have.join(", "))
            }]);
            return Ok(());
        }
    }
    let dir = h.song_path.clone();
    match set_module_asset(h.fs, &h.env, &dir, module, param, (!clearing).then_some(file)) {
        Ok(_) => {
            h.log([if clearing {
                format!("{module}: {param} cleared")
            } else {
                format!("{module}: {file}")
            }]);
            h.reload();
        }
        Err(e) => h.refuse([format!("node param {param} failed: {e}")]),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse;

    fn p(a: &str) -> Option<(String, String, String)> {
        parse(a)
    }

    #[test]
    fn a_param_line_reads_like_the_two_patterns() {
        let t = |a: &str, b: &str, c: &str| Some((a.to_owned(), b.to_owned(), c.to_owned()));
        assert_eq!(p(r#"synth "Filter Freq" 69"#), t("synth", "Filter Freq", "69"));
        assert_eq!(p("synth Filter Freq 69"), t("synth", "Filter Freq", "69"));
        assert_eq!(p("cam @device Built in Mic"), t("cam", "@device", "Built in Mic"));
        assert_eq!(p(r#""my amp" model "a b.nam""#), t("my amp", "model", "a b.nam"));
        assert_eq!(p("synth 69"), None);
    }
}
