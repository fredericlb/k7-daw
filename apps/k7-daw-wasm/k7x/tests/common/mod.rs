// common — what the vector tests share: a JSON reader that keeps key order, the value decoding and the op runner.
// A reader would get wrong why there is no serde_json: its maps lose the order a vector's objects were written in.
#![allow(dead_code, unused_macros)]

use k7x::edit::{
    Document, EditError, PathSeg, StringifyOptions, Value, WriteOptions, apply_sugared, js_number,
};
use k7x::edit::{Sugar, SugarPart};

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn get(&self, k: &str) -> Option<&Json> {
        match self {
            Json::Obj(m) => m.iter().find(|(x, _)| x == k).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn str(&self) -> &str {
        match self {
            Json::Str(s) => s,
            other => panic!("not a string: {other:?}"),
        }
    }

    pub fn arr(&self) -> &[Json] {
        match self {
            Json::Arr(a) => a,
            other => panic!("not an array: {other:?}"),
        }
    }

    pub fn num(&self) -> usize {
        match self {
            Json::Num(n) => *n as usize,
            other => panic!("not a number: {other:?}"),
        }
    }
}

pub fn parse_json(s: &str) -> Json {
    let mut p = P {
        b: s.as_bytes(),
        s,
        i: 0,
    };
    let v = p.value();
    p.ws();
    assert_eq!(p.i, s.len(), "trailing JSON");
    v
}

struct P<'a> {
    b: &'a [u8],
    s: &'a str,
    i: usize,
}

impl P<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && self.b[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn value(&mut self) -> Json {
        self.ws();
        match self.b[self.i] {
            b'{' => {
                self.i += 1;
                let mut m = Vec::new();
                loop {
                    self.ws();
                    if self.b[self.i] == b'}' {
                        self.i += 1;
                        return Json::Obj(m);
                    }
                    if self.b[self.i] == b',' {
                        self.i += 1;
                        continue;
                    }
                    let k = self.string();
                    self.ws();
                    self.i += 1;
                    m.push((k, self.value()));
                }
            }
            b'[' => {
                self.i += 1;
                let mut a = Vec::new();
                loop {
                    self.ws();
                    if self.b[self.i] == b']' {
                        self.i += 1;
                        return Json::Arr(a);
                    }
                    if self.b[self.i] == b',' {
                        self.i += 1;
                        continue;
                    }
                    a.push(self.value());
                }
            }
            b'"' => Json::Str(self.string()),
            b't' => {
                self.i += 4;
                Json::Bool(true)
            }
            b'f' => {
                self.i += 5;
                Json::Bool(false)
            }
            b'n' => {
                self.i += 4;
                Json::Null
            }
            _ => {
                let st = self.i;
                while self.i < self.b.len()
                    && matches!(
                        self.b[self.i],
                        b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'
                    )
                {
                    self.i += 1;
                }
                Json::Num(self.s[st..self.i].parse().expect("a JSON number"))
            }
        }
    }

    fn string(&mut self) -> String {
        self.i += 1;
        let mut out = String::new();
        let mut units: Vec<u16> = Vec::new();
        let flush = |units: &mut Vec<u16>, out: &mut String| {
            out.push_str(&String::from_utf16_lossy(units));
            units.clear();
        };
        loop {
            let c = self.b[self.i];
            if c == b'"' {
                self.i += 1;
                flush(&mut units, &mut out);
                return out;
            }
            if c == b'\\' {
                let e = self.b[self.i + 1];
                self.i += 2;
                match e {
                    b'u' => {
                        units.push(u16::from_str_radix(&self.s[self.i..self.i + 4], 16).unwrap());
                        self.i += 4;
                    }
                    _ => {
                        flush(&mut units, &mut out);
                        out.push(match e {
                            b'n' => '\n',
                            b't' => '\t',
                            b'r' => '\r',
                            b'b' => '\u{8}',
                            b'f' => '\u{c}',
                            other => other as char,
                        });
                    }
                }
                continue;
            }
            flush(&mut units, &mut out);
            let ch = self.s[self.i..].chars().next().unwrap();
            out.push(ch);
            self.i += ch.len_utf8();
        }
    }
}

