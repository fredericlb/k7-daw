// files/notes.rs — a sequence's `notes` in its three spellings: a list, `notes("c4 d4 ~ e4")`, or `notes(seq("…").step(0.5).v(0.8))`.
// A reader would get wrong that a short form is expanded here, once, so everything after sees the list; after song-schema's sugar.ts.

use super::entity::Note;
use super::scalar::{MidiKey, NonNeg, Pos, Unit};
use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::rc::Rc;

/// A sequence's notes, whatever way they were written.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Notes(pub Vec<Note>);

/// How deep `[ ]` may nest and how many steps and notes a string may lay; far past any riff, short of a trap.
const MAX_DEPTH: usize = 64;
const MAX_STEPS: usize = 100_000;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SeqForm {
    seq: String,
    step: Option<Pos>,
    v: Option<Unit>,
}

impl<'de> Deserialize<'de> for Notes {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(NotesVisitor)
    }
}

struct NotesVisitor;

impl<'de> Visitor<'de> for NotesVisitor {
    type Value = Notes;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a list of notes, a string of steps, or seq(…)")
    }

    fn visit_str<E: de::Error>(self, s: &str) -> Result<Notes, E> {
        expand_notes(s, 1.0, None).map(Notes).map_err(E::custom)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Notes, A::Error> {
        let mut out = Vec::new();
        while let Some(n) = seq.next_element::<Note>()? {
            out.push(n);
        }
        Ok(Notes(out))
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Notes, A::Error> {
        let form = SeqForm::deserialize(de::value::MapAccessDeserializer::new(map))?;
        let step = form.step.map_or(1.0, |s| s.0);
        expand_notes(&form.seq, step, form.v.map(|v| v.0))
            .map(Notes)
            .map_err(de::Error::custom)
    }
}

enum Step {
    Rest,
    Key(u8),
    Seq(Vec<Vec<Weighted>>),
}

struct Weighted {
    step: Rc<Step>,
    weight: f64,
}

/// JS `Math.round`: halves go up.
fn js_round(x: f64) -> f64 {
    let r = x.floor();
    if x - r >= 0.5 { r + 1.0 } else { r }
}

fn exact(x: f64) -> f64 {
    js_round(x * 1e9) / 1e9
}

fn pitch(letter: char) -> Option<i32> {
    Some(match letter.to_ascii_lowercase() {
        'c' => 0,
        'd' => 2,
        'e' => 4,
        'f' => 5,
        'g' => 7,
        'a' => 9,
        'b' => 11,
        _ => return None,
    })
}

/// A key as written: a name with its octave (C4 = 60), or a MIDI number.
fn key_of(word: &str) -> Result<u8, String> {
    let outside = || format!("notes: \"{word}\" is outside the MIDI keys");
    if word.bytes().all(|b| b.is_ascii_digit()) {
        return word
            .parse::<u16>()
            .ok()
            .and_then(|n| u8::try_from(n).ok())
            .filter(|n| *n <= 127)
            .ok_or_else(outside);
    }
    let bad = || format!("notes: \"{word}\" is neither a note name (c4, f#3) nor a MIDI number");
    let mut chars = word.chars();
    let letter = chars.next().and_then(pitch).ok_or_else(bad)?;
    let rest = chars.as_str();
    let (accidental, octave) = match rest.strip_prefix('#') {
        Some(o) => (1, o),
        None => match rest.strip_prefix('b') {
            Some(o) => (-1, o),
            None => (0, rest),
        },
    };
    let digits = octave.strip_prefix('-').unwrap_or(octave);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad());
    }
    let octave: i32 = octave.parse().map_err(|_| outside())?;
    let key = octave
        .checked_add(1)
        .and_then(|o| o.checked_mul(12))
        .and_then(|o| o.checked_add(letter + accidental))
        .ok_or_else(outside)?;
    u8::try_from(key).ok().filter(|k| *k <= 127).ok_or_else(outside)
}

struct Parser {
    src: Vec<char>,
    i: usize,
    steps: usize,
}

impl Parser {
    fn fail<T>(&self, msg: &str) -> Result<T, String> {
        let text: String = self.src.iter().collect();
        Err(format!("notes: {msg} at character {} of \"{text}\"", self.i + 1))
    }

