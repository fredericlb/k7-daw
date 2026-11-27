// picture_animate.rs — a shot's `animate:` as keys: read, cut in two at a split, shifted by a head trim, one key set or removed.
// A reader would get wrong that a lone key at 0 stays a key: it is written back as the constant it means.

use crate::error::Res;
use crate::picture_file::{events_of, has_index, key, ms};
use crate::start_time::js_round;
use k7x::edit::{PathSeg, Scope, Value, WriteOptions};

/// What a shot may animate, in the schema's own order.
pub const ANIMATED_PROPS: [&str; 5] = ["x", "y", "scale", "opacity", "rotate"];

/// How a segment travels from its key to the next.
pub const EASES: [&str; 5] = ["linear", "in", "out", "inout", "hold"];

/// One key of an animated property: seconds from the shot's start, a value, and how it leaves.
#[derive(Clone, Debug, PartialEq)]
pub struct Key {
    pub t: f64,
    pub v: f64,
    pub ease: String,
}

/// A property as written: a constant, or keys.
#[derive(Clone, Debug, PartialEq)]
pub enum Animated {
    Const(f64),
    Keys(Vec<Key>),
}

pub type Animate = Vec<(String, Animated)>;

fn parse_key(v: &Value) -> Option<Key> {
    match v {
        Value::List(p) => match p.as_slice() {
            [Value::Num(t), Value::Num(v)] => Some(Key {
                t: *t,
                v: *v,
                ease: "linear".to_owned(),
            }),
            _ => None,
        },
        Value::Map(m) => {
            let (mut t, mut val, mut ease) = (None, None, "linear".to_owned());
            for (k, x) in m {
                match (k.as_str(), x) {
                    ("t", Value::Num(n)) => t = Some(*n),
                    ("v", Value::Num(n)) => val = Some(*n),
                    ("ease", Value::Str(s)) if EASES.contains(&s.as_str()) => ease = s.clone(),
                    _ => return None,
                }
            }
            Some(Key { t: t?, v: val?, ease })
        }
        _ => None,
    }
}

fn parse_animated(v: &Value) -> Option<Animated> {
    match v {
        Value::Num(n) => Some(Animated::Const(*n)),
        Value::List(l) if !l.is_empty() => l.iter().map(parse_key).collect::<Option<Vec<_>>>().map(Animated::Keys),
        _ => None,
    }
}

/// A placement's `animate:` as the schema reads it, in the schema's property order; None when absent or not valid.
pub fn parse_animate(v: Option<&Value>) -> Option<Animate> {
    let Some(Value::Map(m)) = v else {
        return None;
    };
    if m.iter().any(|(k, _)| !ANIMATED_PROPS.contains(&k.as_str())) {
        return None;
    }
    let mut out = Vec::new();
    for p in ANIMATED_PROPS {
        match m.iter().find(|(k, _)| k == p) {
            None => {}
            Some((_, Value::Null)) => {}
            Some((_, x)) => out.push((p.to_owned(), parse_animated(x)?)),
        }
    }
    Some(out)
}

/// The keys of one property, sorted by time; a constant is one key.
pub fn keys_of(a: &Animated) -> Vec<Key> {
    match a {
        Animated::Const(v) => vec![Key {
            t: 0.0,
            v: *v,
            ease: "linear".to_owned(),
        }],
        Animated::Keys(k) => {
            let mut k = k.clone();
            k.sort_by(|p, q| p.t.partial_cmp(&q.t).unwrap_or(std::cmp::Ordering::Equal));
            k
        }
    }
}

fn written(keys: &[Key]) -> Value {
    Value::List(
        keys.iter()
            .map(|k| {
                if k.ease == "linear" {
                    Value::List(vec![Value::Num(k.t), Value::Num(k.v)])
                } else {
                    Value::Map(vec![
                        ("t".to_owned(), Value::Num(k.t)),
                        ("v".to_owned(), Value::Num(k.v)),
                        ("ease".to_owned(), Value::Str(k.ease.clone())),
                    ])
                }
            })
            .collect(),
    )
}

pub fn animate_value(a: &Animate) -> Value {
    Value::Map(
        a.iter()
            .map(|(p, x)| {
                let v = match x {
                    Animated::Const(c) => Value::Num(*c),
                    Animated::Keys(k) => written(k),
                };
                (p.clone(), v)
            })
            .collect(),
    )
}

pub fn animate_path(index: usize) -> Vec<PathSeg> {
    vec![key("events"), PathSeg::Index(index), key("animate")]
}

/// The keys that still shape `[lo, hi]`: those inside it, plus the nearest one beyond each edge.
fn prune(keys: &[Key], lo: f64, hi: f64) -> Vec<Key> {
    let before = keys.iter().rev().find(|k| k.t < lo);
    let after = keys.iter().find(|k| k.t > hi);
    let mut out: Vec<Key> = before.cloned().into_iter().collect();
    out.extend(keys.iter().filter(|k| k.t >= lo && k.t <= hi).cloned());
    out.extend(after.cloned());
    out
}

/// `animate` cut at `at` seconds into the shot, `length` its whole span: the tail is the same keys shifted.
pub fn split_animate(animate: &Animate, at: f64, length: f64) -> (Animate, Animate) {
    let end = if length > 0.0 { length } else { f64::INFINITY };
    let (mut head, mut tail) = (Vec::new(), Vec::new());
    for p in ANIMATED_PROPS {
        let Some((_, a)) = animate.iter().find(|(k, _)| k == p) else {
            continue;
        };
        match a {
            Animated::Const(c) => {
                head.push((p.to_owned(), Animated::Const(*c)));
                tail.push((p.to_owned(), Animated::Const(*c)));
            }
            Animated::Keys(_) => {
                let keys = keys_of(a);
                head.push((p.to_owned(), Animated::Keys(prune(&keys, 0.0, at))));
                let shifted = prune(&keys, at, end)
                    .into_iter()
                    .map(|k| Key {
                        t: js_round((k.t - at) * 1000.0) / 1000.0,
                        ..k
                    })
                    .collect();
                tail.push((p.to_owned(), Animated::Keys(shifted)));
            }
        }
    }
    (head, tail)
}

