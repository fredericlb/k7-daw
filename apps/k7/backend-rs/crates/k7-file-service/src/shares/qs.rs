//! qs.rs — the part of express' `qs` 6.16 a share's query goes through, for one root key (the review's fix 4).
//! What a reader gets wrong: `qs` drops an empty value merged into an existing one and turns an index of 1000 or more into an object.

// Express 4 parses with `arrayLimit: 1000`, which is also the parameter limit.
const LIMIT: usize = 1000;
const PARAMETERS: usize = 1000;
const DEPTH: usize = 5;

/// What `req.query[key]` holds.
#[derive(Clone, Debug, PartialEq)]
pub enum Q {
    Str(String),
    Arr(Vec<Option<Q>>),
    Obj(Obj),
}

/// An object in insertion order; `overflow` is the largest index of an array that grew past the limit.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Obj {
    entries: Vec<(String, Q)>,
    overflow: Option<usize>,
}

impl Obj {
    fn get(&self, key: &str) -> Option<&Q> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    fn set(&mut self, key: &str, value: Q) {
        match self.entries.iter_mut().find(|(k, _)| k == key) {
            Some(e) => e.1 = value,
            None => self.entries.push((key.to_owned(), value)),
        }
    }

    /// `Object.keys` order: array indexes ascending, then the rest as they were added.
    fn keys(&self) -> Vec<&str> {
        let mut index: Vec<(u64, &str)> = vec![];
        let mut rest = vec![];
        for (k, _) in &self.entries {
            match canonical_index(k) {
                Some(n) => index.push((n, k)),
                None => rest.push(k.as_str()),
            }
        }
        index.sort_by_key(|(n, _)| *n);
        index.into_iter().map(|(_, k)| k).chain(rest).collect()
    }
}

fn canonical_index(k: &str) -> Option<u64> {
    let n: u64 = k.parse().ok()?;
    (n.to_string() == k && n < u64::from(u32::MAX)).then_some(n)
}

/// `decodeURIComponent` after `+` to a space, the text kept raw when it throws.
fn decode(s: &str) -> String {
    let s = s.replace('+', " ");
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'%' {
            out.push(b[i]);
            i += 1;
            continue;
        }
        let hex = |k: usize| b.get(k).and_then(|c| (*c as char).to_digit(16));
        let (Some(h), Some(l)) = (hex(i + 1), hex(i + 2)) else {
            return s;
        };
        out.push((h * 16 + l) as u8);
        i += 3;
    }
    String::from_utf8(out).unwrap_or(s)
}

fn arr_of(items: Vec<Q>) -> Q {
    Q::Arr(items.into_iter().map(Some).collect())
}

fn overflowed(items: Vec<Option<Q>>) -> Q {
    let max = items.len().saturating_sub(1);
    let mut o = Obj::default();
    for (i, v) in items.into_iter().enumerate() {
        if let Some(v) = v {
            o.set(&i.to_string(), v);
        }
    }
    o.overflow = Some(max);
    Q::Obj(o)
}

/// `utils.combine(a, b)` with `b` a string, as `parseValues` calls it for a repeated raw key.
fn combine(a: Q, b: Q) -> Q {
    if let Q::Obj(mut o) = a {
        let next = o.overflow.map_or(0, |m| m + 1);
        o.set(&next.to_string(), b);
        o.overflow = Some(next);
        return Q::Obj(o);
    }
    let mut items = match a {
        Q::Arr(v) => v,
        other => vec![Some(other)],
    };
    items.push(Some(b));
    if items.len() > LIMIT {
        overflowed(items)
    } else {
        Q::Arr(items)
    }
}

/// The values of each raw key, a repeated key combined (`parseValues`).
fn parse_values(query: &str, root: &str) -> Vec<(String, Q)> {
    let clean = query
        .replace("%5B", "[")
        .replace("%5b", "[")
        .replace("%5D", "]")
        .replace("%5d", "]");
    let mut flat: Vec<(String, Q)> = vec![];
    for part in clean.split('&').take(PARAMETERS) {
        let pos = match part.find("]=") {
            Some(p) => Some(p + 1),
            None => part.find('='),
        };
        let (key, val) = match pos {
            Some(p) => (decode(&part[..p]), decode(&part[p + 1..])),
            None => (decode(part), String::new()),
        };
        // A key whose first segment is the root, bare or bracketed (`[timeline]=x` is `timeline` to qs).
        let first = segments(&key).into_iter().next().unwrap_or_default();
        if first != root && first != format!("[{root}]") {
            continue;
        }
        let val = Q::Str(val);
        match flat.iter().position(|(k, _)| *k == key) {
            Some(i) => {
                let old = std::mem::replace(&mut flat[i].1, Q::Str(String::new()));
                flat[i].1 = combine(old, val);
            }
            None => flat.push((key, val)),
        }
    }
    flat
}

