// scaffold_needs.rs — which SHARED modules a scaffolded project will not be able to resolve, said when it is made.
// A reader would get wrong that anything is seeded: nothing is copied in, the owed refs are only read off the files and named.

use k7x::edit::{Value, parse};
use song::modules::Vocabulary;
use std::collections::BTreeSet;

/// The LIBRARIES namespace, and only it: the other four need no repository.
const LIBRARY: &str = "@k7-core/";

/// Every `extends` a set of scaffolded files names under the libraries, sorted; a file that is no k7x names none.
#[must_use]
pub fn needed_refs(files: &[(String, String)]) -> Vec<String> {
    let mut out = BTreeSet::new();
    for (_, text) in files {
        let Ok(Value::Map(m)) = parse(text) else { continue };
        if let Some((_, Value::Str(r))) = m.iter().find(|(k, _)| k == "extends")
            && r.starts_with(LIBRARY)
        {
            out.insert(r.clone());
        }
    }
    // JavaScript sorts by UTF-16 code unit; the refs are ASCII in practice, and the set keeps them in byte order.
    out.into_iter().collect()
}

/// Of those, the ones the workspace cannot resolve.
#[must_use]
pub fn missing_refs(needed: &[String], known: &Vocabulary) -> Vec<String> {
    needed
        .iter()
        .filter(|r| !known.modules.contains_key(*r))
        .cloned()
        .collect()
}
