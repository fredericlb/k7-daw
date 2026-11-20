// compare.rs — two JSON values compared as "equal enough", the generic rules of docs/rustCompiler.md.
// A reader would get wrong that key order never counts and array order does, unless the array is named as a set.

use serde_json::Value;
use std::collections::{HashMap, VecDeque};

/// An array, named by its pointer pattern, compared as a set by the identity `key` of each item.
#[derive(Clone, Debug)]
pub struct SetRule {
    pub pointer: String,
    pub key: String,
}

/// A known difference: a pointer pattern (`*` is one segment, a last `**` is the rest) and why it is allowed.
#[derive(Clone, Debug)]
pub struct Allow {
    pub pattern: String,
    pub reason: String,
}

#[derive(Clone, Debug)]
pub struct Rules {
    /// A number is equal within this relative error, or this absolute one near zero.
    pub tolerance: f64,
    pub sets: Vec<SetRule>,
    pub allow: Vec<Allow>,
    /// At most this many differences are collected.
    pub max: usize,
    /// Pointer patterns of `[x, y]` arrays compared as functions: both sampled at every breakpoint of either (decision 355).
    pub functions: Vec<String>,
    /// Two samples of a function are equal within this absolute error (decision 355).
    pub function_tolerance: f64,
    /// Pointer patterns of strings whose trailing `-<16 hex>.wav` is masked as `-#.wav` (decision 355).
    pub masks: Vec<String>,
    /// A difference keeps a value of at most this many bytes of JSON whole, and its first bytes as a string past it (decision 723).
    pub keep: usize,
    /// At most this many values are visited: past it the comparison stops and claims no difference (decision 723).
    pub budget: usize,
}

impl Default for Rules {
    fn default() -> Self {
        Rules {
            tolerance: 1e-9,
            sets: Vec::new(),
            allow: Vec::new(),
            max: 20,
            functions: Vec::new(),
            function_tolerance: 1e-12,
            masks: Vec::new(),
            budget: usize::MAX,
            keep: usize::MAX,
        }
    }
}

/// Where two values differ; None is absent on that side.
#[derive(Clone, Debug, PartialEq)]
pub struct Difference {
    pub pointer: String,
    pub left: Option<Value>,
    pub right: Option<Value>,
}

/// The differences that count, and those an allow-list entry set aside with its reason.
#[derive(Clone, Debug, Default)]
pub struct Compared {
    pub differences: Vec<Difference>,
    pub allowed: Vec<(Difference, String)>,
    /// The budget ended the walk, so no difference is claimed.
    pub capped: bool,
}

/// The differences between `left` and `right`, at most `rules.max`, the allowed ones left out.
pub fn compare(left: &Value, right: &Value, rules: &Rules) -> Vec<Difference> {
    compare_all(left, right, rules).differences
}

pub fn compare_all(left: &Value, right: &Value, rules: &Rules) -> Compared {
    let mut walk = Walk {
        rules,
        out: Compared::default(),
        visited: 0,
    };
    walk.value("", left, right);
    if walk.out.capped {
        walk.out.differences.clear();
        walk.out.allowed.clear();
    }
    walk.out
}

/// `v`, or its first `keep` bytes of JSON as a string when it is longer, so a difference never holds a copy of a megabyte.
fn kept_value(v: &Value, keep: usize) -> Value {
    if keep == usize::MAX || matches!(v, Value::Null | Value::Bool(_) | Value::Number(_)) {
        return v.clone();
    }
    if let Value::String(text) = v {
        let mut end = text.len().min(keep);
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        return Value::String(text[..end].to_owned());
    }
    struct Limit(Vec<u8>, usize);
    impl std::io::Write for Limit {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let room = self.1.saturating_sub(self.0.len());
            self.0.extend_from_slice(&buf[..buf.len().min(room)]);
            if buf.len() > room {
                return Err(std::io::Error::other("cut"));
            }
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut out = Limit(Vec::new(), keep);
    if serde_json::to_writer(&mut out, v).is_ok() {
        return v.clone();
    }
    Value::String(String::from_utf8_lossy(&out.0).into_owned())
}

/// Whether a JSON pointer matches a pattern.
pub fn matches(pattern: &str, pointer: &str) -> bool {
    let (mut pat, mut ptr) = (pattern.split('/'), pointer.split('/'));
    loop {
        match (pat.next(), ptr.next()) {
            (Some("**"), _) => return pat.next().is_none(),
            (Some(p), Some(q)) if p == "*" || p == q => {}
            (None, None) => return true,
            _ => return false,
        }
    }
}

fn escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

struct Walk<'r> {
    rules: &'r Rules,
    out: Compared,
    visited: usize,
}

