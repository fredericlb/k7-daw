// address.rs — what an ssh address, a server's version and a typed port say: the host, the name a clone takes, a readable remote.
// A reader would get wrong that these follow the page's old regexes by hand: the same inputs must answer the same.

use serde::Serialize;

/// The host and port an ssh address names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SshHost {
    pub host: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub port: Option<u32>,
}

/// Whether a path is one a regex's `.` would cross: it stops at a line break.
fn one_line(s: &str) -> bool {
    !s.contains(['\n', '\r', '\u{2028}', '\u{2029}'])
}

/// The `[host]` or `host` an authority starts with, and what follows it.
fn long_host(rest: &str) -> Option<(&str, &str)> {
    if let Some(inner) = rest.strip_prefix('[')
        && let Some(end) = inner.find(']')
        && end > 0
    {
        return Some((&rest[..end + 2], &rest[end + 2..]));
    }
    let end = rest.find([':', '/']).unwrap_or(rest.len());
    (end > 0).then(|| (&rest[..end], &rest[end..]))
}

/// `ssh://[user@]host[:port]` and what follows: the host, the port digits if any, and the tail (empty or a `/…`).
fn long_form(u: &str) -> Option<(&str, Option<&str>, &str)> {
    let rest = u.strip_prefix("ssh://")?;
    let with_user = rest
        .split_once('@')
        .filter(|(user, _)| !user.is_empty() && !user.contains('/'))
        .map(|(_, r)| r);
    for candidate in with_user.into_iter().chain(std::iter::once(rest)) {
        let Some((host, after)) = long_host(candidate) else {
            continue;
        };
        let (port, tail) = match after.strip_prefix(':') {
            Some(p) => {
                let digits = p.len() - p.trim_start_matches(|c: char| c.is_ascii_digit()).len();
                if digits == 0 {
                    continue;
                }
                (Some(&p[..digits]), &p[digits..])
            }
            None => (None, after),
        };
        if tail.is_empty() || tail.starts_with('/') {
            return Some((host, port, tail));
        }
    }
    None
}

/// `[user@]host:path` with no `//` after the colon: the host and the path.
fn scp_form(u: &str) -> Option<(&str, &str)> {
    let plain = |c: char| c != ':' && c != '/' && !c.is_whitespace();
    let with_user = u
        .split_once('@')
        .filter(|(user, _)| !user.is_empty() && user.chars().all(|c| c != '/' && !c.is_whitespace()))
        .map(|(_, r)| r);
    for candidate in with_user.into_iter().chain(std::iter::once(u)) {
        let end = candidate.find(|c| !plain(c)).unwrap_or(candidate.len());
        if end == 0 {
            continue;
        }
        let Some(path) = candidate[end..].strip_prefix(':') else {
            continue;
        };
        if !path.starts_with("//") {
            return Some((&candidate[..end], path));
        }
    }
    None
}

/// The host and port an address names when it is ssh (`ssh://u@h:22/p` or `u@h:path`), else none.
#[must_use]
pub fn ssh_host(url: &str) -> Option<SshHost> {
    let u = url.trim();
    if let Some((host, port, _)) = long_form(u) {
        let port = port.map(|p| p.parse::<u32>().unwrap_or(u32::MAX));
        return Some(SshHost {
            host: host.to_owned(),
            port,
        });
    }
    scp_form(u).map(|(host, _)| SshHost {
        host: host.to_owned(),
        port: None,
    })
}

/// Whether the address is one k7 never takes: the web schemes people paste from a host's page.
#[must_use]
pub fn is_web_address(url: &str) -> bool {
    let u = url.trim().to_ascii_lowercase();
    ["http://", "https://", "git://"].iter().any(|p| u.starts_with(p))
}

/// The `ssh://` form with its path: the path starts after the slash that ends the authority.
fn long_form_path(u: &str) -> Option<(&str, &str)> {
    let (host, _, tail) = long_form(u)?;
    let path = tail.strip_prefix('/')?;
    one_line(path).then_some((host, path))
}

/// A remote as a person reads it: the host and the repository, never the user or the scheme (`github.com/o/r`).
#[must_use]
pub fn remote_name(url: &str) -> String {
    let u = url.trim();
    let hit = long_form_path(u).or_else(|| scp_form(u).filter(|(_, p)| one_line(p)));
    let Some((host, path)) = hit else { return u.to_owned() };
    let path = path.trim_start_matches('/');
    let path = path
        .strip_suffix(".git/")
        .or_else(|| path.strip_suffix(".git"))
        .unwrap_or(path);
    if path.is_empty() {
        host.to_owned()
    } else {
        format!("{host}/{path}")
    }
}

/// The repository's own name in an ssh or file address (`git@host:me/songs.git` gives `songs`), or empty.
#[must_use]
pub fn name_from_url(url: &str) -> String {
    let t = url.trim().trim_end_matches('/');
    let tail = t.rsplit(['/', ':']).next().unwrap_or("");
    let n = tail.len();
    if n >= 4 && tail.is_char_boundary(n - 4) && tail[n - 4..].eq_ignore_ascii_case(".git") {
        tail[..n - 4].to_owned()
    } else {
        tail.to_owned()
    }
}

