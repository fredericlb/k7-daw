// abi.rs — the C ABI the browser calls: the ONLY file with `extern "C"`, and the buffers it owns (apps/k7-daw-frontend/packages/song-schema/docs/wasmBoundary.md).
// A reader would get wrong that JS never frees or holds a pointer across a call: any call may grow the memory.
// The module imports nothing: a call reads only what its request carries.

use crate::ops::curve::Head;
use crate::ops::{OK, Session};
use std::cell::RefCell;

/// The playhead has no place: an envelope, a negative or non-finite beat.
pub const NONE: u32 = 2;
/// The handle names no open curve.
pub const UNKNOWN_HANDLE: u32 = 3;

thread_local! {
    static INPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static OUTPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static SCALARS: RefCell<[f64; 4]> = const { RefCell::new([0.0; 4]) };
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
            SCALARS.with(|s| *s.borrow_mut() = [p.x, p.y, 0.0, 0.0]);
            OK
        }
        Head::Nothing => NONE,
        Head::Unknown => UNKNOWN_HANDLE,
    }
}

/// A fixed `[f64; 4]`: the numeric answer of a hot call.
#[unsafe(no_mangle)]
pub extern "C" fn k7_scalars_ptr() -> *const f64 {
    SCALARS.with(|s| s.borrow().as_ptr())
}

/// A beat rounded to six decimals: a number in, a number out, called as often as a drag moves.
#[unsafe(no_mangle)]
pub extern "C" fn k7_round_beat(beats: f64) -> f64 {
    song_schema::round_beat(beats)
}
