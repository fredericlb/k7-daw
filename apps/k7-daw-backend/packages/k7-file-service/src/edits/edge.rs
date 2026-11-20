//! edge.rs — song's `DiskSource` behind the one place that spells a posix-form path the way the host reads it.
//! What a reader gets wrong: `/C:/x` is song-edit's form and `C:/x` is DiskSource's; every path crosses `disk` here, and nowhere else.

use song::{DiskSource, Entry, Source, SourceError, Span, Stat};

/// A posix-form path in DiskSource's spelling: `/C:/x` is `C:/x`, a posix path is unchanged.
#[must_use]
pub fn disk(p: &str) -> String {
    song_edit::winpath::from_posix(p).replace('\\', "/")
}

/// An `Env` with every root spelled posix, the only form song-edit computes on.
#[must_use]
pub fn posix_env(mut env: song_edit::confine::Env) -> song_edit::confine::Env {
    let p = |s: &str| super::posix(s);
    env.studio_home = p(&env.studio_home);
    env.libraries_home = p(&env.libraries_home);
    env.meta_root = p(&env.meta_root);
    env.models_root = p(&env.models_root);
    for (_, root) in &mut env.overlays {
        *root = p(root);
    }
    if let Some(roots) = &mut env.library_roots {
        for (_, root) in roots {
            *root = p(root);
        }
    }
    env
}

pub struct Edge(DiskSource);

impl Edge {
    /// A source over the folders `mounts`, given as real or posix-form paths.
    ///
    /// # Errors
    /// A mount that is not an absolute path.
    pub fn new<S: AsRef<str>>(mounts: &[S]) -> Result<Edge, SourceError> {
        let spelled: Vec<String> = mounts
            .iter()
            .map(|m| disk(&super::posix(m.as_ref())))
            .collect();
        DiskSource::new(&spelled).map(Edge)
    }
}

impl Edge {
    /// A source over folder mounts and single-file mounts, each given as a real or posix-form path.
    ///
    /// # Errors
    /// A mount that is not an absolute path.
    pub fn with_files<S: AsRef<str>, F: AsRef<str>>(
        dirs: &[S],
        files: &[F],
    ) -> Result<Edge, SourceError> {
        let spell = |m: &str| disk(&super::posix(m));
        let dirs: Vec<String> = dirs.iter().map(|m| spell(m.as_ref())).collect();
        let files: Vec<String> = files.iter().map(|m| spell(m.as_ref())).collect();
        DiskSource::with_files(&dirs, &files).map(Edge)
    }
}

impl Source for Edge {
    fn stamps_are_times(&self) -> bool {
        true
    }

    fn folder_id(&self, path: &str) -> Option<String> {
        self.0.folder_id(&disk(path))
    }

    fn stat(&self, path: &str) -> Result<Stat, SourceError> {
        self.0.stat(&disk(path))
    }

    fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError> {
        self.0.list(&disk(dir))
    }

    fn read(&self, path: &str, span: Span) -> Result<Vec<u8>, SourceError> {
        self.0.read(&disk(path), span)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use song::source::path::{normalize, windows_hazard};

    // The Source is handed the disk's own spelling, never the posix form a drive takes in the algebra.
    #[test]
    fn a_windows_shaped_path_reaches_disk_source_in_its_own_spelling() {
        for p in [
            "/C:/Users/a/p.k7",
            "/c:/Users/a/p.k7/x y.k7x",
            "/C:/Users",
            "/D:",
        ] {
            let spelled = disk(p);
            assert!(!spelled.starts_with('/'), "{p} -> {spelled}");
            assert!(!windows_hazard(&normalize(&spelled).unwrap()), "{spelled}");
        }
        assert_eq!(disk("/C:/Users/a"), "C:/Users/a");
        assert_eq!(disk("/tmp/x"), "/tmp/x");
        // A path a compile hands over in the host's own form is spelled the same way.
        assert_eq!(disk("C:\\lib\\kit\\a.sfz"), "C:/lib/kit/a.sfz");
    }

    // A bench's or a slot's mounts are folders and single files in the real Windows form.
    #[test]
    fn windows_folder_and_file_mounts_confine_the_posix_form() {
        let edge =
            Edge::with_files(&["C:\\libs\\k7-core"], &["C:\\studio\\_library-pins.k7x"]).unwrap();
        let ok = |p: &str| !matches!(edge.stat(p), Err(SourceError::Outside));
        assert!(ok("/C:/libs/k7-core/chains/a.k7x"));
        assert!(ok("/C:/studio/_library-pins.k7x"));
        assert!(!ok("/C:/studio/other.k7x"));
        assert!(!ok("/C:/libs/private/a.k7x"));
    }

    // A mount spelled as a real Windows path confines the posix-form paths under it and refuses the others.
    #[test]
    fn a_windows_mount_confines_the_posix_form() {
        let edge = Edge::new(&["C:\\Users\\a\\p.k7"]).unwrap();
        let inside = edge.stat("/C:/Users/a/p.k7/x.k7x");
        assert!(!matches!(inside, Err(SourceError::Outside)), "{inside:?}");
        assert!(matches!(
            edge.stat("/C:/Users/b/x.k7x"),
            Err(SourceError::Outside)
        ));
        assert!(matches!(
            edge.list("/C:/Users/a"),
            Err(SourceError::Outside)
        ));
    }

    // A UNC or verbatim mount keeps its two slashes from the real spelling to DiskSource and back to the real one.
    #[test]
    fn unc_and_verbatim_roots_survive_the_round_trip() {
        for (mount, posix, real) in [
            (
                "\\\\srv\\share\\p",
                "/UNC:/srv/share/p",
                "\\\\srv\\share\\p",
            ),
            (
                "\\\\?\\UNC\\srv\\share\\p",
                "/UNC:/srv/share/p",
                "\\\\srv\\share\\p",
            ),
            ("\\\\?\\C:\\p", "/C:/p", "C:\\p"),
        ] {
            assert_eq!(super::super::posix(mount), posix);
            assert_eq!(super::super::real(posix), real);
            let edge = Edge::new(&[mount]).unwrap();
            let inside = format!("{posix}/x.k7x");
            assert!(
                !matches!(edge.stat(&inside), Err(SourceError::Outside)),
                "{mount}"
            );
            assert!(
                !matches!(edge.list(posix), Err(SourceError::Outside)),
                "{mount}"
            );
            let sibling = posix.replace("/p", "/q/x.k7x");
            assert!(
                matches!(edge.stat(&sibling), Err(SourceError::Outside)),
                "{mount}"
            );
        }
        assert_eq!(disk("/UNC:/srv/share/p/x"), "//srv/share/p/x");
        assert_eq!(normalize("//srv/share/p/x").unwrap(), "//srv/share/p/x");
    }
}
