// instance_name.rs — how a thing this project makes is named after what it came from: `<owner>_<what>_<n>`; the port of instanceName.ts.
// A reader would get wrong that the number is ALWAYS there: the first free name is `_1`, never the bare stem.

/// One segment the module vocabulary accepts: runs of anything else become `_`, the ends lose theirs, nothing left is `x`.
#[must_use]
pub fn name_part(s: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            if gap && !out.is_empty() {
                out.push('_');
            }
            gap = false;
            out.push(c);
        } else {
            gap = true;
        }
    }
    let t = out.trim_matches('_');
    if t.is_empty() { "x".to_owned() } else { t.to_owned() }
}

/// The first free `<owner>_<what>_<n>`, counting from 1.
#[must_use]
pub fn instance_name(what: &str, owner: Option<&str>, taken: &dyn Fn(&str) -> bool) -> String {
    let stem = match owner {
        Some(o) if !o.is_empty() => format!("{}_{}", name_part(o), name_part(what)),
        _ => name_part(what),
    };
    let mut n = 1;
    while taken(&format!("{stem}_{n}")) {
        n += 1;
    }
    format!("{stem}_{n}")
}
