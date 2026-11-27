// assembly_param.rs — a library assembly's promoted params written into its `assembly.k7x`: a knob on the box's face, or taken back off.
// A reader would get wrong that a file which does not read as a template is refused before any write: a bad file is never half-edited.

use crate::error::{Res, fail};
use k7x::edit::{PathSeg, Scope, Value, WriteOptions};
use song::assemblies::file::decode_assembly_template;

/// The library file an assembly is declared in, named in a refusal.
const ASSEMBLY_FILE: &str = "assembly.k7x";

fn key(k: &str) -> PathSeg {
    PathSeg::Key(k.to_owned())
}

/// Refuses early rather than write over a file that does not read as a template; an empty file reads as one.
fn check(sc: &mut Scope) -> Res<()> {
    let text = sc.document().text().to_owned();
    if matches!(sc.to_js()?, None | Some(Value::Null)) || matches!(sc.to_js()?, Some(Value::Map(m)) if m.is_empty()) {
        return Ok(());
    }
    match decode_assembly_template(ASSEMBLY_FILE, &text) {
        Ok(_) => Ok(()),
        Err(f) => fail(format!("{ASSEMBLY_FILE}: {}", f.message)),
    }
}

/// Promotes a member's param onto the box's face, or replaces what that name already promoted.
///
/// # Errors
/// A file that is not a template, or what the document's edit fails with.
pub fn set_param(sc: &mut Scope, name: &str, node: &str, param: &str) -> Res<()> {
    check(sc)?;
    let target = Value::Map(vec![
        ("node".to_owned(), Value::Str(node.to_owned())),
        ("param".to_owned(), Value::Str(param.to_owned())),
    ]);
    Ok(sc.set_in(&[key("params"), key(name)], &target, WriteOptions::default())?)
}

/// Takes a promoted param back off, the empty map going with the last entry; true when there was one to take.
///
/// # Errors
/// A file that is not a template, or what the document's edit fails with.
pub fn drop_param(sc: &mut Scope, name: &str) -> Res<bool> {
    check(sc)?;
    let at = [key("params"), key(name)];
    if !sc.has_in(&at)? {
        return Ok(false);
    }
    Ok(sc.delete_in(&at, true)?)
}
