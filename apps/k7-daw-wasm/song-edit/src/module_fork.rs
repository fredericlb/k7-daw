// module_fork.rs — taking ownership of a module the ENGINE merely reports: it has no folder, so it is materialised into the workspace's `_modules`.
// A reader would get wrong where the catalogue and the panel come from: the host reads them (the loader stays TypeScript), and hands them in.

use crate::chain_item_edit::{edit_chain_item, is_chain_project};
use crate::confine::Env;
use crate::error::{Res, fail, io_error};
use crate::fs::Fs;
use crate::graph_file::rename_node;
use crate::instance_name::name_part;
use crate::module_io::module_file_for;
use crate::paths::{basename, dirname, is_within, join, relative, resolve};
use crate::shared_folders::{SHARED_MODULES_PATH, module_library_dirs, module_namespace_of};
use crate::workspace_doc::{ws_text, ws_write_text};
use indexmap::IndexMap;
use k7x::edit::{Document, PathSeg, StringifyOptions, Value, WriteOptions, stringify};
use song::Cache;
use song::modules::LoadedModule;
use song::modules::types::{ENGINES_DIR, type_meta};

/// What a materialisation did: the shared ref, the local module, its file and the chains now naming it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForkedModule {
    pub reference: String,
    pub module: String,
    pub path: String,
    pub chains: Vec<String>,
}

/// A shared module file's ref, folder, leaf and type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedRef {
    pub reference: String,
    pub folder: String,
    pub leaf: String,
    pub kind: String,
}

/// The shared roots this project inherits, each under the namespace its LOCATION gives it.
#[must_use]
pub fn shared_roots(fs: &dyn Fs, env: &Env, dir: &str) -> Vec<(String, String)> {
    module_library_dirs(fs, env, dir)
        .into_iter()
        .map(|root| {
            let ns = module_namespace_of(fs, env, &root);
            (root, ns)
        })
        .collect()
}

/// The ref a shared module file answers to, keyed as the module tree is read.
#[must_use]
pub fn shared_module_ref(fs: &dyn Fs, env: &Env, dir: &str, file: &str) -> Option<SharedRef> {
    let abs = resolve(file);
    for (root, ns) in shared_roots(fs, env, dir) {
        if !is_within(&root, &abs) || is_within(&abs, &root) {
            continue;
        }
        let segs: Vec<String> = relative(&root, &abs).split('/').map(str::to_owned).collect();
        // An ENGINE's file is its module's path plus `engines/<name>/`.
        let engine = usize::from(segs.len() >= 5 && segs[segs.len() - 3] == ENGINES_DIR) * 2;
        if segs.len() - engine < 3 || segs.len() - engine > 4 || type_meta(&segs[0]).is_none() {
            return None;
        }
        let folder = dirname(&abs);
        return Some(SharedRef {
            reference: format!("@{ns}/{}/{}", segs[0], segs[1..segs.len() - 1].join("/")),
            leaf: basename(&folder).to_owned(),
            folder,
            kind: segs[0].clone(),
        });
    }
    None
}

/// Every chain of a graph's k7x text that reaches `module`, `out` counting as well as `use`.
#[must_use]
pub fn chains_using(text: &str, module: &str) -> Vec<String> {
    let Ok(segments) = song::routing::graph_file::decode_graph("graph", text) else {
        return vec![];
    };
    song::routing::derive_chains(&segments, &|_| false)
        .into_iter()
        .filter(|(name, c)| {
            name != module && (c.use_steps.iter().any(|s| s.module == module) || c.out.iter().any(|o| o.to == module))
        })
        .map(|(n, _)| n)
        .collect()
}

fn repoint_chain_items(fs: &mut dyn Fs, dir: &str, from: &str, to: &str) -> Res<Vec<String>> {
    let source: &dyn song::Source = fs;
    let cp =
        song::routing::read_chain_project(source, &mut Cache::new(), dir).map_err(|e| io_error(&e, "scandir", dir))?;
    let mut chains: Vec<String> = vec![];
    for n in cp.nodes.values() {
        if n.track.is_some()
            || !matches!(&n.value, song::routing::chain_file::Item::Node(m) if m.module.extends.as_deref() == Some(from))
        {
            continue;
        }
        let (name, chain) = (n.name.clone(), n.chain.clone());
        edit_chain_item(fs, dir, &name, |item| {
            item.set_in(
                &[PathSeg::Key("extends".to_owned())],
                &Value::Str(to.to_owned()),
                WriteOptions::default(),
            )?;
            Ok(())
        })?;
        if !chains.contains(&chain) {
            chains.push(chain);
        }
    }
    Ok(chains)
}

/// The panel of the engine module being given a folder, as `uiFileText` writes it under the header it is handed.
pub type Panel<'a> = &'a dyn Fn(&str) -> Res<String>;