/// Shifts the keys of the placement at `index` by `-delta` seconds, so a trimmed head keeps its animation; false when it has none.
pub fn shift_animation_at(sc: &mut Scope, index: usize, delta: f64) -> Res<bool> {
    let Some(events) = events_of(sc) else {
        return Ok(false);
    };
    let Some(shot) = events.get(index) else {
        return Ok(false);
    };
    let Some(animate) = parse_animate(shot.get("animate")) else {
        return Ok(false);
    };
    if delta == 0.0 {
        return Ok(false);
    }
    let shifted: Animate = animate
        .iter()
        .map(|(p, a)| {
            let moved = match a {
                Animated::Const(c) => Animated::Const(*c),
                Animated::Keys(_) => Animated::Keys(
                    keys_of(a)
                        .into_iter()
                        .map(|k| Key {
                            t: ms(k.t - delta),
                            ..k
                        })
                        .collect(),
                ),
            };
            (p.clone(), moved)
        })
        .collect();
    sc.set_in(
        &animate_path(index),
        &animate_value(&shifted),
        WriteOptions { inline: true },
    )?;
    Ok(true)
}

fn keys_at(sc: &Scope, index: usize) -> Vec<(String, Vec<Key>)> {
    let v = sc.get_in(&animate_path(index)).ok().flatten();
    parse_animate(v.as_ref())
        .unwrap_or_default()
        .iter()
        .map(|(p, a)| (p.clone(), keys_of(a)))
        .collect()
}

fn write_keys(sc: &mut Scope, index: usize, keys: &[(String, Vec<Key>)]) -> Res<()> {
    let out: Animate = keys
        .iter()
        .filter_map(|(p, ks)| {
            let only = ks.first()?;
            let a = if ks.len() == 1 && only.t == 0.0 && only.ease == "linear" {
                Animated::Const(only.v)
            } else {
                Animated::Keys(ks.clone())
            };
            Some((p.clone(), a))
        })
        .collect();
    if out.is_empty() {
        sc.delete_in(&animate_path(index), false)?;
    } else {
        sc.set_in(
            &animate_path(index),
            &animate_value(&out),
            WriteOptions { inline: true },
        )?;
    }
    Ok(())
}

/// Sets one key of the placement at `index`, `t` seconds into the SHOT, replacing one already there; false with no such placement.
pub fn set_key_at(sc: &mut Scope, index: usize, prop: &str, t: f64, v: f64, ease: &str) -> Res<bool> {
    if !has_index(&events_of(sc), index) {
        return Ok(false);
    }
    let mut keys = keys_at(sc, index);
    let at = ms(t);
    let was = keys.iter().position(|(p, _)| p == prop);
    let mut mine: Vec<Key> = match was {
        Some(i) => keys.remove(i).1,
        None => Vec::new(),
    };
    mine.retain(|k| (k.t - at).abs() > 5e-4);
    mine.push(Key {
        t: at,
        v,
        ease: ease.to_owned(),
    });
    mine.sort_by(|p, q| p.t.partial_cmp(&q.t).unwrap_or(std::cmp::Ordering::Equal));
    match was {
        Some(i) => keys.insert(i, (prop.to_owned(), mine)),
        None => keys.push((prop.to_owned(), mine)),
    }
    write_keys(sc, index, &keys)?;
    Ok(true)
}

/// Removes the key of `prop` at `t` seconds into the shot; false when there is none there.
pub fn remove_key_at(sc: &mut Scope, index: usize, prop: &str, t: f64) -> Res<bool> {
    if !has_index(&events_of(sc), index) {
        return Ok(false);
    }
    let mut keys = keys_at(sc, index);
    let Some(i) = keys.iter().position(|(p, _)| p == prop) else {
        return Ok(false);
    };
    let before = keys[i].1.len();
    keys[i].1.retain(|k| (k.t - ms(t)).abs() > 5e-4);
    if keys[i].1.len() == before {
        return Ok(false);
    }
    write_keys(sc, index, &keys)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(t: &[(f64, f64)]) -> Animated {
        Animated::Keys(
            t.iter()
                .map(|(t, v)| Key {
                    t: *t,
                    v: *v,
                    ease: "linear".to_owned(),
                })
                .collect(),
        )
    }

    #[test]
    fn an_animation_cut_in_two_plays_on_in_both_halves() {
        let a = vec![
            ("x".to_owned(), keys(&[(0.0, 0.0), (4.0, 40.0)])),
            ("scale".to_owned(), Animated::Const(2.0)),
        ];
        let (head, tail) = split_animate(&a, 1.0, 4.0);
        assert_eq!(head[0].1, keys(&[(0.0, 0.0), (4.0, 40.0)]));
        assert_eq!(tail[0].1, keys(&[(-1.0, 0.0), (3.0, 40.0)]));
        assert!(
            head.iter()
                .chain(&tail)
                .any(|(p, x)| p == "scale" && *x == Animated::Const(2.0))
        );
    }

    #[test]
    fn a_property_that_is_not_one_makes_the_whole_map_invalid() {
        let bad = Value::Map(vec![("zoom".to_owned(), Value::Num(1.0))]);
        assert_eq!(parse_animate(Some(&bad)), None);
    }
}
