// notes.rs — a sequence's `notes("c4 d4 ~ e4")` short form expanded to notes: song-schema's sugar.ts mini-notation.
// A reader would get wrong the positions: the TypeScript counts UTF-16 units, so the parser walks them and a message names one.

use crate::error::{Res, fail};
use k7x::edit::Value;

struct Note {
    t: f64,
    d: f64,
    k: f64,
}

enum Step {
    Rest,
    Key(f64),
    Seq(Vec<Vec<Weighted>>),
}

struct Weighted {
    step: usize,
    weight: f64,
}

struct Parser<'a> {
    src: &'a [u16],
    i: usize,
    steps: Vec<Step>,
}

fn text(u: &[u16]) -> String {
    String::from_utf16_lossy(u)
}

fn is_word(c: u16) -> bool {
    c < 128 && ((c as u8).is_ascii_alphanumeric() || c == b'#' as u16 || c == b'-' as u16)
}

/// A key as written: a name with its octave (C4 = 60), or a MIDI number.
fn key_of(word: &str) -> Res<f64> {
    if !word.is_empty() && word.bytes().all(|b| b.is_ascii_digit()) {
        return Ok(word.parse::<f64>().unwrap_or(f64::NAN));
    }
    let b = word.as_bytes();
    let pitch = b.first().and_then(|c| match c.to_ascii_lowercase() {
        b'c' => Some(0.0),
        b'd' => Some(2.0),
        b'e' => Some(4.0),
        b'f' => Some(5.0),
        b'g' => Some(7.0),
        b'a' => Some(9.0),
        b'b' => Some(11.0),
        _ => None,
    });
    let (accidental, rest) = match b.get(1) {
        Some(b'#') => (1.0, &word[2..]),
        Some(b'b') => (-1.0, &word[2..]),
        _ => (0.0, word.get(1..).unwrap_or("")),
    };
    let digits = rest.strip_prefix('-').unwrap_or(rest);
    let ok = !digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit());
    let (Some(p), true) = (pitch, ok) else {
        return fail(format!(
            "notes: \"{word}\" is neither a note name (c4, f#3) nor a MIDI number"
        ));
    };
    let k = p + accidental + (rest.parse::<f64>().unwrap_or(f64::NAN) + 1.0) * 12.0;
    if !(0.0..=127.0).contains(&k) {
        return fail(format!("notes: \"{word}\" is outside the MIDI keys"));
    }
    Ok(k)
}

impl Parser<'_> {
    fn err<T>(&self, msg: &str) -> Res<T> {
        fail(format!(
            "notes: {msg} at character {} of \"{}\"",
            self.i + 1,
            text(self.src)
        ))
    }

    fn layers(&mut self, close: Option<u16>) -> Res<Vec<Vec<Weighted>>> {
        let mut out: Vec<Vec<Weighted>> = vec![vec![]];
        loop {
            while self.src.get(self.i) == Some(&32) {
                self.i += 1;
            }
            let Some(&c) = self.src.get(self.i) else {
                return match close {
                    Some(q) => self.err(&format!("\"{}\" missing", text(&[q]))),
                    None => Ok(out),
                };
            };
            if Some(c) == close {
                self.i += 1;
                return Ok(out);
            }
            if c == b',' as u16 {
                self.i += 1;
                out.push(vec![]);
                continue;
            }
            let step = if c == b'[' as u16 {
                self.i += 1;
                let inner = self.layers(Some(b']' as u16))?;
                Step::Seq(inner)
            } else if c == b'~' as u16 {
                self.i += 1;
                Step::Rest
            } else {
                let n = self.src[self.i..].iter().take_while(|u| is_word(**u)).count();
                if n == 0 {
                    return self.err(&format!("unexpected \"{}\"", text(&[c])));
                }
                let word = text(&self.src[self.i..self.i + n]);
                self.i += n;
                Step::Key(key_of(&word)?)
            };
            let (mut weight, mut times) = (1.0, 1.0);
            while let Some((marker, value, used)) = self.modifier() {
                self.i += used;
                if marker == b'@' {
                    weight = value;
                } else {
                    times = value;
                }
            }
            self.steps.push(step);
            let id = self.steps.len() - 1;
            let mut n = 0.0;
            while n < times {
                if let Some(last) = out.last_mut() {
                    last.push(Weighted { step: id, weight });
                }
                n += 1.0;
            }
        }
    }

    /// `@n` or `!n` at the cursor: the marker, its number and the units it took.
    fn modifier(&self) -> Option<(u8, f64, usize)> {
        let rest = self.src.get(self.i..)?;
        let m = u8::try_from(*rest.first()?).ok().filter(|m| *m == b'@' || *m == b'!')?;
        let int = rest[1..]
            .iter()
            .take_while(|u| (b'0' as u16..=b'9' as u16).contains(u))
            .count();
        if int == 0 {
            return None;
        }
        let mut used = 1 + int;
        if rest.get(used) == Some(&(b'.' as u16)) {
            let frac = rest[used + 1..]
                .iter()
                .take_while(|u| (b'0' as u16..=b'9' as u16).contains(u))
                .count();
            if frac > 0 {
                used += 1 + frac;
            }
        }
        let n = text(&rest[1..used]).parse::<f64>().unwrap_or(f64::NAN);
        Some((m, n, used))
    }
}

/// JS `Math.round(x * 1e9) / 1e9`.
fn exact(x: f64) -> f64 {
    let y = x * 1e9;
    let f = y.floor();
    (if y - f >= 0.5 { f + 1.0 } else { f }) / 1e9
}

fn lay(p: &Parser, steps: &[Weighted], start: f64, span: f64, out: &mut Vec<Note>) {
    let total = steps.iter().fold(0.0, |n, s| n + s.weight);
    let mut at = start;
    for w in steps {
        let len = (span * w.weight) / total;
        match &p.steps[w.step] {
            Step::Key(k) => out.push(Note {
                t: exact(at),
                d: exact(len),
                k: *k,
            }),
            Step::Seq(layers) => {
                for layer in layers {
                    lay(p, layer, at, len, out);
                }
            }
            Step::Rest => {}
        }
        at += len;
    }
}

/// A sequence's notes from the short form: one step a beat, or `step` beats; `v` the velocity of every note.
///
/// # Errors
/// The sentence the TypeScript throws for a word that is not a note, a missing bracket or a stray character.
pub fn expand_notes(src: &str, step: f64, v: Option<f64>) -> Res<Vec<Value>> {
    let units: Vec<u16> = src.encode_utf16().collect();
    let mut p = Parser {
        src: &units,
        i: 0,
        steps: vec![],
    };
    let layers = p.layers(None)?;
    let mut out = Vec::new();
    for layer in &layers {
        let total = layer.iter().fold(0.0, |n, s| n + s.weight);
        lay(&p, layer, 0.0, total * step, &mut out);
    }
    out.sort_by(|a, b| {
        let d = a.t - b.t;
        if d != 0.0 {
            d.partial_cmp(&0.0).unwrap_or(std::cmp::Ordering::Equal)
        } else {
            (a.k - b.k).partial_cmp(&0.0).unwrap_or(std::cmp::Ordering::Equal)
        }
    });
    let with_v = v.filter(|v| *v != 1.0);
    Ok(out
        .into_iter()
        .map(|n| {
            let mut m = vec![
                ("t".to_owned(), Value::Num(n.t)),
                ("d".to_owned(), Value::Num(n.d)),
                ("k".to_owned(), Value::Num(n.k)),
            ];
            if let Some(v) = with_v {
                m.push(("v".to_owned(), Value::Num(v)));
            }
            Value::Map(m)
        })
        .collect())
}