/// `splitKeyIntoSegments` with the depth of 5.
fn segments(key: &str) -> Vec<String> {
    let mut out = vec![];
    let first = key.find('[');
    let parent = first.map_or(key, |f| &key[..f]);
    if !parent.is_empty() {
        out.push(parent.to_owned());
    }
    let b = key.as_bytes();
    let mut open = first;
    let mut collected = 0;
    while let Some(o) = open {
        if collected >= DEPTH {
            break;
        }
        let (mut level, mut i, mut close) = (1, o + 1, None);
        while i < b.len() && close.is_none() {
            match b[i] {
                b'[' => level += 1,
                b']' => {
                    level -= 1;
                    if level == 0 {
                        close = Some(i);
                    }
                }
                _ => {}
            }
            i += 1;
        }
        let Some(c) = close else {
            out.push(format!("[{}]", &key[o..]));
            return out;
        };
        out.push(key[o..=c].to_owned());
        collected += 1;
        open = key[c + 1..].find('[').map(|n| c + 1 + n);
    }
    if let Some(o) = open {
        out.push(format!("[{}]", &key[o..]));
    }
    out
}

/// JavaScript's `parseInt(s, 10)` of a segment, when `String(index) === s`.
fn array_index(clean: &str) -> Option<usize> {
    let n: usize = clean.parse().ok()?;
    (n.to_string() == clean).then_some(n)
}

fn parse_object(chain: &[String], leaf: Q) -> Q {
    let mut leaf = leaf;
    for root in chain.iter().rev() {
        leaf = if root == "[]" {
            match leaf {
                Q::Obj(o) if o.overflow.is_some() => Q::Obj(o),
                Q::Arr(v) => spread(v),
                other => arr_of(vec![other]),
            }
        } else {
            let bracketed = root.starts_with('[') && root.ends_with(']') && root.len() >= 2;
            let clean = if bracketed {
                &root[1..root.len() - 1]
            } else {
                root.as_str()
            };
            let index = if bracketed { array_index(clean) } else { None };
            match index {
                Some(i) if i < LIMIT => {
                    let mut v: Vec<Option<Q>> = vec![None; i];
                    v.push(Some(leaf));
                    Q::Arr(v)
                }
                Some(i) => {
                    let mut o = Obj::default();
                    o.set(clean, leaf);
                    o.overflow = Some(i);
                    Q::Obj(o)
                }
                None => {
                    let mut o = Obj::default();
                    if clean != "__proto__" {
                        o.set(clean, leaf);
                    }
                    Q::Obj(o)
                }
            }
        };
    }
    leaf
}

fn spread(v: Vec<Option<Q>>) -> Q {
    if v.len() > LIMIT {
        overflowed(v)
    } else {
        Q::Arr(v)
    }
}

fn objectish(q: &Q) -> bool {
    matches!(q, Q::Arr(_) | Q::Obj(_))
}

fn falsy(q: &Q) -> bool {
    matches!(q, Q::Str(s) if s.is_empty())
}

fn array_to_object(v: &[Option<Q>]) -> Obj {
    let mut o = Obj::default();
    for (i, x) in v.iter().enumerate() {
        if let Some(x) = x {
            o.set(&i.to_string(), x.clone());
        }
    }
    o
}

/// `utils.merge(target, source)` with `strictMerge` on.
fn merge(target: Q, source: Q) -> Q {
    if falsy(&source) {
        return target;
    }
    if !objectish(&source) {
        return merge_string(target, source);
    }
    if !objectish(&target) {
        return merge_into_value(target, source);
    }
    match (target, source) {
        (Q::Arr(mut t), Q::Arr(s)) => {
            for (i, item) in s.into_iter().enumerate() {
                let Some(item) = item else { continue };
                match t.get(i).cloned().flatten() {
                    Some(have) if objectish(&have) && objectish(&item) => {
                        t[i] = Some(merge(have, item));
                    }
                    Some(_) => t.push(Some(item)),
                    None => {
                        if t.len() <= i {
                            t.resize(i + 1, None);
                        }
                        t[i] = Some(item);
                    }
                }
            }
            spread(t)
        }
        (target, source) => merge_objects(target, source),
    }
}

