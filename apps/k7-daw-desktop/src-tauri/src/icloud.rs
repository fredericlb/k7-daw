// icloud.rs — the Mac app's iCloud: NSUbiquitousKeyValueStore as the front's roaming store, and the keychain switch,
// both only when the bundle embeds its provisioning profile (docs/icloud.md). What a reader would get wrong: the
// store is reached through the Objective-C runtime's C calls, no binding crate, so each message names its exact type.

use std::ffi::{c_char, c_void, CStr, CString};
use std::sync::Arc;

type Id = *mut c_void;
type Sel = *mut c_void;

#[link(name = "objc")]
extern "C" {
    fn objc_getClass(name: *const c_char) -> Id;
    fn sel_registerName(name: *const c_char) -> Sel;
    fn objc_msgSend();
    fn objc_autoreleasePoolPush() -> *mut c_void;
    fn objc_autoreleasePoolPop(pool: *mut c_void);
}

#[link(name = "Foundation", kind = "framework")]
extern "C" {}

/// `objc_msgSend` cast to one message's real signature, as arm64 requires.
macro_rules! send {
    ($obj:expr, $sel:literal $(, $arg:expr => $ty:ty)* ; $ret:ty) => {{
        let f: unsafe extern "C" fn(Id, Sel $(, $ty)*) -> $ret =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f($obj, sel_registerName($sel.as_ptr()) $(, $arg)*)
    }};
}

/// Whether the bundle carries `Contents/embedded.provisionprofile`, which the build adds with the entitlements.
pub fn entitled() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.parent()?.join("embedded.provisionprofile")))
        .is_some_and(|p| p.is_file())
}

struct Kvs;

/// The default store; a pool around each use, since these calls hand back autoreleased objects.
unsafe fn store() -> Id {
    send!(objc_getClass(c"NSUbiquitousKeyValueStore".as_ptr()), c"defaultStore"; Id)
}

unsafe fn ns_string(s: &CStr) -> Id {
    send!(objc_getClass(c"NSString".as_ptr()), c"stringWithUTF8String:", s.as_ptr() => *const c_char; Id)
}

unsafe fn text_of(s: Id) -> Option<String> {
    if s.is_null() {
        return None;
    }
    let p: *const c_char = send!(s, c"UTF8String"; *const c_char);
    (!p.is_null()).then(|| CStr::from_ptr(p).to_string_lossy().into_owned())
}

fn ours(key: &str) -> bool {
    k7_front::embedded::KEY_PREFIXES
        .iter()
        .any(|p| key.starts_with(p))
}

impl k7_front::embedded::ListStore for Kvs {
    fn pairs(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        // SAFETY: every message below is sent with the signature Foundation declares for it, inside a pool.
        unsafe {
            let pool = objc_autoreleasePoolPush();
            let dict: Id = send!(store(), c"dictionaryRepresentation"; Id);
            let keys: Id = send!(dict, c"allKeys"; Id);
            let string_class = objc_getClass(c"NSString".as_ptr());
            let n: usize = send!(keys, c"count"; usize);
            for i in 0..n {
                let key: Id = send!(keys, c"objectAtIndex:", i => usize; Id);
                let Some(k) = text_of(key).filter(|k| ours(k)) else {
                    continue;
                };
                let value: Id = send!(dict, c"objectForKey:", key => Id; Id);
                let is_text: bool = send!(value, c"isKindOfClass:", string_class => Id; bool);
                if let Some(v) = text_of(value).filter(|_| is_text) {
                    out.push((k, v));
                }
            }
            objc_autoreleasePoolPop(pool);
        }
        out
    }

    fn put(&self, key: &str, value: Option<&str>) {
        let Ok(k) = CString::new(key) else { return };
        if !ours(key) {
            return;
        }
        let v = match value.map(CString::new) {
            Some(Ok(v)) => Some(v),
            Some(Err(_)) => return,
            None => None,
        };
        // SAFETY: as above; the C strings outlive the calls that copy them into NSStrings.
        unsafe {
            let pool = objc_autoreleasePoolPush();
            let s = store();
            let nk = ns_string(&k);
            match v {
                Some(v) => {
                    let nv = ns_string(&v);
                    send!(s, c"setString:forKey:", nv => Id, nk => Id; ())
                }
                None => send!(s, c"removeObjectForKey:", nk => Id; ()),
            }
            let _: bool = send!(s, c"synchronize"; bool);
            objc_autoreleasePoolPop(pool);
        }
    }
}

/// Before the front starts: with the profile, the lists and the forge sign-ins sync; the log says which it is.
pub fn install() -> &'static str {
    if !entitled() {
        return "iCloud sync is off: this build carries no provisioning profile";
    }
    // SAFETY: as above.
    unsafe {
        let pool = objc_autoreleasePoolPush();
        let _: bool = send!(store(), c"synchronize"; bool);
        objc_autoreleasePoolPop(pool);
    }
    k7_front::embedded::use_icloud(Arc::new(Kvs));
    "iCloud sync is on: the workspace list, the Forgejo servers and the forge sign-ins"
}
