//! ffi.rs — the store as the host's two C callbacks: Swift lists the pairs of NSUbiquitousKeyValueStore and puts one.
//! What a reader gets wrong: the callbacks are called from any thread and must not call back into the front.

use std::ffi::{CStr, CString, c_char, c_void};
use std::sync::Arc;

use crate::store::{ListStore, install};

/// Called once per pair by the host's `List`, with the `ctx` it was given.
pub type Emit = extern "C" fn(ctx: *mut c_void, key: *const c_char, value: *const c_char);
/// Lists every pair of the store through `emit`.
pub type List = extern "C" fn(emit: Emit, ctx: *mut c_void);
/// Puts a pair; a null value removes the key.
pub type Put = extern "C" fn(key: *const c_char, value: *const c_char);

pub struct Callbacks {
    pub list: List,
    pub put: Put,
}

extern "C" fn collect(ctx: *mut c_void, key: *const c_char, value: *const c_char) {
    if ctx.is_null() || key.is_null() || value.is_null() {
        return;
    }
    // SAFETY: `ctx` is the `Vec` `pairs` passed below, alive for the whole call, and both strings are NUL-terminated.
    let out = unsafe { &mut *ctx.cast::<Vec<(String, String)>>() };
    let (k, v) = unsafe { (CStr::from_ptr(key), CStr::from_ptr(value)) };
    if let (Ok(k), Ok(v)) = (k.to_str(), v.to_str()) {
        out.push((k.to_string(), v.to_string()));
    }
}

impl ListStore for Callbacks {
    fn pairs(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = Vec::new();
        (self.list)(collect, (&raw mut out).cast());
        out
    }

    fn put(&self, key: &str, value: Option<&str>) {
        let Ok(k) = CString::new(key) else { return };
        match value.map(CString::new) {
            Some(Ok(v)) => (self.put)(k.as_ptr(), v.as_ptr()),
            Some(Err(_)) => {}
            None => (self.put)(k.as_ptr(), std::ptr::null()),
        }
    }
}

/// Installs the host's callbacks as the store.
pub fn install_callbacks(list: List, put: Put) {
    install(Arc::new(Callbacks { list, put }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static CELL: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

    extern "C" fn list(emit: Emit, ctx: *mut c_void) {
        for (k, v) in CELL.lock().unwrap().iter() {
            let (k, v) = (
                CString::new(k.as_str()).unwrap(),
                CString::new(v.as_str()).unwrap(),
            );
            emit(ctx, k.as_ptr(), v.as_ptr());
        }
    }

    extern "C" fn put(key: *const c_char, value: *const c_char) {
        let k = unsafe { CStr::from_ptr(key) }.to_str().unwrap().to_string();
        let mut cell = CELL.lock().unwrap();
        cell.retain(|(x, _)| *x != k);
        if !value.is_null() {
            let v = unsafe { CStr::from_ptr(value) }
                .to_str()
                .unwrap()
                .to_string();
            cell.push((k, v));
        }
    }

    #[test]
    fn the_callbacks_are_a_store_the_list_rules_run_over() {
        let s = Callbacks { list, put };
        crate::list::upsert(
            &s,
            &crate::Entry {
                id: "a".into(),
                name: "Band".into(),
                remote: Some("git@h:o/r".into()),
                added_at: 1,
            },
        );
        assert_eq!(crate::list::live(&s).len(), 1);
        crate::list::forget(&s, "a", 2);
        assert!(crate::list::live(&s).is_empty());
        s.put("ws.a", None);
        assert!(s.pairs().is_empty());
    }
}
