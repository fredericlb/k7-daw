// modules/keys.rs — how a module is KEYED and referenced: the chain key a path points into, a library's ref, a shader knob's uniform, a file param's picks.
// A reader would get wrong that `module_key_from_path` takes the LAST `modules/` segment: a workspace-relative path and an absolute one land on the same one.

use super::files::{LIBRARY_FILE_FAMILIES, LIBRARY_FILE_PARAMS, TAP_SAMPLE};
use super::types::MODULE_TYPES;

/// The chain KEY of the module a path points into: everything between the type folder and the filename.
#[must_use]
pub fn module_key_from_path(path: &str) -> Option<String> {
    let segs = collapse(path);
    let at = segs.iter().rposition(|s| *s == "modules")?;
    let rest = &segs[at + 1..];
    if rest.len() < 3 || !MODULE_TYPES.iter().any(|t| t.name == rest[0]) {
        return None;
    }
    Some(rest[1..rest.len() - 1].join("/"))
}

/// `path` split on runs of `/` and `\`, the empty edges kept as the page's regexp split keeps them.
fn collapse(path: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut in_sep = false;
    for (i, c) in path.char_indices() {
        let sep = c == '/' || c == '\\';
        if sep && !in_sep {
            out.push(&path[start..i]);
            in_sep = true;
        } else if !sep && in_sep {
            start = i;
            in_sep = false;
        }
    }
    out.push(if in_sep { "" } else { &path[start..] });
    out
}

/// The ref a library gives the thing at `rest` inside it: `@k7-core/audio/reverb`.
#[must_use]
pub fn library_ref(library: &str, rest: &str) -> String {
    format!("@{library}/{rest}")
}

/// The uniform each shader knob lands on: one rule, so a panel and a shader cannot disagree.
#[must_use]
pub fn shader_uniform_for(param: &str) -> String {
    format!("u{param}")
}

/// The extensions a file-valued param takes: an amp model is a NAM file or an RTNeural JSON, any other its own NAME (`onnx` loads `.onnx`).
fn asset_exts_for(param: &str) -> Vec<String> {
    match param {
        "model" => vec![".nam".to_owned(), ".json".to_owned()],
        _ => vec![format!(".{}", param.to_lowercase())],
    }
}

fn family_exts(family: &str) -> Option<&'static [&'static str]> {
    LIBRARY_FILE_FAMILIES.iter().find(|(f, _)| *f == family).map(|(_, e)| *e)
}

/// Those of `files` a param of this name can take: a library pick only the `@<library>/…` sound files; a panel naming its `family` filters by that.
#[must_use]
pub fn assets_for_param<'a>(files: &'a [String], param: &str, family: Option<&str>) -> Vec<&'a String> {
    let ends = |f: &str, exts: &[&str]| {
        let lower = f.to_lowercase();
        exts.iter().any(|e| lower.ends_with(e))
    };
    // A panel naming a family says what its picker takes, whatever the bound param is called.
    if let Some(exts) = family.and_then(family_exts) {
        return files.iter().filter(|f| ends(f, exts)).collect();
    }
    if LIBRARY_FILE_PARAMS.iter().any(|(p, _)| *p == param) || param == TAP_SAMPLE {
        let sounds = family_exts("samples").unwrap_or(&[]);
        return files.iter().filter(|f| f.starts_with('@') && ends(f, sounds)).collect();
    }
    let owned = asset_exts_for(param);
    let exts: Vec<&str> = owned.iter().map(String::as_str).collect();
    files.iter().filter(|f| ends(f, &exts)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_names_the_module_it_points_into() {
        let key = module_key_from_path;
        assert_eq!(key("modules/audio/amp/nam/module.k7x").as_deref(), Some("amp/nam"));
        assert_eq!(key("/home/a/ws/modules/instruments/bass/module.k7x").as_deref(), Some("bass"));
        assert_eq!(key("C:\\ws\\modules\\audio\\reverb\\module.k7x").as_deref(), Some("reverb"));
        assert_eq!(key("modules//audio//reverb//module.k7x").as_deref(), Some("reverb"));
        assert_eq!(key("modules/modules/audio/x/y.k7x").as_deref(), Some("x"));
    }

    #[test]
    fn a_path_that_points_into_no_module_names_none() {
        for p in ["", "audio/reverb/module.k7x", "modules/audio/module.k7x", "modules/nope/x/module.k7x", "modules"] {
            assert_eq!(module_key_from_path(p), None, "{p}");
        }
    }

    #[test]
    fn a_ref_and_a_uniform_are_spelled_one_way() {
        assert_eq!(library_ref("k7-core", "audio/reverb"), "@k7-core/audio/reverb");
        assert_eq!(shader_uniform_for("P1"), "uP1");
    }

    fn files() -> Vec<String> {
        ["a.nam", "B.JSON", "c.onnx", "@k7-core/irs/hall.wav", "@k7-core/samples/k.flac", "plain.wav", "@k7-core/models/x.nam"]
            .map(str::to_owned)
            .to_vec()
    }

    fn picks(param: &str, family: Option<&str>) -> Vec<String> {
        assets_for_param(&files(), param, family).into_iter().cloned().collect()
    }

    #[test]
    fn a_file_param_takes_its_own_extension_and_a_model_takes_two() {
        assert_eq!(picks("onnx", None), ["c.onnx"]);
        assert_eq!(picks("model", None), ["a.nam", "B.JSON", "@k7-core/models/x.nam"]);
    }

    #[test]
    fn a_library_param_takes_only_the_librarys_sound_files() {
        let want = ["@k7-core/irs/hall.wav", "@k7-core/samples/k.flac"];
        assert_eq!(picks("ir", None), want);
        assert_eq!(picks("sample", None), want);
    }

    #[test]
    fn a_panels_family_wins_over_the_bound_params_name() {
        assert_eq!(picks("whatever", Some("models")), ["a.nam", "B.JSON", "@k7-core/models/x.nam"]);
        assert_eq!(picks("onnx", Some("nope")), ["c.onnx"]);
    }
}
