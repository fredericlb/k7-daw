//! verb_paths.rs — the virtual roots a verb bundle shows the browser: `/w` the workspace, `/l/<name>` a library, `/k7` the code,
//! `/s` the account's home, `/b/<n>` a sample bank, `/x/<n>` any other. What a reader gets wrong: the longest real root names a path.

use sha2::{Digest, Sha256};
use song::roots::{Library, Roots};
use song::source::path::PathRule;

use crate::paths::{is_within, resolve_all};

/// The real roots of one bundle and the virtual folder each one shows as.
pub struct Virtual {
    /// Longest real root first, so the nearest root names a path.
    pairs: Vec<(String, String)>,
}

impl Virtual {
    /// The roots of a bundle: the workspace, each library by name, the code, the account's home, then the roots' own other folders.
    pub fn new(
        workspace: &str,
        libraries: &[(String, String)],
        code: &str,
        studio: &str,
        roots: &Roots,
    ) -> Virtual {
        let mut v = Virtual { pairs: Vec::new() };
        v.add(workspace, "/w".into());
        for (name, dir) in libraries {
            v.add(dir, format!("/l/{name}"));
        }
        v.add(code, "/k7".into());
        v.add(studio, "/s".into());
        for (i, d) in roots.sample_banks.iter().enumerate() {
            v.add(d, format!("/b/{i}"));
        }
        let rest = roots
            .also_readable
            .iter()
            .chain(&roots.confine)
            .chain(&roots.models)
            .chain(&roots.renders)
            .chain(&roots.scan);
        for (i, d) in rest.enumerate() {
            if !v
                .pairs
                .iter()
                .any(|(r, _)| is_within(r, &resolve_all(&[d])))
            {
                v.add(d, format!("/x/{i}"));
            }
        }
        v.pairs.sort_by_key(|(r, _)| std::cmp::Reverse(r.len()));
        v
    }

    fn add(&mut self, real: &str, virt: String) {
        let real = resolve_all(&[real]);
        if !self.pairs.iter().any(|(r, _)| *r == real) {
            self.pairs.push((real, virt));
        }
    }

    /// The virtual spelling of a real path; a path under no root shows as a hash, never as itself.
    pub fn virt(&self, path: &str) -> String {
        let p = resolve_all(&[path]);
        for (real, virt) in &self.pairs {
            if is_within(real, &p) {
                return format!("{virt}{}", &p[real.len()..]);
            }
        }
        let sum: String = Sha256::digest(p.as_bytes())
            .iter()
            .take(6)
            .map(|b| format!("{b:02x}"))
            .collect();
        format!("/x/{sum}")
    }

    /// The virtual spelling of a real path under one of the roots; `None` for a path under none.
    pub fn shown(&self, path: &str) -> Option<String> {
        let p = resolve_all(&[path]);
        self.pairs
            .iter()
            .find(|(real, _)| is_within(real, &p))
            .map(|(real, virt)| format!("{virt}{}", &p[real.len()..]))
    }

    /// The real path a virtual one names; `None` for a path under no virtual root or one that climbs.
    pub fn real(&self, path: &str) -> Option<String> {
        if path.split('/').any(|s| s == "..") {
            return None;
        }
        let mut by_virt: Vec<&(String, String)> = self.pairs.iter().collect();
        by_virt.sort_by_key(|(_, v)| std::cmp::Reverse(v.len()));
        by_virt.into_iter().find_map(|(real, virt)| {
            let rest = path.strip_prefix(virt.as_str())?;
            (rest.is_empty() || rest.starts_with('/')).then(|| format!("{real}{rest}"))
        })
    }

    /// Whether a canonical real path lies under one of the roots, each canonicalized too (a link out of a root is not under it).
    pub fn reaches(&self, canonical: &str) -> bool {
        self.pairs.iter().any(|(real, _)| {
            let root = std::fs::canonicalize(real).map_or_else(
                |_| real.clone(),
                |c| crate::confine::spelled(&c).unwrap_or_else(|| real.clone()),
            );
            is_within(&root, canonical)
        })
    }

    /// The roots of a compile with every folder virtual, spelled by the posix rule the browser's vfs reads them by.
    pub fn roots(&self, r: &Roots) -> Roots {
        let one = |s: &String| self.virt(s);
        let opt = |s: &Option<String>| s.as_ref().map(one);
        Roots {
            project: one(&r.project),
            workspace: opt(&r.workspace),
            workspace_namespace: r.workspace_namespace.clone(),
            confine: opt(&r.confine),
            libraries: r
                .libraries
                .iter()
                .map(|l| Library {
                    name: l.name.clone(),
                    root: one(&l.root),
                    editable: l.editable,
                })
                .collect(),
            meta: opt(&r.meta),
            scan: opt(&r.scan),
            renders: opt(&r.renders),
            models: opt(&r.models),
            sample_banks: r.sample_banks.iter().map(one).collect(),
            also_readable: r.also_readable.iter().map(one).collect(),
            paths: PathRule::Posix,
        }
    }
}
