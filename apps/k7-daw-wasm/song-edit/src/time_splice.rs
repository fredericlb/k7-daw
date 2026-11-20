// time_splice.rs — inserting or deleting a stretch of time in one list of positioned items, whatever the unit and the file.
// A reader would get wrong the order: items go DESCENDING, so a removal never shifts the address of the next.

use crate::error::Res;
use crate::start_time::above;

/// What a splice did, for a caller reporting it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TimeSplice {
    pub moved: usize,
    pub removed: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Insert,
    Delete,
}

impl Mode {
    /// The mode a command spells, `insert` or `delete`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Mode> {
        match s {
            "insert" => Some(Mode::Insert),
            "delete" => Some(Mode::Delete),
            _ => None,
        }
    }
}

/// The list being spliced: its length, each item's position (None leaves it alone) and the two edits.
pub trait SplicedList {
    fn count(&self) -> usize;
    fn position_of(&mut self, i: usize) -> Option<f64>;
    fn remove(&mut self, i: usize) -> Res<()>;
    fn shift(&mut self, i: usize, to: f64) -> Res<()>;
}

/// Splices `len` at `at` into one list, adding what it did to `into`.
///
/// # Errors
/// What an edit of the list fails with.
pub fn splice_list(list: &mut dyn SplicedList, at: f64, len: f64, mode: Mode, into: &mut TimeSplice) -> Res<()> {
    if !above(len, 0.0) {
        return Ok(());
    }
    let end = at + len;
    for i in (0..list.count()).rev() {
        let Some(start) = list.position_of(i) else { continue };
        if mode == Mode::Delete && start >= at - 1e-6 && start < end - 1e-6 {
            list.remove(i)?;
            into.removed += 1;
            continue;
        }
        let from = if mode == Mode::Delete { end } else { at };
        if start < from - 1e-6 {
            continue;
        }
        list.shift(i, start + if mode == Mode::Delete { -len } else { len })?;
        into.moved += 1;
    }
    Ok(())
}
