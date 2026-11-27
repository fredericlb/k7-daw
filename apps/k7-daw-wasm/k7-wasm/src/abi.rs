// abi.rs — the C ABI the browser calls: the JSON call, the curve's playhead and the buffers every call crosses by (apps/k7-daw-frontend/packages/song-schema/docs/wasmBoundary.md); the typed gesture calls are typed.rs.
// A reader would get wrong that JS never frees or holds a pointer across a call: any call may grow the memory.
// The module imports nothing: a call reads only what its request carries.

use crate::ops::curve::Head;
use crate::ops::{OK, Session};
use std::cell::RefCell;

/// The playhead has no place: an envelope, a negative or non-finite beat.
pub const NONE: u32 = 2;
/// The handle names no open curve.
pub const UNKNOWN_HANDLE: u32 = 3;
/// How many numbers a typed call answers in the scalars.
pub const SCALARS_LEN: usize = 16;

thread_local! {
    static INPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static OUTPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static SCALARS: RefCell<[f64; SCALARS_LEN]> = const { RefCell::new([0.0; SCALARS_LEN]) };
    // The numbers a typed call reads in, and the list of numbers it answers with when a few scalars are not enough.
    static F64_IN: RefCell<Vec<f64>> = const { RefCell::new(Vec::new()) };
    static F64_OUT: RefCell<Vec<f64>> = const { RefCell::new(Vec::new()) };
    static SESSION: RefCell<Session> = RefCell::new(Session::new());
}

/// A buffer of `len` bytes to write the request into, valid until the next call.
#[unsafe(no_mangle)]
pub extern "C" fn k7_input(len: usize) -> *mut u8 {
    INPUT.with(|i| {
        let mut i = i.borrow_mut();
        i.resize(len, 0);
        i.as_mut_ptr()
    })
}

/// Runs the request in the first `len` bytes of the input: `0` and the answer's JSON, or `1` and a `WasmError`'s.
#[unsafe(no_mangle)]
pub extern "C" fn k7_call(len: usize) -> u32 {
    INPUT.with(|i| {
        let input = i.borrow();
        let bytes = input.get(..len).unwrap_or(&[]);
        OUTPUT.with(|o| SESSION.with(|s| s.borrow_mut().handle(bytes, &mut o.borrow_mut())))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_output_ptr() -> *const u8 {
    OUTPUT.with(|o| o.borrow().as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_output_len() -> usize {
    OUTPUT.with(|o| o.borrow().len())
}

/// The playhead of an open curve at song beat `beat`: `0` and x, y in the scalars, `2` for none, `3` for an unknown handle.
#[unsafe(no_mangle)]
pub extern "C" fn k7_curve_playhead(handle: u32, beat: f64) -> u32 {
    let head = SESSION.with(|s| s.borrow_mut().curves.playhead(handle, beat));
    match head {
        Head::At(p) => {
            set_scalars(&[p.x, p.y]);
            OK
        }
        Head::Nothing => NONE,
        Head::Unknown => UNKNOWN_HANDLE,
    }
}

/// A fixed `[f64; 16]`: the numeric answer of a hot call.
#[unsafe(no_mangle)]
pub extern "C" fn k7_scalars_ptr() -> *const f64 {
    SCALARS.with(|s| s.borrow().as_ptr())
}

/// A buffer of `len` numbers to write a typed call's array arguments into, valid until the next call.
#[unsafe(no_mangle)]
pub extern "C" fn k7_f64_in(len: usize) -> *mut f64 {
    F64_IN.with(|b| {
        let mut b = b.borrow_mut();
        b.resize(len, 0.0);
        b.as_mut_ptr()
    })
}

/// The list of numbers the last typed call answered with, and how many.
#[unsafe(no_mangle)]
pub extern "C" fn k7_f64_out_ptr() -> *const f64 {
    F64_OUT.with(|b| b.borrow().as_ptr())
}

/// Runs `f` over the first `n` numbers the host wrote with `k7_f64_in`; fewer were written when `n` is larger.
pub fn with_f64_in<R>(n: usize, f: impl FnOnce(&[f64]) -> R) -> R {
    F64_IN.with(|b| {
        let b = b.borrow();
        f(b.get(..n).unwrap_or(&b))
    })
}

/// Answers a typed call with up to `SCALARS_LEN` numbers.
pub fn set_scalars(values: &[f64]) {
    SCALARS.with(|s| {
        let mut s = s.borrow_mut();
        for (slot, v) in s.iter_mut().zip(values) {
            *slot = *v;
        }
    });
}

/// Answers a typed call with a list of numbers, and its length.
pub fn set_f64_out(values: Vec<f64>) -> usize {
    let n = values.len();
    F64_OUT.with(|b| *b.borrow_mut() = values);
    n
}

/// Answers a typed call with a text, in the output buffer the JSON call also answers in; its byte length.
pub fn set_text(text: &str) -> usize {
    OUTPUT.with(|o| {
        let mut o = o.borrow_mut();
        o.clear();
        o.extend_from_slice(text.as_bytes());
        o.len()
    })
}

/// A beat rounded to six decimals: a number in, a number out, called as often as a drag moves.
#[unsafe(no_mangle)]
pub extern "C" fn k7_round_beat(beats: f64) -> f64 {
    song_schema::round_beat(beats)
}

/// The layout of the last `k7x.outline`, as u32 cells (k7x's outline.rs); valid until the next call.
#[unsafe(no_mangle)]
pub extern "C" fn k7_cells_ptr() -> *const u32 {
    SESSION.with(|s| s.borrow().k7x.cells.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn k7_cells_len() -> usize {
    SESSION.with(|s| s.borrow().k7x.cells.len())
}
