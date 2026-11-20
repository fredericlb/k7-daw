// resolve/extends.rs — the `extends` half: the ancestry folded deepest first, and what a child named re-resolved against the chain.
// A reader would get wrong that a fold is kept per call: a parent folded once serves every node over it, which is what bounds a fan-out.

use super::{Kept, MAX_DEPTH, Resolver, weigh};
use crate::files::Severity;
use crate::files::scalar::js_space;
use crate::modules::assets::Files;
use crate::modules::files::FILE_PARAMS;
use crate::modules::preset::base_preset_in;
use crate::modules::{LoadedModule, merge_loaded};
use crate::source::SourceError;
use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::Arc;

/// The fields a module owns rather than inherits, by key presence: a decorated key is the child's even when it is empty.
pub const OWN_ONLY: [&str; 7] = ["name", "file", "node", "home", "chain", "nodeId", "presetRel"];

impl<'a> Resolver<'a> {
    /// `m` with its ancestry folded in, deepest first; None when the budget stops it. A cycle or a deep chain stops the fold with a warning.
    pub(super) fn fold_extends(
        &mut self,
        m: &LoadedModule,
        seen: &HashSet<String>,
        cache_key: Option<&str>,
    ) -> Result<Option<LoadedModule>, SourceError> {
        let mut seen = seen.clone();
        let mut chain: Vec<(String, Cow<'a, LoadedModule>)> = Vec::new();
        let mut base = None;
        let mut clean = true;
        let mut next = m.authored.extends.clone();
        while let Some(r) = next.take().filter(|r| !r.is_empty()) {
            if seen.contains(&r) {
                let path: Vec<&str> = chain.iter().map(|(r, _)| r.as_str()).collect();
                self.finding(
                    Severity::Warning,
                    format!(
                        "extends cycle through {r} (after {}) — the fold stopped there",
                        path.join(" > ")
                    ),
                );
                clean = false;
                break;
            }
            if !self.by_ref.contains_key(&r) {
                break;
            }
            if chain.len() >= MAX_DEPTH {
                self.finding(
                    Severity::Warning,
                    format!("extends goes more than {MAX_DEPTH} levels deep at {r} — the fold stopped there"),
                );
                clean = false;
                break;
            }
            // A kept fold is reused only where walking would have given the same answer: depth left, no ref on the path.
            if let Some(hit) = self.folded.get(&r)
                && chain.len() + hit.refs.len() <= MAX_DEPTH
                && hit.refs.iter().all(|x| !seen.contains(x))
            {
                base = Some(hit.clone());
                break;
            }
            seen.insert(r.clone());
            let Some(p) = self.module(&r)? else { break };
            next = p.authored.extends.clone();
            chain.push((r, p));
        }
        let mut acc = base;
        for (r, p) in chain.iter().rev() {
            let kept = match acc.take() {
                None => {
                    if !self.charge(weigh(p)) {
                        return Ok(None);
                    }
                    Kept {
                        module: Arc::new(p.as_ref().clone()),
                        folders: Arc::new(vec![p.folder_abs.clone()]),
                        refs: Arc::new(vec![r.clone()]),
                    }
                }
                Some(below) => {
                    if !self.charge(weigh(&below.module) + weigh(p)) {
                        return Ok(None);
                    }
                    let folders = stack(p.folder_abs.clone(), &below.folders);
                    let merged = self.inherit_files(merge_loaded(&below.module, p), p, &folders)?;
                    Kept {
                        module: Arc::new(merged),
                        folders: Arc::new(folders),
                        refs: Arc::new(stack(r.clone(), &below.refs)),
                    }
                }
            };
            if clean {
                self.folded.insert(r.clone(), kept.clone());
            }
            acc = Some(kept);
        }
        let Some(below) = acc else {
            return Ok(Some(m.clone()));
        };
        if !self.charge(weigh(&below.module) + weigh(m)) {
            return Ok(None);
        }
        let folders = stack(m.folder_abs.clone(), &below.folders);
        let out = self.inherit_files(merge_loaded(&below.module, m), m, &folders)?;
        if clean && let Some(key) = cache_key {
            let kept = Kept {
                module: Arc::new(out.clone()),
                folders: Arc::new(folders),
                refs: Arc::new(stack(key.to_owned(), &below.refs)),
            };
            self.folded.insert(key.to_owned(), kept);
        }
        Ok(Some(out))
    }

    /// Re-resolves what `child` NAMED against the whole chain's folders, nearest first; an inherited ref was resolved where declared.
    fn inherit_files(
        &mut self,
        merged: LoadedModule,
        child: &LoadedModule,
        folders: &[Option<String>],
    ) -> Result<LoadedModule, SourceError> {
        let base = child
            .authored
            .base
            .as_deref()
            .filter(|b| !b.trim_matches(js_space).is_empty());
        let params: Vec<&str> = FILE_PARAMS
            .iter()
            .copied()
            .filter(|p| {
                child
                    .assets
                    .as_ref()
                    .and_then(|a| a.get(*p))
                    .is_some_and(|r| !r.is_empty())
            })
            .collect();
        if (params.is_empty() && base.is_none()) || folders.len() < 2 {
            return Ok(merged);
        }
        let refs: Vec<Option<&str>> = folders.iter().map(Option::as_deref).collect();
        let mut out = merged;
        let mut files = Files::new(self.source, self.places, &self.at);
        for param in params {
            let Some(r) = child.assets.as_ref().and_then(|a| a.get(param)) else {
                continue;
            };
            let found = files.file_param_in(param, r, &refs)?;
            match param {
                "model" => out.model = found,
                _ => out.onnx = found,
            }
        }
        // The nearest folder that HAS this base wins; a miss leaves the merged answer alone.
        if let Some(base) = base {
            for folder in folders.iter().flatten() {
                if let Some(hit) = base_preset_in(&mut files, folder, base)? {
                    out.preset = Some(hit);
                    break;
                }
            }
        }
        self.findings.append(&mut files.findings);
        Ok(out)
    }
}

/// `head` over `rest`, capped at the depth a fold can reach.
fn stack<T: Clone>(head: T, rest: &[T]) -> Vec<T> {
    std::iter::once(head)
        .chain(rest.iter().take(MAX_DEPTH).cloned())
        .collect()
}
