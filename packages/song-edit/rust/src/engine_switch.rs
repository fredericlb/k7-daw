// engine_switch.rs — running a module as another of its engines, by writing `engine:` on the attribute set that is on.
// A reader would get wrong whose names these are: the RESOLVED module's, since a node inherits its engines from the module's folder.

use crate::alternative_file::set_alternative_field;
use crate::confine::Env;
use crate::error::{Res, fail};
use crate::fs::Fs;
use crate::loader::resolve_node;
use crate::project_edit::edit_module_file;
use song::resolve::{current_engine, engine_names};

/// What a module can run as, and which one it does: empty when it has no engines.
///
/// # Errors
/// A module the project does not have.
pub fn module_engines(fs: &dyn Fs, env: &Env, dir: &str, module: &str) -> Res<(Vec<String>, Option<String>)> {
    let Some(r) = resolve_node(fs, env, dir, module)? else {
        return fail(format!("no module named '{module}'"));
    };
    Ok((engine_names(&r), current_engine(&r)))
}

/// Switch a module to one of its engines; refused for a name it has not got.
///
/// # Errors
/// A module with no engines, or one that lacks `name`.
pub fn select_module_engine(fs: &mut dyn Fs, env: &Env, dir: &str, module: &str, name: &str) -> Res<String> {
    let (names, _) = module_engines(fs, env, dir, module)?;
    if names.is_empty() {
        return fail(format!("{module} has no engines"));
    }
    if !names.iter().any(|n| n == name) {
        return fail(format!("{module} has no engine '{name}' — it has {}", names.join(", ")));
    }
    edit_module_file(fs, env, dir, module, |doc| set_alternative_field(doc, "engine", name))
}
