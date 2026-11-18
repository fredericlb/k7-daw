// loader.rs — a project's modules as the loader reads them, over an `Fs`: the nodes, the vocabulary and one node resolved.
// A reader would get wrong that the libraries are unversioned here unless the host sets `Env::library_roots` to its pinned folders.

use crate::confine::{Env, library_read_roots};
use crate::error::{Error, Res};
use crate::fs::Fs;
use crate::paths::{basename, dirname, resolve};
use indexmap::IndexMap;
use song::modules::{LoadedModule, Vocabulary, load_nodes};
use song::resolve::{Resolved, resolve_map};
use song::{Cache, Library, Roots};
use std::sync::Arc;

pub const NODES_DIR: &str = "nodes";
const ACCOUNT_LIBRARY: &str = "account";

fn src(e: song::SourceError) -> Error {
    Error::Msg(e.to_string())
}

/// The places a project reads from: its workspace, the account's libraries nearest first, `@meta`.
#[must_use]
pub fn roots_for(fs: &dyn Fs, env: &Env, dir: &str) -> Roots {
    let project = resolve(dir);
    let account = crate::paths::join(&[&env.studio_home, "library"]);
    let pinned = env.library_roots.as_ref().map(|list| {
        std::iter::once((ACCOUNT_LIBRARY.to_owned(), account.clone()))
            .chain(list.iter().cloned())
            .chain(
                env.overlays
                    .iter()
                    .filter(|(n, _)| !list.iter().any(|(m, _)| m == n))
                    .cloned(),
            )
            .collect::<Vec<_>>()
    });
    let libraries = pinned
        .map_or_else(
            || {
                library_read_roots(fs, env)
                    .into_iter()
                    .map(|root| (None, root))
                    .collect()
            },
            |l| l.into_iter().map(|(n, root)| (Some(n), root)).collect::<Vec<_>>(),
        )
        .into_iter()
        .map(|(named, root)| {
            let name = if let Some(n) = named {
                n
            } else if root == account {
                ACCOUNT_LIBRARY.to_owned()
            } else {
                env.overlays
                    .iter()
                    .find(|(_, r)| *r == root)
                    .map_or_else(|| basename(&root).to_owned(), |(n, _)| n.clone())
            };
            let editable = env.overlays.iter().any(|(_, r)| *r == root);
            Library { name, root, editable }
        })
        .collect();
    Roots {
        workspace: Some(dirname(&project)),
        confine: Some(crate::confine::workspace_dir_of(env, &project)),
        libraries,
        meta: Some(env.meta_root.clone()),
        project,
        ..Roots::default()
    }
}

/// A project's own nodes, keyed by name, as `loadModules` answers.
///
/// # Errors
/// A read the source refuses.
pub fn load_modules(fs: &dyn Fs, env: &Env, dir: &str) -> Res<IndexMap<String, LoadedModule>> {
    let roots = roots_for(fs, env, dir);
    let read = load_nodes(fs, &mut Cache::new(), &roots).map_err(src)?;
    Ok(read.nodes)
}

/// Every module a project can name by ref, as `loadModuleRefs` answers.
///
/// # Errors
/// A read the source refuses.
pub fn load_module_refs(fs: &dyn Fs, env: &Env, dir: &str) -> Res<Arc<Vocabulary>> {
    let roots = roots_for(fs, env, dir);
    Cache::new().vocabulary(fs, &roots).map_err(src)
}

/// One node resolved as the compile resolves it, or None when `key` names no node.
///
/// # Errors
/// A read the source refuses.
pub fn resolve_node(fs: &dyn Fs, env: &Env, dir: &str, key: &str) -> Res<Option<Resolved>> {
    resolve_node_with(fs, env, dir, key, |_| {})
}

/// The same, with the node changed by `adjust` before it resolves.
///
/// # Errors
/// A read the source refuses.
pub fn resolve_node_with(
    fs: &dyn Fs,
    env: &Env,
    dir: &str,
    key: &str,
    adjust: impl FnOnce(&mut LoadedModule),
) -> Res<Option<Resolved>> {
    let roots = roots_for(fs, env, dir);
    let mut cache = Cache::new();
    let nodes = load_nodes(fs, &mut cache, &roots).map_err(src)?.nodes;
    let Some(node) = nodes.get(key) else { return Ok(None) };
    let vocab = cache.vocabulary(fs, &roots).map_err(src)?;
    let mut node = node.clone();
    adjust(&mut node);
    let one: IndexMap<String, LoadedModule> = std::iter::once((key.to_owned(), node)).collect();
    let out = resolve_map(fs, &roots, &one, &vocab.modules).map_err(src)?;
    Ok(out.modules.into_iter().next().map(|(_, r)| r))
}

/// A project's nodes loaded once, for a caller that resolves several of them: the same resolution as `resolve_node`.
pub struct NodeResolver<'a> {
    fs: &'a dyn Fs,
    roots: Roots,
    nodes: IndexMap<String, LoadedModule>,
    vocab: Arc<Vocabulary>,
}

impl<'a> NodeResolver<'a> {
    /// # Errors
    /// A read the source refuses.
    pub fn load(fs: &'a dyn Fs, env: &Env, dir: &str) -> Res<NodeResolver<'a>> {
        let roots = roots_for(fs, env, dir);
        let mut cache = Cache::new();
        let nodes = load_nodes(fs, &mut cache, &roots).map_err(src)?.nodes;
        let vocab = cache.vocabulary(fs, &roots).map_err(src)?;
        Ok(NodeResolver {
            fs,
            roots,
            nodes,
            vocab,
        })
    }

    /// One node resolved, or None when `key` names no node.
    ///
    /// # Errors
    /// A read the source refuses.
    pub fn resolve(&self, key: &str) -> Res<Option<Resolved>> {
        let Some(node) = self.nodes.get(key) else {
            return Ok(None);
        };
        let one: IndexMap<String, LoadedModule> = std::iter::once((key.to_owned(), node.clone())).collect();
        let out = resolve_map(self.fs, &self.roots, &one, &self.vocab.modules).map_err(src)?;
        Ok(out.modules.into_iter().next().map(|(_, r)| r))
    }
}

/// Every node of a project resolved, in the loader's order: what `resolveModuleMap(loadModules(dir), loadModuleRefs(dir))` gives.
///
/// # Errors
/// A read the source refuses.
pub fn resolve_all(fs: &dyn Fs, env: &Env, dir: &str) -> Res<IndexMap<String, Resolved>> {
    let roots = roots_for(fs, env, dir);
    let mut cache = Cache::new();
    let nodes = load_nodes(fs, &mut cache, &roots).map_err(src)?.nodes;
    let vocab = cache.vocabulary(fs, &roots).map_err(src)?;
    Ok(resolve_map(fs, &roots, &nodes, &vocab.modules).map_err(src)?.modules)
}