impl Walk<'_> {
    fn full(&self) -> bool {
        self.out.capped || self.out.differences.len() >= self.rules.max
    }

    fn differ(&mut self, pointer: &str, left: Option<&Value>, right: Option<&Value>) {
        let kept = |v: Option<&Value>| v.map(|v| kept_value(v, self.rules.keep));
        let d = Difference {
            pointer: pointer.to_owned(),
            left: kept(left),
            right: kept(right),
        };
        match self.rules.allow.iter().find(|a| matches(&a.pattern, pointer)) {
            Some(a) if self.out.allowed.len() < self.rules.max => self.out.allowed.push((d, a.reason.clone())),
            Some(_) => {}
            None if !self.full() => self.out.differences.push(d),
            None => {}
        }
    }

    fn close(&self, a: f64, b: f64) -> bool {
        let gap = (a - b).abs();
        a == b || gap <= self.rules.tolerance || gap <= self.rules.tolerance * a.abs().max(b.abs())
    }

    fn value(&mut self, ptr: &str, l: &Value, r: &Value) {
        self.visited += 1;
        if self.visited > self.rules.budget {
            self.out.capped = true;
        }
        if self.full() {
            return;
        }
        if let (Value::Array(a), Value::Array(b)) = (l, r)
            && self.rules.functions.iter().any(|p| matches(p, ptr))
        {
            if !same_function(a, b, self.rules.function_tolerance) {
                self.differ(ptr, Some(l), Some(r));
            }
            return;
        }
        if let (Value::String(a), Value::String(b)) = (l, r)
            && self.rules.masks.iter().any(|p| matches(p, ptr))
        {
            if mask_render(a) != mask_render(b) {
                self.differ(ptr, Some(l), Some(r));
            }
            return;
        }
        match (l, r) {
            (Value::Number(a), Value::Number(b)) => {
                let same = match (a.as_f64(), b.as_f64()) {
                    (Some(a), Some(b)) => self.close(a, b),
                    _ => a == b,
                };
                if !same {
                    self.differ(ptr, Some(l), Some(r));
                }
            }
            (Value::Object(a), Value::Object(b)) => {
                for (k, av) in a {
                    let child = format!("{ptr}/{}", escape(k));
                    match b.get(k) {
                        Some(bv) => self.value(&child, av, bv),
                        None => self.differ(&child, Some(av), None),
                    }
                }
                for (k, bv) in b {
                    if !a.contains_key(k) {
                        self.differ(&format!("{ptr}/{}", escape(k)), None, Some(bv));
                    }
                }
            }
            (Value::Array(a), Value::Array(b)) => match self.rules.sets.iter().find(|s| matches(&s.pointer, ptr)) {
                Some(rule) => self.set(ptr, a, b, &rule.key.clone()),
                None => {
                    for i in 0..a.len().max(b.len()) {
                        let child = format!("{ptr}/{i}");
                        match (a.get(i), b.get(i)) {
                            (Some(av), Some(bv)) => self.value(&child, av, bv),
                            (av, bv) => self.differ(&child, av, bv),
                        }
                    }
                }
            },
            _ if l == r => {}
            _ => self.differ(ptr, Some(l), Some(r)),
        }
    }

    fn set(&mut self, ptr: &str, a: &[Value], b: &[Value], key: &str) {
        let id = |v: &Value| v.get(key).map_or_else(|| v.to_string(), Value::to_string);
        let mut right: HashMap<String, VecDeque<&Value>> = HashMap::new();
        let mut order: Vec<String> = Vec::new();
        for v in b {
            let k = id(v);
            if !right.contains_key(&k) {
                order.push(k.clone());
            }
            right.entry(k).or_default().push_back(v);
        }
        for av in a {
            let k = id(av);
            let child = format!("{ptr}/{}", escape(&k));
            match right.get_mut(&k).and_then(VecDeque::pop_front) {
                Some(bv) => self.value(&child, av, bv),
                None => self.differ(&child, Some(av), None),
            }
        }
        for k in order {
            for bv in right.remove(&k).into_iter().flatten() {
                self.differ(&format!("{ptr}/{}", escape(&k)), None, Some(bv));
            }
        }
    }
}

/// A render's name with its fingerprint hidden: a trailing `-<16 hex>.wav` becomes `-#.wav`.
fn mask_render(s: &str) -> String {
    let Some(stem) = s.strip_suffix(".wav") else {
        return s.to_owned();
    };
    let Some(cut) = stem.len().checked_sub(17) else {
        return s.to_owned();
    };
    match (stem.get(..cut), stem.get(cut..)) {
        (Some(head), Some(tail))
            if tail.starts_with('-') && tail.len() == 17 && tail[1..].bytes().all(|b| b.is_ascii_hexdigit()) =>
        {
            format!("{head}-#.wav")
        }
        _ => s.to_owned(),
    }
}

fn points(a: &[Value]) -> Option<Vec<(f64, f64)>> {
    a.iter()
        .map(|p| {
            let p = p.as_array()?;
            Some((p.first()?.as_f64()?, p.get(1)?.as_f64()?))
        })
        .collect()
}

/// The value of a breakpoint list at `x`: linear between breakpoints, the first held before and the last held after.
fn sample(p: &[(f64, f64)], x: f64) -> f64 {
    let (Some(first), Some(last)) = (p.first(), p.last()) else {
        return 0.0;
    };
    if x <= first.0 {
        return first.1;
    }
    if x >= last.0 {
        return last.1;
    }
    let at = p.partition_point(|b| b.0 <= x);
    let (a, b) = (p[at - 1], p[at]);
    if b.0 == a.0 {
        b.1
    } else {
        a.1 + (b.1 - a.1) * ((x - a.0) / (b.0 - a.0))
    }
}

/// Two breakpoint lists play the same curve: equal at every breakpoint of either, within `tolerance` (absolute).
fn same_function(a: &[Value], b: &[Value], tolerance: f64) -> bool {
    let (Some(a), Some(b)) = (points(a), points(b)) else {
        return false;
    };
    a.iter()
        .chain(&b)
        .all(|&(x, _)| (sample(&a, x) - sample(&b, x)).abs() <= tolerance)
}