/// A vector's value: JSON with `{"$num": "-0"}` for what JSON cannot spell.
pub fn value_of(j: &Json) -> Value {
    match j {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Bool(*b),
        Json::Num(n) => Value::Num(*n),
        Json::Str(s) => Value::Str(s.clone()),
        Json::Arr(a) => Value::List(a.iter().map(value_of).collect()),
        Json::Obj(m) => match m.as_slice() {
            [(k, Json::Str(t))] if k == "$num" => Value::Num(match t.as_str() {
                "-0" => -0.0,
                "NaN" => f64::NAN,
                "Infinity" => f64::INFINITY,
                _ => f64::NEG_INFINITY,
            }),
            _ => Value::object(m.iter().map(|(k, v)| (k.clone(), value_of(v))).collect()),
        },
    }
}

/// The JSON a value is recorded as, `enc` of the generator.
pub fn json_of(v: &Value) -> Json {
    match v {
        Value::Null => Json::Null,
        Value::Bool(b) => Json::Bool(*b),
        Value::Num(n) if !n.is_finite() || (*n == 0.0 && n.is_sign_negative()) => {
            Json::Obj(vec![(
                "$num".into(),
                Json::Str(if *n == 0.0 {
                    "-0".into()
                } else {
                    js_number(*n)
                }),
            )])
        }
        Value::Num(n) => Json::Num(*n),
        Value::Str(s) => Json::Str(s.clone()),
        Value::List(a) => Json::Arr(a.iter().map(json_of).collect()),
        Value::Map(m) => Json::Obj(m.iter().map(|(k, v)| (k.clone(), json_of(v))).collect()),
    }
}

pub fn path_of(j: Option<&Json>) -> Vec<PathSeg> {
    j.map_or_else(Vec::new, |p| {
        p.arr()
            .iter()
            .map(|s| match s {
                Json::Str(k) => PathSeg::Key(k.clone()),
                other => PathSeg::Index(other.num()),
            })
            .collect()
    })
}

struct SpecSugar(Vec<SugarPart>, Vec<(String, String)>);

impl Sugar for SpecSugar {
    fn parts(&self) -> &[SugarPart] {
        &self.0
    }

    fn expand(&self, raw: &Value) -> Value {
        let Value::Map(m) = raw else {
            return raw.clone();
        };
        let mut out = m.clone();
        for (key, target) in &self.1 {
            if key == target || !m.iter().any(|(k, _)| k == key) {
                continue;
            }
            let v = raw.get(key).cloned().unwrap_or(Value::Null);
            out.retain(|(k, _)| k != key);
            match out.iter_mut().find(|(k, _)| k == target) {
                Some(slot) => slot.1 = v,
                None => out.push((target.clone(), v)),
            }
        }
        Value::object(out)
    }
}

fn sugar_of(spec: &Json) -> SpecSugar {
    let pairs: Vec<(String, String)> = spec
        .get("parts")
        .unwrap()
        .arr()
        .iter()
        .map(|p| {
            (
                p.get("key").unwrap().str().to_owned(),
                p.get("target").unwrap().str().to_owned(),
            )
        })
        .collect();
    let parts = pairs
        .iter()
        .map(|(k, t)| {
            let key = k.clone();
            SugarPart {
                key: k.clone(),
                target: t.clone(),
                present: Box::new(move |raw: &Value| raw.get(&key).is_some()),
            }
        })
        .collect();
    SpecSugar(parts, pairs)
}

fn lines_of(j: &Json) -> Vec<String> {
    j.arr().iter().map(|l| l.str().to_owned()).collect()
}

