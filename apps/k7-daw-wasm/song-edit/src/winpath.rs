// winpath.rs — the host edge for Windows: real `C:\x` and `\\srv\share\x` paths to the posix form the crate computes on, and back.
// A reader would get wrong that nothing else here parses a backslash: past this edge a path is `/C:/x` or `/UNC:/srv/share/x`.

/// The posix prefix of a UNC share: `/UNC:/<server>/<share>/…`.
pub const UNC: &str = "UNC:";

/// Whether `s` is a drive letter with its colon, such as `C:`.
#[must_use]
pub fn is_drive(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

/// Whether an absolute posix-form path lies on a drive or a share, where names compare without case.
#[must_use]
pub fn is_win_form(p: &str) -> bool {
    let first = p.strip_prefix('/').and_then(|r| r.split('/').next()).unwrap_or("");
    is_drive(first) || first == UNC
}

fn unc_led(s: &str) -> bool {
    s.starts_with("\\\\") || s.starts_with("//")
}

fn split_all(s: &str) -> Vec<&str> {
    s.split(['/', '\\']).filter(|c| !c.is_empty()).collect()
}

/// A real Windows path in posix form (`C:\x` is `/C:/x`, `\\?\UNC\s\sh\x` is `/UNC:/s/sh/x`); None for a device or drive-relative path.
#[must_use]
pub fn to_posix(real: &str) -> Option<String> {
    let verbatim = ["\\\\?\\", "//?/"].iter().find_map(|p| real.strip_prefix(p));
    let rest = verbatim.unwrap_or(real);
    if verbatim.is_none() && (real.starts_with("\\\\.\\") || real.starts_with("//./")) {
        return None;
    }
    let segs = split_all(rest);
    let first = segs.first().copied().unwrap_or("");
    let body = if is_drive(first) {
        // `C:x` without a separator is relative to the drive's working directory.
        let sep = rest.as_bytes().get(2);
        if segs.len() > 1 && !matches!(sep, Some(b'/' | b'\\')) {
            return None;
        }
        segs
    } else if verbatim.is_some() && first.eq_ignore_ascii_case("UNC") && segs.len() >= 3 {
        let mut v = vec![UNC];
        v.extend(&segs[1..]);
        v
    } else if verbatim.is_none() && unc_led(real) && segs.len() >= 2 {
        let mut v = vec![UNC];
        v.extend(&segs);
        v
    } else if verbatim.is_none() && real.starts_with('/') {
        return Some(crate::paths::resolve(real));
    } else {
        return None;
    };
    Some(crate::paths::resolve(&format!("/{}", body.join("/"))))
}

/// The real path of a posix-form one: `/C:/x` is `C:\x`, `/UNC:/s/sh/x` is `\\s\sh\x`, anything else is returned as is.
#[must_use]
pub fn from_posix(p: &str) -> String {
    if !is_win_form(p) {
        return p.to_owned();
    }
    let segs = split_all(p);
    if segs[0] == UNC {
        return format!("\\\\{}", segs[1..].join("\\"));
    }
    if segs.len() == 1 {
        format!("{}\\", segs[0])
    } else {
        segs.join("\\")
    }
}

/// A path a song file names, in posix form: backslashes are separators, `C:\x` is absolute, `C:x` and a device path are refused.
#[must_use]
pub fn user_path(reference: &str) -> Option<String> {
    let b = reference.as_bytes();
    let drive_led = b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':';
    let unc_led = unc_led(reference);
    if drive_led || unc_led {
        return to_posix(reference);
    }
    Some(reference.replace('\\', "/"))
}
