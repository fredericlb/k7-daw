// abi.rs — the C ABI the browser calls: the JSON call, the curve's playhead and the buffers every call crosses by (apps/k7-daw-frontend/packages/song-schema/docs/wasmBoundary.md); the typed gesture calls are typed.rs.
// A reader would get wrong that JS never frees or holds a pointer across a call: any call may grow the memory.

use crate::curve_ops::Head;
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
    // A text an import crosses in, apart from INPUT, which `k7_call` borrows for the whole call.
    static FS_BUF: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
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

/// A buffer of `len` bytes the host fills before an import returns; never the call's input (it is borrowed).
#[unsafe(no_mangle)]
pub extern "C" fn k7_fs_buf(len: usize) -> *mut u8 {
    FS_BUF.with(|b| {
        let mut b = b.borrow_mut();
        b.resize(len, 0);
        b.as_mut_ptr()
    })
}

/// Why an import did not answer.
#[derive(Debug, PartialEq, Eq)]
pub enum FsErr {
    NotFound,
    /// The host's file system does not hold the path yet: the caller fetches it and runs the call again.
    Needs,
    Io(String),
}

/// Takes the buffer the host filled and clears it, mapping the import's status.
fn take(status: u32) -> Result<Vec<u8>, FsErr> {
    let bytes = FS_BUF.with(|b| std::mem::take(&mut *b.borrow_mut()));
    match status {
        0 => Ok(bytes),
        1 => Err(FsErr::NotFound),
        2 => Err(FsErr::Needs),
        _ => Err(FsErr::Io(String::from_utf8_lossy(&bytes).into_owned())),
    }
}

#[cfg(target_arch = "wasm32")]
mod host {
    #[link(wasm_import_module = "k7")]
    unsafe extern "C" {
        pub fn k7_fs_read(p: *const u8, n: usize) -> u32;
        pub fn k7_fs_stat(p: *const u8, n: usize) -> u32;
        pub fn k7_fs_list(p: *const u8, n: usize) -> u32;
        pub fn k7_fs_real_path(p: *const u8, n: usize) -> u32;
        pub fn k7_fs_write(p: *const u8, n: usize, b: *const u8, m: usize) -> u32;
        pub fn k7_fs_remove(p: *const u8, n: usize) -> u32;
        pub fn k7_fs_remove_tree(p: *const u8, n: usize) -> u32;
        pub fn k7_fs_mkdir(p: *const u8, n: usize) -> u32;
        pub fn k7_fs_rename(p: *const u8, n: usize, q: *const u8, m: usize) -> u32;
        pub fn k7_fs_copy(p: *const u8, n: usize, q: *const u8, m: usize) -> u32;
        pub fn k7_new_id() -> u32;
    }
}

/// Off WebAssembly there is no host: every import fails, and a native caller gives the crate its own `Fs`.
#[cfg(not(target_arch = "wasm32"))]
mod host {
    const NO_HOST: u32 = 3;
    pub unsafe fn k7_fs_read(_: *const u8, _: usize) -> u32 {
        NO_HOST
    }
    pub unsafe fn k7_fs_stat(_: *const u8, _: usize) -> u32 {
        NO_HOST
    }
    pub unsafe fn k7_fs_list(_: *const u8, _: usize) -> u32 {
        NO_HOST
    }
    pub unsafe fn k7_fs_real_path(_: *const u8, _: usize) -> u32 {
        NO_HOST
    }
    pub unsafe fn k7_fs_write(_: *const u8, _: usize, _: *const u8, _: usize) -> u32 {
        NO_HOST
    }
    pub unsafe fn k7_fs_remove(_: *const u8, _: usize) -> u32 {
        NO_HOST
    }
    pub unsafe fn k7_fs_remove_tree(_: *const u8, _: usize) -> u32 {
        NO_HOST
    }
    pub unsafe fn k7_fs_mkdir(_: *const u8, _: usize) -> u32 {
        NO_HOST
    }
    pub unsafe fn k7_fs_rename(_: *const u8, _: usize, _: *const u8, _: usize) -> u32 {
        NO_HOST
    }
    pub unsafe fn k7_fs_copy(_: *const u8, _: usize, _: *const u8, _: usize) -> u32 {
        NO_HOST
    }
    pub unsafe fn k7_new_id() -> u32 {
        NO_HOST
    }
}

macro_rules! one_path {
    ($(#[$m:meta])* $name:ident, $import:ident) => {
        $(#[$m])*
        pub fn $name(path: &str) -> Result<Vec<u8>, FsErr> {
            // SAFETY: the host reads `path` during the call and keeps no pointer.
            take(unsafe { host::$import(path.as_ptr(), path.len()) })
        }
    };
}

one_path!(
    /// The file's bytes.
    fs_read, k7_fs_read
);
one_path!(
    /// `{kind, len, mtimeNs, settled}` as JSON.
    fs_stat, k7_fs_stat
);
one_path!(
    /// `[{name, kind}]` as JSON.
    fs_list, k7_fs_list
);
one_path!(
    /// The real path as text.
    fs_real_path, k7_fs_real_path
);
one_path!(
    /// `rmSync(path, { force: true })`.
    fs_remove, k7_fs_remove
);
one_path!(
    /// `rmSync(path, { recursive: true, force: true })`.
    fs_remove_tree, k7_fs_remove_tree
);
one_path!(
    /// `mkdirSync(path, { recursive: true })`.
    fs_mkdir, k7_fs_mkdir
);

pub fn fs_write(path: &str, bytes: &[u8]) -> Result<Vec<u8>, FsErr> {
    // SAFETY: the host reads both slices during the call.
    take(unsafe { host::k7_fs_write(path.as_ptr(), path.len(), bytes.as_ptr(), bytes.len()) })
}

pub fn fs_rename(from: &str, to: &str) -> Result<Vec<u8>, FsErr> {
    // SAFETY: as `fs_write`.
    take(unsafe { host::k7_fs_rename(from.as_ptr(), from.len(), to.as_ptr(), to.len()) })
}

pub fn fs_copy(from: &str, to: &str) -> Result<Vec<u8>, FsErr> {
    // SAFETY: as `fs_write`.
    take(unsafe { host::k7_fs_copy(from.as_ptr(), from.len(), to.as_ptr(), to.len()) })
}

/// A fresh v4 UUID, from the host's random source.
pub fn new_id() -> String {
    // SAFETY: the import takes no pointer.
    let bytes = take(unsafe { host::k7_new_id() }).unwrap_or_default();
    String::from_utf8_lossy(&bytes).into_owned()
}