    fn layers(&mut self, close: Option<char>, depth: usize) -> Result<Vec<Vec<Weighted>>, String> {
        if depth > MAX_DEPTH {
            return self.fail("brackets nest too deep");
        }
        let mut out: Vec<Vec<Weighted>> = vec![Vec::new()];
        loop {
            while self.src.get(self.i) == Some(&' ') {
                self.i += 1;
            }
            let Some(&c) = self.src.get(self.i) else {
                if let Some(close) = close {
                    return self.fail(&format!("\"{close}\" missing"));
                }
                return Ok(out);
            };
            if Some(c) == close {
                self.i += 1;
                return Ok(out);
            }
            if c == ',' {
                self.i += 1;
                out.push(Vec::new());
                continue;
            }
            let step = if c == '[' {
                self.i += 1;
                Step::Seq(self.layers(Some(']'), depth + 1)?)
            } else if c == '~' {
                self.i += 1;
                Step::Rest
            } else {
                let start = self.i;
                while self
                    .src
                    .get(self.i)
                    .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '#' || *c == '-')
                {
                    self.i += 1;
                }
                if self.i == start {
                    return self.fail(&format!("unexpected \"{c}\""));
                }
                let word: String = self.src.get(start..self.i).unwrap_or_default().iter().collect();
                Step::Key(key_of(&word)?)
            };
            let (weight, times) = self.modifiers();
            let step = Rc::new(step);
            let mut n = 0.0;
            while n < times {
                self.steps += 1;
                if self.steps > MAX_STEPS {
                    return self.fail("too many steps");
                }
                if let Some(layer) = out.last_mut() {
                    layer.push(Weighted {
                        step: Rc::clone(&step),
                        weight,
                    });
                }
                n += 1.0;
            }
        }
    }

    /// The `@n` length and `!n` repeat that may follow a step.
    fn modifiers(&mut self) -> (f64, f64) {
        let (mut weight, mut times) = (1.0, 1.0);
        while let Some(&m) = self.src.get(self.i) {
            if m != '@' && m != '!' {
                break;
            }
            let mut j = self.i + 1;
            let digits = |j: &mut usize, src: &[char]| {
                let from = *j;
                while src.get(*j).is_some_and(char::is_ascii_digit) {
                    *j += 1;
                }
                *j > from
            };
            if !digits(&mut j, &self.src) {
                break;
            }
            if self.src.get(j) == Some(&'.') {
                let mut k = j + 1;
                if digits(&mut k, &self.src) {
                    j = k;
                }
            }
            let n: f64 = self
                .src
                .get(self.i + 1..j)
                .unwrap_or_default()
                .iter()
                .collect::<String>()
                .parse()
                .unwrap_or(1.0);
            if m == '@' {
                weight = n;
            } else {
                times = n;
            }
            self.i = j;
        }
        (weight, times)
    }
}

/// What `lay` has made and visited so far; the limits count OUTPUT, since `!n` shares one step n times.
struct Laid {
    notes: Vec<Note>,
    visits: usize,
}

/// Every note a sequence of weighted steps holds, laid over [start, start + span).
fn lay(steps: &[Weighted], start: f64, span: f64, laid: &mut Laid) -> Result<(), String> {
    let total: f64 = steps.iter().map(|s| s.weight).sum();
    let mut at = start;
    for Weighted { step, weight } in steps {
        laid.visits += 1;
        if laid.visits > MAX_STEPS || laid.notes.len() > MAX_STEPS {
            return Err("notes: the short form lays too many notes".to_owned());
        }
        let len = span * weight / total;
        match &**step {
            Step::Key(k) => laid.notes.push(Note {
                t: NonNeg(exact(at)),
                d: Pos(exact(len)),
                k: MidiKey(*k),
                v: None,
                repeat: None,
                skip: None,
            }),
            Step::Seq(layers) => {
                for layer in layers {
                    lay(layer, at, len, laid)?;
                }
            }
            Step::Rest => {}
        }
        at += len;
    }
    Ok(())
}

/// A sequence's notes from the short form: one step a beat, or `step` beats; `v` the velocity of every note.
pub fn expand_notes(src: &str, step: f64, v: Option<f64>) -> Result<Vec<Note>, String> {
    let mut parser = Parser {
        src: src.chars().collect(),
        i: 0,
        steps: 0,
    };
    let layers = parser.layers(None, 0)?;
    let mut laid = Laid {
        notes: Vec::new(),
        visits: 0,
    };
    for layer in &layers {
        let total: f64 = layer.iter().map(|s| s.weight).sum();
        lay(layer, 0.0, total * step, &mut laid)?;
    }
    let mut out = laid.notes;
    for n in &out {
        let (t, d) = (n.t.0, n.d.0);
        if !(t.is_finite() && t >= 0.0 && d.is_finite() && d > 0.0) {
            return Err("notes: a step has no length — check its @ weights".to_owned());
        }
    }
    out.sort_by(|a, b| {
        a.t.0
            .partial_cmp(&b.t.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.k.0.cmp(&b.k.0))
    });
    if let Some(v) = v.filter(|v| *v != 1.0) {
        for n in &mut out {
            n.v = Some(Unit(v));
        }
    }
    Ok(out)
}