fn merge_string(target: Q, source: Q) -> Q {
    match target {
        Q::Arr(mut t) => {
            let next = t.len();
            t.push(Some(source));
            if next < LIMIT {
                return Q::Arr(t);
            }
            let mut o = array_to_object(&t);
            o.overflow = Some(next);
            Q::Obj(o)
        }
        Q::Obj(mut o) => match o.overflow {
            Some(m) => {
                o.set(&(m + 1).to_string(), source);
                o.overflow = Some(m + 1);
                Q::Obj(o)
            }
            None => arr_of(vec![Q::Obj(o), source]),
        },
        other => arr_of(vec![other, source]),
    }
}

fn merge_into_value(target: Q, source: Q) -> Q {
    if let Q::Obj(o) = &source
        && let Some(m) = o.overflow
    {
        let mut r = Obj::default();
        r.set("0", target);
        for k in o.keys() {
            if let Some(n) = canonical_index(k) {
                r.set(
                    &(n + 1).to_string(),
                    o.get(k).cloned().unwrap_or(Q::Str(String::new())),
                );
            }
        }
        r.overflow = Some(m + 1);
        return Q::Obj(r);
    }
    let mut combined = vec![Some(target)];
    match source {
        Q::Arr(v) => combined.extend(v),
        other => combined.push(Some(other)),
    }
    if combined.len() > LIMIT {
        overflowed(combined)
    } else {
        Q::Arr(combined)
    }
}

fn merge_objects(target: Q, source: Q) -> Q {
    let mut acc = match target {
        Q::Arr(v) => array_to_object(&v),
        Q::Obj(o) => o,
        other => return merge_into_value(other, source),
    };
    let (pairs, source_max) = match source {
        Q::Obj(o) => {
            let keys: Vec<String> = o.keys().into_iter().map(str::to_owned).collect();
            let pairs = keys
                .into_iter()
                .filter_map(|k| o.get(&k).cloned().map(|v| (k, v)))
                .collect::<Vec<_>>();
            (pairs, o.overflow)
        }
        Q::Arr(v) => (
            v.into_iter()
                .enumerate()
                .filter_map(|(i, x)| x.map(|x| (i.to_string(), x)))
                .collect(),
            None,
        ),
        Q::Str(_) => return Q::Obj(acc),
    };
    for (key, value) in pairs {
        let merged = match acc.get(&key).cloned() {
            Some(have) => merge(have, value),
            None => value,
        };
        acc.set(&key, merged);
        if source_max.is_some() && acc.overflow.is_none() {
            acc.overflow = source_max;
        }
        if let (Some(m), Some(n)) = (acc.overflow, canonical_index(&key))
            && n as usize > m
        {
            acc.overflow = Some(n as usize);
        }
    }
    Q::Obj(acc)
}

fn compact(q: Q) -> Q {
    match q {
        Q::Arr(v) => Q::Arr(v.into_iter().flatten().map(compact).map(Some).collect()),
        Q::Obj(mut o) => {
            for e in &mut o.entries {
                e.1 = compact(std::mem::replace(&mut e.1, Q::Str(String::new())));
            }
            Q::Obj(o)
        }
        s => s,
    }
}

/// `req.query[key]` of a query string (no leading `?`), or `None` when it is absent.
pub fn query_value(query: &str, key: &str) -> Option<Q> {
    let query = query.strip_prefix('?').unwrap_or(query);
    let mut top = Q::Obj(Obj::default());
    for (raw, value) in parse_values(query, key) {
        let chain = segments(&raw);
        let parsed = parse_object(&chain, value);
        top = merge(top, parsed);
    }
    match compact(top) {
        Q::Obj(o) => o.get(key).cloned(),
        _ => None,
    }
}

impl Q {
    /// `JSON.stringify` of the value.
    pub fn json(&self) -> String {
        match self {
            Q::Str(s) => serde_json::to_string(s).unwrap_or_default(),
            Q::Arr(v) => {
                let items: Vec<String> = v.iter().flatten().map(Q::json).collect();
                format!("[{}]", items.join(","))
            }
            Q::Obj(o) => {
                let items: Vec<String> = o
                    .keys()
                    .into_iter()
                    .filter_map(|k| {
                        let q = o.get(k)?;
                        Some(format!("{}:{}", serde_json::to_string(k).ok()?, q.json()))
                    })
                    .collect();
                format!("{{{}}}", items.join(","))
            }
        }
    }

    /// JavaScript's `String(value)`: an array reads as its elements joined by commas.
    pub fn string(&self) -> String {
        match self {
            Q::Str(s) => s.clone(),
            Q::Arr(v) => v
                .iter()
                .flatten()
                .map(Q::string)
                .collect::<Vec<_>>()
                .join(","),
            Q::Obj(_) => "[object Object]".to_owned(),
        }
    }
}