/// Give a module the ENGINE (or an io role) merely reports a folder of its own.
///
/// # Errors
/// A ref the catalogue does not hold, a category that is no module type, or the file system's.
pub fn materialise_engine_module(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    reference: &str,
    catalogue: &IndexMap<String, LoadedModule>,
    panel: Option<Panel>,
) -> Res<ForkedModule> {
    let Some(found) = catalogue.get(reference) else {
        return fail(format!(
            "{reference} is not a module this machine's engine offers — nothing to give a folder to"
        ));
    };
    let kind = found.category.clone().unwrap_or_default();
    let Some(meta) = type_meta(&kind) else {
        return fail(format!("{reference}: unknown module category '{kind}'"));
    };
    let stem = name_part(&found.file);
    let type_dir = resolve(&format!("{dir}/{SHARED_MODULES_PATH}/{kind}"));
    let mut module = stem.clone();
    let mut n = 2;
    while fs.exists(&join(&[&type_dir, &module])) {
        module = format!("{stem}_{n}");
        n += 1;
    }
    let folder = join(&[&type_dir, &module]);
    fs.mkdir_all(&folder).map_err(|e| io_error(&e, "mkdir", &folder))?;
    let body = match &found.authored.plugin {
        Some(p) => Value::object(vec![(
            "plugin".to_owned(),
            Value::object(vec![
                ("type".to_owned(), Value::Str(p.plugin_type.clone().unwrap_or_default())),
                ("name".to_owned(), Value::Str(p.name.clone().unwrap_or_default())),
            ]),
        )]),
        None => {
            let io = found.authored.io.map_or(String::new(), |i| io_name(i).to_owned());
            Value::object(vec![("io".to_owned(), Value::Str(io))])
        }
    };
    let header = [format!(
        "{module}: {reference}, given a folder so it can carry a panel and presets."
    )];
    let text = stringify(
        &body,
        &StringifyOptions {
            header: &header,
            inline: None,
        },
    )?;
    ws_write_text(fs, &join(&[&folder, "module.k7x"]), &text)?;

    let local_ref = format!(
        "@workspace/{}",
        module_ref_of(&kind, &module, meta.kind == song::modules::ModuleKind::Io)
    );
    if let Some(make) = panel {
        write_panel(
            fs,
            env,
            dir,
            &local_ref,
            &make(&format!("panel for {local_ref} (was {reference})"))?,
        )?;
    }
    let graph = module_file_for(fs, dir, "", "graph");
    let mut chains = vec![];
    if is_chain_project(fs, dir) {
        chains = repoint_chain_items(fs, dir, reference, &local_ref)?;
    } else if let Some(g) = graph {
        let mut doc = Document::new(&ws_text(fs, &g.path)?);
        rename_node(&mut doc.at(&[]), reference, &local_ref)?;
        ws_write_text(fs, &g.path, doc.text())?;
        chains = chains_using(doc.text(), &local_ref);
    }
    Ok(ForkedModule {
        reference: reference.to_owned(),
        path: format!("{SHARED_MODULES_PATH}/{kind}/{module}/module.k7x"),
        module,
        chains,
    })
}

fn module_ref_of(kind: &str, name: &str, io: bool) -> String {
    if io { name.to_owned() } else { format!("{kind}/{name}") }
}

fn io_name(i: song::modules::IoKind) -> &'static str {
    use song::modules::IoKind::{Camera, File, In, Midi, Out, VideoOut};
    match i {
        Out => "out",
        In => "in",
        VideoOut => "video_out",
        File => "file",
        Camera => "camera",
        Midi => "midi",
    }
}

/// `writeUiFile`: the panel's text under the library whose namespace the ref names.
fn write_panel(fs: &mut dyn Fs, env: &Env, dir: &str, reference: &str, text: &str) -> Res<()> {
    let Some(ns) = reference.strip_prefix('@').and_then(|r| r.split('/').next()) else {
        return fail(format!(
            "{reference}: a module ref is always namespaced now — bare names resolve nothing"
        ));
    };
    let Some(root) = module_library_dirs(fs, env, dir)
        .into_iter()
        .find(|r| module_namespace_of(fs, env, r) == ns)
    else {
        return fail(format!(
            "{reference}: no module library this project can see declares '@{ns}', so there is no folder to draw it in"
        ));
    };
    let bare = reference.split('/').skip(1).collect::<Vec<_>>().join("/");
    let with_type = if bare.contains('/') { bare } else { format!("io/{bare}") };
    let path = join(&[&root, &with_type, "ui.k7x"]);
    let folder = dirname(&path);
    fs.mkdir_all(&folder).map_err(|e| io_error(&e, "mkdir", &folder))?;
    ws_write_text(fs, &path, text)?;
    Ok(())
}
