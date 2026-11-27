// address.rs — the grammar of a project's address, `/<repo>/<project>[/<view>]`, and the names it reserves; pure strings, no origin.
// A reader would get wrong that a project ends at its LAST `.k7` segment: the view after it never claims the suffix.
// The reasoning is in apps/k7-daw-frontend/packages/web-protocol/docs/addresses.md.

use serde::Serialize;

/// The suffix of a project folder: one, since everything a project can be is a shape inside `.k7`.
pub const PROJECT_SUFFIX: &str = ".k7";
/// The workspace stocked with the demos, which the home page offers last.
pub const EXAMPLES_WORKSPACE: &str = "Examples";
/// The bench's reserved repository name, which resolves through no lookup at all.
pub const BENCH_REPO: &str = "@banc";
/// The workspace the bench lives in on the machine.
pub const BENCH_WORKSPACE: &str = "Banc";
/// The bench's one project, a live rig rebuilt on every open.
pub const BENCH_PROJECT: &str = "live.k7";
/// A project in a workspace with no repository.
pub const LOCAL_REPO: &str = "@local";
/// The reserved repository of a surface that belongs to another device.
pub const LAN_REPO: &str = "@lan";
/// The segment below a project that opens one surface on its own.
pub const EXTERNAL_SEGMENT: &str = "@external";
/// The pages that hang off k7's home; the first five are whole words, the media library is a prefix.
pub const SURFACES_PAGE: &str = "surfaces";
pub const LIBRARIES_PAGE: &str = "libraries";
pub const SETTINGS_PAGE: &str = "settings";
pub const ABOUT_PAGE: &str = "about";
pub const TASKS_PAGE: &str = "tasks";
pub const MEDIA_PAGE: &str = "media";
/// The foreign project files a drop may carry: the self-contained ones.
pub const DROPPABLE_PROJECTS: [&str; 2] = [".dawproject", ".zip"];

const HOME_PAGES: [&str; 6] = [
    LIBRARIES_PAGE,
    SETTINGS_PAGE,
    ABOUT_PAGE,
    TASKS_PAGE,
    SURFACES_PAGE,
    MEDIA_PAGE,
];

/// Which project an editor was opened on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Launch {
    /// The repository's short name, never a URI.
    pub repo: String,
    /// The project folder inside the repository; `.` when the repository is the project.
    pub project: String,
}

/// One surface of a project on another device, as its address names it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", ts(rename_all = "camelCase"))]
pub struct LanSurface {
    pub engine_id: String,
    pub workspace: String,
    pub project: String,
    pub surface: String,
}

/// A workspace and the project inside it, `.` when the workspace is the project.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LocalTarget {
    pub workspace: String,
    pub project: String,
}

/// Split a path into segments, tolerating any number of slashes at either end.
fn segments(p: &str) -> Vec<&str> {
    p.split('/').filter(|s| !s.is_empty()).collect()
}

/// The segments below the mount, or none when the address is not under it.
fn below_mount<'a>(pathname: &'a str, base: &str) -> Option<Vec<&'a str>> {
    let segs = segments(pathname);
    let skip = segments(base);
    if skip.iter().enumerate().any(|(i, s)| segs.get(i) != Some(s)) {
        return None;
    }
    Some(segs[skip.len()..].to_vec())
}

/// Whether a folder with this NAME is a project; the name decides, never the contents.
#[must_use]
pub fn is_project(name: &str) -> bool {
    name.len() > PROJECT_SUFFIX.len() && name.ends_with(PROJECT_SUFFIX)
}

/// The folder name with its suffix taken off, what a project is called.
#[must_use]
pub fn project_label(name: &str) -> String {
    if is_project(name) {
        name[..name.len() - PROJECT_SUFFIX.len()].to_owned()
    } else {
        name.to_owned()
    }
}

/// `decodeURIComponent`, keeping the text as typed when a `%` is no escape or the bytes are no UTF-8.
fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'%' {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        let hex = |k: usize| bytes.get(k).and_then(|b| (*b as char).to_digit(16));
        match (hex(i + 1), hex(i + 2)) {
            (Some(h), Some(l)) => {
                out.push((h * 16 + l) as u8);
                i += 3;
            }
            _ => return s.to_owned(),
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_owned())
}