/// The name a clone starts with: the workspace's own, and the next free `name 2` when this device has one of that name.
#[must_use]
pub fn clone_name(name: &str, taken: &[String]) -> String {
    let has = |n: &str| taken.iter().any(|t| t.to_lowercase() == n.to_lowercase());
    if !has(name) {
        return name.to_owned();
    }
    (2..)
        .map(|i| format!("{name} {i}"))
        .find(|n| !has(n))
        .unwrap_or_default()
}

/// A server's version without its build metadata: `17.0.2+gitea-1.22.0` is `17.0.2`.
#[must_use]
pub fn short_version(v: &str) -> String {
    v.split('+').next().unwrap_or("").trim().to_owned()
}

/// The ssh port field: empty is the default, anything else a whole port or bad.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PortAnswer {
    Default,
    Bad,
    Port { port: u16 },
}

/// A number as JavaScript's `Number(text)` reads it, for the spellings a port field can hold.
fn js_number(t: &str) -> f64 {
    for (p, r) in [("0x", 16), ("0o", 8), ("0b", 2)] {
        let digits = t.strip_prefix(p).or_else(|| t.strip_prefix(&p.to_uppercase()));
        if let Some(d) = digits {
            return u64::from_str_radix(d, r).map_or(f64::NAN, |v| v as f64);
        }
    }
    let ok = t
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '+' | '-' | '.' | 'e' | 'E'));
    if ok {
        t.parse::<f64>().unwrap_or(f64::NAN)
    } else {
        f64::NAN
    }
}

#[must_use]
pub fn ssh_port(typed: &str) -> PortAnswer {
    let t = typed.trim();
    if t.is_empty() {
        return PortAnswer::Default;
    }
    let n = js_number(t);
    if n.fract() == 0.0 && n > 0.0 && n < 65536.0 {
        PortAnswer::Port { port: n as u16 }
    } else {
        PortAnswer::Bad
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(h: &str, port: Option<u32>) -> Option<SshHost> {
        Some(SshHost { host: h.into(), port })
    }

    #[test]
    fn reads_both_spellings_of_an_ssh_host() {
        assert_eq!(ssh_host("ssh://git@h.org:2222/me/r.git"), host("h.org", Some(2222)));
        assert_eq!(ssh_host("git@h.org:me/r.git"), host("h.org", None));
        assert_eq!(ssh_host("https://h.org/r.git"), None);
        assert_eq!(ssh_host("/srv/r.git"), None);
        assert_eq!(ssh_host("ssh://[::1]:22/r"), host("[::1]", Some(22)));
        assert_eq!(ssh_host("ssh://h"), host("h", None));
    }

    #[test]
    fn refuses_the_web_schemes() {
        for url in [
            "https://h.org/r.git",
            "http://h.org/r.git",
            "git://h.org/r.git",
            " HTTPS://h.org/r.git",
        ] {
            assert!(is_web_address(url), "{url}");
        }
        assert!(!is_web_address("ssh://git@h.org/r.git"));
        assert!(!is_web_address("file:///srv/r.git"));
    }

    #[test]
    fn names_a_remote_as_a_person_reads_it() {
        assert_eq!(remote_name("git@github.com:o/r.git"), "github.com/o/r");
        assert_eq!(
            remote_name("ssh://git@host:2222/band/live-set.git"),
            "host/band/live-set"
        );
        assert_eq!(remote_name("ssh://host/"), "host");
        assert_eq!(remote_name("ssh://host"), "ssh://host");
        assert_eq!(remote_name("file:///tmp/r.git"), "file:///tmp/r.git");
        assert_eq!(remote_name(" /srv/r.git "), "/srv/r.git");
        assert_eq!(remote_name("git@h:/o/r.git/"), "h/o/r");
    }

    #[test]
    fn takes_the_clone_name_from_the_address() {
        assert_eq!(name_from_url("git@git.example.org:me/songs.git"), "songs");
        assert_eq!(name_from_url("ssh://git@host:2222/band/live-set.git"), "live-set");
        assert_eq!(name_from_url("file:///tmp/k7/remote.git/"), "remote");
        assert_eq!(name_from_url(""), "");
        assert_eq!(name_from_url("x/Repo.GIT"), "Repo");
    }

    #[test]
    fn names_a_clone_the_next_free_name() {
        let t = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(clone_name("Band", &[]), "Band");
        assert_eq!(clone_name("Band", &t(&["band"])), "Band 2");
        assert_eq!(clone_name("Band", &t(&["Band", "Band 2"])), "Band 3");
    }

    #[test]
    fn drops_a_versions_build_metadata() {
        assert_eq!(short_version("17.0.2+gitea-1.22.0"), "17.0.2");
        assert_eq!(short_version("17.0.2"), "17.0.2");
    }

    #[test]
    fn reads_the_ssh_port_field() {
        assert_eq!(ssh_port(""), PortAnswer::Default);
        assert_eq!(ssh_port(" 2222 "), PortAnswer::Port { port: 2222 });
        assert_eq!(ssh_port("0"), PortAnswer::Bad);
        assert_eq!(ssh_port("70000"), PortAnswer::Bad);
        assert_eq!(ssh_port("2x"), PortAnswer::Bad);
        assert_eq!(ssh_port("abc"), PortAnswer::Bad);
        assert_eq!(ssh_port("0x10"), PortAnswer::Port { port: 16 });
        assert_eq!(ssh_port("1e3"), PortAnswer::Port { port: 1000 });
        assert_eq!(ssh_port("22.5"), PortAnswer::Bad);
    }
}
