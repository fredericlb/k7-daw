// project_root.rs — project.k7x's root read as the loader reads it: format refused when newer, shape checked, data kept.
// A reader would get wrong what a bad shape does: an empty root, unless the caller asked to validate, then the first finding.

use crate::error::{Res, fail};
use crate::format_level::{FORMAT_LEVEL, format_level_of_text};
use crate::fs::Fs;
use crate::module_io::entity_file_for;
use crate::start_time::above;
use crate::start_time::position;
use crate::workspace_doc::{ws_data, ws_text};
use k7x::edit::{Value, js_number};
use song_schema::is_timecode;

/// The root's data: an empty map when there is no file, or when it does not match the schema and nobody asked to validate.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectRoot {
    pub data: Value,
}

/// A marker as `parseMarkers` reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct Marker {
    pub name: String,
    pub beat: f64,
}

impl ProjectRoot {
    /// The `project:` tier.
    #[must_use]
    pub fn project(&self) -> Option<&Value> {
        self.data.get("project").filter(|v| matches!(v, Value::Map(_)))
    }

    /// The markers, in beat order; one whose position does not read is skipped.
    #[must_use]
    pub fn markers(&self, num: f64, den: f64) -> Vec<Marker> {
        let Some(Value::Map(m)) = self.data.get("markers") else {
            return vec![];
        };
        let mut out: Vec<Marker> = m
            .iter()
            .filter_map(|(tc, name)| match name {
                Value::Str(n) => Some(Marker {
                    name: n.clone(),
                    beat: position(tc, num, den).ok()?,
                }),
                _ => None,
            })
            .collect();
        out.sort_by(|a, b| a.beat.partial_cmp(&b.beat).unwrap_or(std::cmp::Ordering::Equal));
        out
    }
}

fn too_new(level: f64, file: &str) -> Res<()> {
    if level > f64::from(FORMAT_LEVEL) {
        let shown = js_number(level);
        return fail(format!(
            "{file}: this project was written by a newer k7 (format {shown}); this k7 reads format {FORMAT_LEVEL} — update k7 to open it"
        ));
    }
    Ok(())
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Num(_) => "number",
        Value::Str(_) => "string",
        Value::List(_) => "array",
        Value::Map(_) => "object",
    }
}

fn wrong(want: &str, v: &Value) -> String {
    format!("Invalid input: expected {want}, received {}", type_name(v))
}

fn strings(v: &Value) -> Option<String> {
    match v {
        Value::List(l) => l
            .iter()
            .find(|x| !matches!(x, Value::Str(_)))
            .map(|x| wrong("string", x)),
        other => Some(wrong("array", other)),
    }
}

fn wall_clock(k: &str) -> bool {
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let (head, frac) = k.split_once('.').map_or((k, None), |(h, f)| (h, Some(f)));
    if frac.is_some_and(|f| !digits(f)) {
        return false;
    }
    match head.split_once(':') {
        None => digits(head),
        Some((m, s)) => digits(m) && s.len() == 2 && s.as_bytes()[0] <= b'5' && digits(s),
    }
}

