//! manifest.rs — a version's `library.k7x`: read from k7x, written as k7x.
//! `libraryVersions.ts`' `parseLibraryManifest` and `stringifyLibraryManifest`, byte for byte on what it writes.
//! What a reader gets wrong: a manifest that does not read is none, never a partial one.

use k7x::edit::{StringifyOptions, Value, js_trim, parse, stringify};
use serde_json::{Map, Value as Json, json};

/// What a published version says about itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    pub name: String,
    pub version: u64,
    pub requires: u64,
    pub notes: Option<String>,
    pub seed: Option<u64>,
}

impl Manifest {
    /// A manifest with nothing but the three fields every version has.
    pub fn plain(name: &str, version: u64, requires: u64) -> Manifest {
        Manifest {
            name: name.into(),
            version,
            requires,
            notes: None,
            seed: None,
        }
    }

    /// The listing's spelling: `name, version, requires`, then `notes` and `seed` when set.
    pub fn json(&self) -> Json {
        let mut m = Map::new();
        m.insert("name".into(), json!(self.name));
        m.insert("version".into(), json!(self.version));
        m.insert("requires".into(), json!(self.requires));
        if let Some(n) = &self.notes {
            m.insert("notes".into(), json!(n));
        }
        if let Some(s) = self.seed {
            m.insert("seed".into(), json!(s));
        }
        Json::Object(m)
    }

    /// A manifest from a JSON object the way `libraryManifestData` reads data; None when it is not one.
    pub fn from_json(v: &Json) -> Option<Manifest> {
        let d = v.as_object()?;
        let int = |k: &str| d.get(k).and_then(int_of);
        let version = int("version")?;
        let requires = int("requires").unwrap_or(1);
        let name = d.get("name")?.as_str()?.to_string();
        let notes = d
            .get("notes")
            .and_then(Json::as_str)
            .map(|n| js_trim(n).to_string())
            .filter(|n| !n.is_empty());
        Some(Manifest {
            name,
            version,
            requires,
            notes,
            seed: int("seed"),
        })
    }
}

/// An integer of at least 1, as the manifest's `int` takes it.
fn int_of(v: &Json) -> Option<u64> {
    let n = v.as_f64()?;
    (n.fract() == 0.0 && (1.0..=9_007_199_254_740_991.0).contains(&n)).then_some(n as u64)
}

fn from_value(v: &Value) -> Option<Manifest> {
    let Value::Map(entries) = v else { return None };
    let mut o = Map::new();
    for (k, v) in entries {
        o.insert(
            k.clone(),
            match v {
                Value::Num(n) => json!(n),
                Value::Str(s) => json!(s),
                _ => Json::Null,
            },
        );
    }
    Manifest::from_json(&Json::Object(o))
}

/// A `library.k7x`'s data, or None when the text is not one.
pub fn parse_manifest(text: &str) -> Option<Manifest> {
    from_value(&parse(text).ok()?)
}

/// The text of a `library.k7x`.
pub fn stringify_manifest(m: &Manifest) -> String {
    let mut e = vec![
        ("name".to_string(), Value::Str(m.name.clone())),
        ("version".into(), Value::Num(m.version as f64)),
        ("requires".into(), Value::Num(m.requires as f64)),
    ];
    if let Some(n) = &m.notes {
        e.push(("notes".into(), Value::Str(n.clone())));
    }
    if let Some(s) = m.seed {
        e.push(("seed".into(), Value::Num(s as f64)));
    }
    stringify(&Value::object(e), &StringifyOptions::default()).unwrap_or_default()
}