/// Runs one op on the document; a query gives its recorded answer.
pub fn exec(doc: &mut Document, op: &Json) -> Result<Option<Json>, EditError> {
    let name = op.get("op").unwrap().str();
    let path = path_of(op.get("path"));
    let value = || value_of(op.get("value").unwrap());
    let opts = WriteOptions {
        inline: matches!(op.get("inline"), Some(Json::Bool(true))),
    };
    let source = || op.get("source").unwrap().str().to_owned();
    let n = |k: &str| op.get(k).unwrap().num();
    match name {
        "setIn" => doc.set_in(&path, &value(), opts)?,
        "setSourceIn" => doc.set_source_in(&path, &source())?,
        "addIn" => doc.add_in(&path, &value(), opts)?,
        "replaceSource" => doc.replace_source(&path, &source())?,
        "insertIn" => doc.insert_in(&path, n("index"), &value(), opts)?,
        "insertSourceIn" => doc.insert_source_in(&path, n("index"), &source())?,
        "moveIn" => doc.move_in(&path, n("from"), n("to"))?,
        "renameKey" => doc.rename_key(&path, op.get("key").unwrap().str())?,
        "applyIn" => doc.apply_in(&path, &value())?,
        "deleteIn" => {
            doc.delete_in(&path, matches!(op.get("prune"), Some(Json::Bool(true))))?;
        }
        "setHeader" => doc.set_header(&lines_of(op.get("lines").unwrap()))?,
        "applySugared" => {
            let after = value_of(op.get("after").unwrap());
            let sugar = op.get("sugar").map(sugar_of);
            apply_sugared(doc, &after, sugar.as_ref().map(|s| s as &dyn Sugar))?;
        }
        "getIn" => {
            let v = doc.get_in(&path)?;
            let has = v.is_some();
            return Ok(Some(Json::Obj(vec![
                ("g".into(), v.as_ref().map_or(Json::Null, json_of)),
                ("has".into(), Json::Bool(has)),
            ])));
        }
        "getSource" => {
            let s = doc.get_source(&path)?;
            return Ok(Some(Json::Obj(vec![(
                "g".into(),
                s.map_or(Json::Null, Json::Str),
            )])));
        }
        "positionOf" => {
            let p = doc.position_of(&path)?;
            let g = p.map_or(Json::Null, |(l, c)| {
                Json::Obj(vec![
                    ("line".into(), Json::Num(l as f64)),
                    ("col".into(), Json::Num(c as f64)),
                ])
            });
            return Ok(Some(Json::Obj(vec![("g".into(), g)])));
        }
        other => panic!("unknown op {other}"),
    }
    Ok(None)
}

pub fn default_opts() -> StringifyOptions<'static> {
    StringifyOptions::default()
}

/// The 64-bit hash the generator writes, as two 32-bit FNV-1a runs.
pub fn fnv(s: &str) -> String {
    let (mut h1, mut h2) = (0x811c_9dc5_u32, 0x0100_0193_u32 ^ 0x9e37_79b9);
    for c in s.bytes() {
        h1 = (h1 ^ u32::from(c)).wrapping_mul(0x0100_0193);
        h2 = (h2 ^ u32::from(c)).wrapping_mul(0x85eb_ca6b);
    }
    format!("{h1:08x}{h2:08x}")
}

// ---- the hand-written document tests' shorthand ----

pub trait Seg {
    fn seg(self) -> PathSeg;
}
impl Seg for &str {
    fn seg(self) -> PathSeg {
        PathSeg::Key(self.to_owned())
    }
}
impl Seg for usize {
    fn seg(self) -> PathSeg {
        PathSeg::Index(self)
    }
}
macro_rules! p {
    ($($x:expr),* $(,)?) => { vec![$($crate::common::Seg::seg($x)),*] };
}

pub fn v(json: &str) -> Value {
    value_of(&parse_json(json))
}

pub fn edit(text: &str, f: impl FnOnce(&mut Document)) -> String {
    let mut d = Document::new(text);
    f(&mut d);
    d.text().to_owned()
}

pub const INLINE: WriteOptions = WriteOptions { inline: true };
pub const PLAIN: WriteOptions = WriteOptions { inline: false };

pub fn set(d: &mut Document, path: &[PathSeg], value: &str) {
    d.set_in(path, &v(value), PLAIN).unwrap();
}

pub fn data(text: &str) -> Value {
    k7x::edit::parse(text).unwrap()
}

pub fn lines(l: &[&str]) -> Vec<String> {
    l.iter().map(|s| (*s).to_owned()).collect()
}