fn project_issue(p: &Value) -> Option<(String, String)> {
    let Value::Map(entries) = p else {
        return Some(("project".into(), wrong("object", p)));
    };
    const KNOWN: [&str; 13] = [
        "uuid",
        "name",
        "tempo",
        "sampleRate",
        "timeSignature",
        "masterVolume",
        "console",
        "modules_from",
        "libraries_from",
        "video",
        "viewport",
        "layers",
        "tags",
    ];
    for (k, v) in entries {
        let at = format!("project.{k}");
        let bad = match (k.as_str(), v) {
            ("uuid" | "name", Value::Str(_)) | ("video", Value::Bool(_)) => None,
            ("uuid" | "name", other) => Some(wrong("string", other)),
            ("video", other) => Some(wrong("boolean", other)),
            ("tempo", Value::Num(n)) => (!above(*n, 0.0)).then(|| "Too small: expected number to be >0".to_owned()),
            ("tempo" | "sampleRate" | "masterVolume", other) if !matches!(other, Value::Num(_)) => {
                Some(wrong("number", other))
            }
            ("timeSignature", Value::Str(s)) => {
                let ok = s.split_once('/').is_some_and(|(a, b)| {
                    [a, b]
                        .iter()
                        .all(|x| !x.is_empty() && x.bytes().all(|c| c.is_ascii_digit()))
                });
                (!ok).then(|| "expected a time signature like 4/4".to_owned())
            }
            ("timeSignature" | "console", other) if !matches!(other, Value::Str(_)) => Some(wrong("string", other)),
            ("modules_from" | "libraries_from" | "layers" | "tags", other) => strings(other),
            _ => None,
        };
        if let Some(m) = bad {
            return Some((at, m));
        }
        if !KNOWN.contains(&k.as_str()) {
            return Some(("project".into(), format!("Unrecognized key: \"{k}\"")));
        }
    }
    None
}

fn root_issue(data: &Value) -> Option<(String, String)> {
    let Value::Map(entries) = data else {
        return Some((String::new(), wrong("object", data)));
    };
    for (k, v) in entries {
        let bad = match (k.as_str(), v) {
            ("format", Value::Num(n)) => {
                (n.fract() != 0.0 || *n < 1.0).then(|| "Too small: expected number to be >=1".to_owned())
            }
            ("format", other) => Some(wrong("number", other)),
            ("project", p) => {
                if let Some(found) = project_issue(p) {
                    return Some(found);
                }
                None
            }
            ("type" | "project_type", Value::Str(_)) => None,
            ("type" | "project_type", other) => Some(wrong("string", other)),
            ("trackOrder" | "soloed" | "highlighted", other) => strings(other),
            ("markers", Value::Map(m)) => {
                if let Some((mk, _)) = m.iter().find(|(mk, _)| !is_timecode(mk) && !wall_clock(mk)) {
                    return Some((format!("markers.{mk}"), "Invalid key in record".to_owned()));
                }
                if let Some((mk, x)) = m.iter().find(|(_, x)| !matches!(x, Value::Str(_))) {
                    return Some((format!("markers.{mk}"), wrong("string", x)));
                }
                None
            }
            ("markers", other) => Some(wrong("object", other)),
            _ => return Some((String::new(), format!("Unrecognized key: \"{k}\""))),
        };
        if let Some(m) = bad {
            return Some((k.clone(), m));
        }
    }
    None
}

/// The project's ROOT from `project.k7x`, `{}` when the file is absent.
///
/// # Errors
/// A newer format, a file that does not read, or, with `validate`, a root that does not match its schema.
pub fn load_project_root(fs: &dyn Fs, dir: &str, validate: bool) -> Res<ProjectRoot> {
    let empty = ProjectRoot {
        data: Value::Map(vec![]),
    };
    let Some(file) = entity_file_for(fs, dir, "", "project") else {
        return Ok(empty);
    };
    let rel = file.rel.trim_start_matches('/').to_owned();
    // A newer format is refused whether or not this read validates, so no writer clobbers it.
    if let Some(level) = format_level_of_text(&ws_text(fs, &file.path)?) {
        too_new(f64::from(level), &rel)?;
    }
    let raw = ws_data(fs, &file.path)?;
    if let Some(Value::Num(n)) = raw.get("format").filter(|_| matches!(raw, Value::Map(_)))
        && n.is_finite()
    {
        too_new(*n, &rel)?;
    }
    let data = if matches!(raw, Value::Null) {
        Value::Map(vec![])
    } else {
        raw
    };
    match root_issue(&data) {
        None => Ok(ProjectRoot { data }),
        Some((path, message)) if validate => {
            let path = if path.is_empty() { "(root)".to_owned() } else { path };
            fail(format!("invalid project:\n  {}: {path}: {message}", file.rel))
        }
        Some(_) => Ok(empty),
    }
}
