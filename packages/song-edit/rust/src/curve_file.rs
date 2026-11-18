// curve_file.rs — editing a curve file in place: only the positioned half, time spliced across a drawn curve's points.
// A reader would get wrong which curves move: a `custom` and an `onoff` one; an LFO is periodic and an ADSR is fired by note-ons.

use crate::error::Res;
use crate::start_time::{js_string, position, to_position};
use crate::time_splice::{Mode, SplicedList, TimeSplice, splice_list};
use k7x::edit::{PathSeg, Scope, Value, WriteOptions};

struct Points<'a, 'd> {
    doc: &'a mut Scope<'d>,
    points: Vec<Value>,
    num: f64,
    den: f64,
}

fn point_path(i: usize, rest: &[&str]) -> Vec<PathSeg> {
    let mut p = vec![PathSeg::Key("points".to_owned()), PathSeg::Index(i)];
    p.extend(rest.iter().map(|k| PathSeg::Key((*k).to_owned())));
    p
}

impl SplicedList for Points<'_, '_> {
    fn count(&self) -> usize {
        self.points.len()
    }

    fn position_of(&mut self, i: usize) -> Option<f64> {
        let p = self.points.get(i).filter(|v| matches!(v, Value::Map(_)))?;
        let time = match p.get("time") {
            None | Some(Value::Null) => String::new(),
            Some(t) => js_string(t),
        };
        position(&time, self.num, self.den).ok()
    }

    fn remove(&mut self, i: usize) -> Res<()> {
        self.doc.delete_in(&point_path(i, &[]), false)?;
        Ok(())
    }

    fn shift(&mut self, i: usize, to: f64) -> Res<()> {
        let tc = to_position(to, self.num, self.den)?;
        Ok(self
            .doc
            .set_in(&point_path(i, &["time"]), &Value::Str(tc), WriteOptions::default())?)
    }
}

/// Inserts or deletes time in one curve file, by each point's own position.
///
/// # Errors
/// A time that does not write back, or the write's.
pub fn splice_curve_time(
    doc: &mut Scope,
    at_beats: f64,
    len_beats: f64,
    mode: Mode,
    num: f64,
    den: f64,
) -> Res<TimeSplice> {
    let mut out = TimeSplice::default();
    let Some(raw @ Value::Map(_)) = doc.to_js()? else {
        return Ok(out);
    };
    let drawn = matches!(raw.get("type"), Some(Value::Str(t)) if t == "custom" || t == "onoff");
    if !drawn {
        return Ok(out);
    }
    let points = match raw.get("points") {
        Some(Value::List(l)) => l.clone(),
        _ => vec![],
    };
    let mut list = Points { doc, points, num, den };
    splice_list(&mut list, at_beats, len_beats, mode, &mut out)?;
    Ok(out)
}