/// `encodeURIComponent`, leaving `@` alone: RFC 3986 allows it literally.
fn encode_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')'
            | b'@' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Read `/<repo>/<project…>[/<view…>]` into a target; `None` for an address outside `base` or with no repository.
#[must_use]
pub fn parse_launch(pathname: &str, base: &str) -> Option<Launch> {
    let below = below_mount(pathname, base)?;
    let (repo, tail) = below.split_first()?;
    let project = match tail.iter().rposition(|s| is_project(s)) {
        None => ".".to_owned(),
        Some(last) => tail[..=last].iter().map(|s| decode(s)).collect::<Vec<_>>().join("/"),
    };
    Some(Launch {
        repo: decode(repo),
        project,
    })
}

/// The path `/<repo>/<project>`, each segment encoded; the project `.` adds none.
#[must_use]
pub fn launch_path(repo: &str, project: &str) -> String {
    let mut parts = vec![encode_segment(repo)];
    if project != "." {
        parts.extend(segments(project).into_iter().map(encode_segment));
    }
    format!("/{}", parts.join("/"))
}

/// The router `basename`: a slice of the given path up to the project's end, `base` when it names none.
#[must_use]
pub fn project_basename(pathname: &str, base: &str) -> String {
    let Some(l) = parse_launch(pathname, base) else {
        return base.to_owned();
    };
    let project = if l.project == "." {
        0
    } else {
        l.project.split('/').count()
    };
    let depth = segments(base).len() + 1 + project;
    let segs = segments(pathname);
    format!("/{}", segs[..depth.min(segs.len())].join("/"))
}

/// Whether a file NAME is one a drop may carry.
#[must_use]
pub fn is_droppable_project(name: &str) -> bool {
    let lower = name.to_lowercase();
    DROPPABLE_PROJECTS.iter().any(|ext| lower.ends_with(ext))
}

/// A surface hung off a project URL somebody else built, encoded once.
#[must_use]
pub fn external_of(project_url: &str, surface: &str) -> String {
    format!("{project_url}/{EXTERNAL_SEGMENT}/{}", encode_segment(surface))
}

/// Which surface this address opens on its own; `None` for a bare `@external` or an address naming no project.
#[must_use]
pub fn parse_external(pathname: &str, base: &str) -> Option<String> {
    parse_launch(pathname, base)?;
    let segs = segments(pathname);
    let i = segs.iter().position(|s| *s == EXTERNAL_SEGMENT)?;
    segs.get(i + 1).map(|s| decode(s))
}

/// The path of a surface of another device: below the project like `external_of`, under that device's id.
#[must_use]
pub fn lan_surface_path(engine_id: &str, workspace: &str, project: &str, surface: &str) -> String {
    let at = if project == "." {
        workspace.to_owned()
    } else {
        format!("{workspace}/{project}")
    };
    external_of(&launch_path(LAN_REPO, &format!("{engine_id}/{at}")), surface)
}

/// Split a `@local` project part into workspace and project, checked not trusted.
#[must_use]
pub fn local_target(project: &str) -> Option<LocalTarget> {
    let segs = segments(project);
    let (workspace, rest) = segs.split_first()?;
    if *workspace == "." || *workspace == ".." || workspace.contains('\\') {
        return None;
    }
    let project = if rest.is_empty() {
        ".".to_owned()
    } else {
        rest.join("/")
    };
    Some(LocalTarget {
        workspace: (*workspace).to_owned(),
        project,
    })
}

/// The path of a `@local` project, built like every other project address.
#[must_use]
pub fn local_project_path(workspace: &str, project: &str) -> String {
    let at = if project == "." {
        workspace.to_owned()
    } else {
        format!("{workspace}/{project}")
    };
    launch_path(LOCAL_REPO, &at)
}

/// Read a `@lan` surface address; `None` for anything else, an `@lan` with no surface after it included.
#[must_use]
pub fn parse_lan_surface(pathname: &str, base: &str) -> Option<LanSurface> {
    let at = parse_launch(pathname, base)?;
    if at.repo != LAN_REPO {
        return None;
    }
    let surface = parse_external(pathname, base)?;
    let mut parts = at.project.split('/');
    let engine_id = parts.next().filter(|s| !s.is_empty())?;
    let rest: Vec<&str> = parts.collect();
    let target = local_target(&rest.join("/"))?;
    Some(LanSurface {
        engine_id: engine_id.to_owned(),
        workspace: target.workspace,
        project: target.project,
        surface,
    })
}

