// alternative_switch.rs — switching a module to another of its attribute sets, and saying whether a running engine can take it as values.
// A reader would get wrong when `whole` is true: a value came from what the module extends, so the resolved params are all given.

use crate::alternative_file::{params_switch, set_alternative};
use crate::confine::Env;
use crate::error::Res;
use crate::fs::Fs;
use crate::loader::resolve_node;
use crate::project_edit::edit_module_file;
use k7x::edit::Value;

/// The values a switch changed: a patch over the plugin's params, or the whole of them.
#[derive(Debug, PartialEq)]
pub struct SwitchedParams {
    pub params: Vec<(String, f64)>,
    pub whole: bool,
}

/// Switch a module's set by name; the values are present only when the switch changed nothing but numbers.
///
/// # Errors
/// A name no set answers to, or as `edit_module_file`.
pub fn select_module_alternative(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    module: &str,
    name: &str,
) -> Res<(String, Option<SwitchedParams>)> {
    let mut found = None;
    let path = edit_module_file(fs, env, dir, module, |doc| {
        let raw = doc.to_js()?.unwrap_or(Value::Map(vec![]));
        found = params_switch(&raw, name);
        set_alternative(doc, name)
    })?;
    let Some(sw) = found else { return Ok((path, None)) };
    if sw.inherited.is_empty() {
        return Ok((
            path,
            Some(SwitchedParams {
                params: sw.params,
                whole: false,
            }),
        ));
    }
    let Some(resolved) = resolve_node(fs, env, dir, module)? else {
        return Ok((path, None));
    };
    let params = resolved
        .authored
        .params
        .iter()
        .flatten()
        .filter_map(|(k, v)| match v {
            song::modules::ParamValue::Number(n) if n.is_finite() => Some((k.clone(), *n)),
            _ => None,
        })
        .collect();
    Ok((path, Some(SwitchedParams { params, whole: true })))
}
