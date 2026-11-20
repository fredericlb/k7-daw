//! plugin.rs — `resolvePlugin` of `pluginRef.ts`: the friendly name of a plugin turned into the pair the engine takes (2340).
//! What a reader gets wrong: a valid `ref` wins over `format` and `id`, and a hit with no `id` is no hit at all.

use song::modules::{Scan, engine_module_of};

/// The `(format, id)` pair, or the status and sentence to send.
pub fn resolve_plugin(
    scan: &Scan,
    r#ref: &str,
    format: &str,
    id: &str,
) -> Result<(String, String), (u16, String)> {
    if !r#ref.is_empty() {
        let hit = scan.plugins.iter().find(|p| {
            p.id.is_some()
                && engine_module_of(&p.name, &p.format)
                    .is_some_and(|(candidate, _)| candidate == r#ref)
        });
        return match hit.and_then(|p| p.id.as_ref().map(|id| (p.format.clone(), id.clone()))) {
            Some(pair) => Ok(pair),
            None => Err((404, format!("no scanned plugin for \"{}\"", r#ref))),
        };
    }
    if format.is_empty() || id.is_empty() {
        return Err((400, "format and id, or ref, are required".to_owned()));
    }
    Ok((format.to_owned(), id.to_owned()))
}