/// The library chain a bench project names: `Some(None)` for the bare rig, `None` when it names no bench.
#[must_use]
pub fn bench_chain_of(project: &str) -> Option<Option<String>> {
    if project == BENCH_PROJECT {
        return Some(None);
    }
    let reference = project.strip_suffix(&format!("/{BENCH_PROJECT}"))?;
    // Folder names only: an empty or dotted segment, or one claiming a project, is no chain ref.
    let plain = |s: &str| !s.is_empty() && s != "." && s != ".." && !s.ends_with(PROJECT_SUFFIX);
    reference.split('/').all(plain).then(|| Some(reference.to_owned()))
}

/// The project half of a bench address, seeded with `chain` when one is given.
#[must_use]
pub fn bench_project(chain: Option<&str>) -> String {
    match chain {
        Some(c) if !c.is_empty() => format!("{c}/{BENCH_PROJECT}"),
        _ => BENCH_PROJECT.to_owned(),
    }
}

/// How a project is named in an engine claim, the one spelling "one engine per project" rests on.
#[must_use]
pub fn claim_key(repo: &str, project: &str) -> String {
    format!("{repo}/{project}")
}

/// Which of an account's engine sessions a claimed address plays in: the bench has its own.
#[must_use]
pub fn claim_is_bench(claim: &str) -> bool {
    claim.starts_with(&format!("{BENCH_REPO}/"))
}

/// Whether this address is one of the pages that hang off k7's home, or any page below one.
#[must_use]
pub fn is_home_page(pathname: &str, base: &str) -> bool {
    below_mount(pathname, base)
        .and_then(|s| s.first().copied())
        .is_some_and(|w| HOME_PAGES.contains(&w))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_project_ends_at_its_last_suffixed_segment() {
        let l = parse_launch("/@local/Studio/songs/winter.k7/timeline", "/").unwrap();
        assert_eq!(
            (l.repo.as_str(), l.project.as_str()),
            ("@local", "Studio/songs/winter.k7")
        );
        assert_eq!(parse_launch("/r/view/edit", "/").unwrap().project, ".");
        assert_eq!(project_basename("/r/a.k7/tl/x", "/"), "/r/a.k7");
        assert_eq!(project_basename("/", "/"), "/");
    }

    #[test]
    fn an_escape_that_is_none_is_kept_as_typed() {
        assert_eq!(decode("100%"), "100%");
        assert_eq!(decode("%E0%A4%A"), "%E0%A4%A");
        assert_eq!(decode("a%2Fb%20c"), "a/b c");
        assert_eq!(decode("%FF"), "%FF");
    }

    #[test]
    fn a_segment_encodes_as_the_page_always_did() {
        assert_eq!(encode_segment("a b@c/é"), "a%20b@c%2F%C3%A9");
        assert_eq!(launch_path("@local", "W/x.k7"), "/@local/W/x.k7");
    }

    #[test]
    fn a_lan_surface_reads_back_what_was_written() {
        let path = lan_surface_path("eng1", "W", "p.k7", "deck 1");
        let s = parse_lan_surface(&path, "/").unwrap();
        assert_eq!(
            (
                s.engine_id.as_str(),
                s.workspace.as_str(),
                s.project.as_str(),
                s.surface.as_str()
            ),
            ("eng1", "W", "p.k7", "deck 1")
        );
        assert_eq!(parse_lan_surface("/@lan/eng1/W/p.k7", "/"), None);
    }

    #[test]
    fn the_bench_names_a_rig_bare_or_seeded() {
        assert_eq!(bench_chain_of("live.k7"), Some(None));
        assert_eq!(bench_chain_of("@a/v/warm/live.k7"), Some(Some("@a/v/warm".into())));
        assert_eq!(bench_chain_of("/live.k7"), None);
        assert_eq!(bench_chain_of("../live.k7"), None);
        assert_eq!(bench_chain_of("a.k7/live.k7"), None);
        assert_eq!(bench_project(Some("@a/c")), "@a/c/live.k7");
    }

    #[test]
    fn the_home_pages_are_read_before_a_workspace() {
        assert!(is_home_page("/settings/engine", "/"));
        assert!(!is_home_page("/Studio/settings", "/"));
        assert!(is_droppable_project("A.DAWPROJECT"));
    }
}
