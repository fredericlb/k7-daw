//! ssh.rs — an SSH remote's LFS endpoint: `git-lfs-authenticate <path> upload|download` on the russh connection,
//! which answers where to speak HTTPS and with what header. What a reader gets wrong: the answer is reused only
//! until it expires, and a server that does not know the command is a server without LFS, not a failure.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::transport::ssh::{parse, quote, run};
use crate::{Error, Net, Result};

/// What the server granted: the endpoint and the headers that open it.
#[derive(Clone, Debug)]
pub struct Granted {
    pub href: String,
    pub header: BTreeMap<String, String>,
    until: Instant,
}

#[derive(Deserialize, Debug)]
struct Answer {
    href: String,
    #[serde(default)]
    header: BTreeMap<String, String>,
    expires_in: Option<u64>,
    expires_at: Option<String>,
}

/// What the host's answer to `git-lfs-authenticate` means: a grant, no LFS at all, or a refusal in the host's own words.
fn read_grant(out: &[u8], err: &[u8], status: Option<u32>, host: &str) -> Result<Answer> {
    if let Ok(a) = serde_json::from_slice::<Answer>(out) {
        return Ok(a);
    }
    let said = |b: &[u8]| {
        String::from_utf8_lossy(b)
            .lines()
            .map(|l| {
                l.trim()
                    .chars()
                    .filter(|c| !c.is_control())
                    .collect::<String>()
            })
            .find(|l| !l.is_empty())
    };
    let line = said(err).or_else(|| said(out));
    let unknown = [
        "unknown command",
        "invalid command",
        "unrecognized command",
        "command not found",
        "not a git command",
    ];
    let knows_no_lfs = line.as_deref().map_or(status != Some(0), |l| {
        let l = l.to_lowercase();
        status == Some(127) || unknown.iter().any(|u| l.contains(u))
    });
    match line {
        Some(m) if !knows_no_lfs => Err(Error::LfsRefused(m.chars().take(200).collect())),
        _ => Err(Error::NoLfs {
            host: host.to_string(),
        }),
    }
}

/// Seconds since the epoch of an RFC 3339 time, to the second; none when it does not read.
pub(crate) fn epoch_of(t: &str) -> Option<i64> {
    let (date, rest) = t.split_once('T')?;
    let mut d = date.split('-').map(|n| n.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let clock: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == ':')
        .collect();
    let mut c = clock.split(':').map(|n| n.parse::<i64>().ok());
    let (hh, mm, ss) = (c.next()??, c.next()??, c.next().flatten().unwrap_or(0));
    let tz = &rest[clock.len()..];
    let tz = tz.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    let offset = match tz.as_bytes().first() {
        Some(b'+') | Some(b'-') => {
            let sign = if tz.starts_with('-') { -1 } else { 1 };
            let mut p = tz[1..].split(':').map(|n| n.parse::<i64>().ok());
            sign * (p.next()?? * 3600 + p.next().flatten().unwrap_or(0) * 60)
        }
        _ => 0,
    };
    let (y2, m2) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * m2 + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + hh * 3600 + mm * 60 + ss - offset)
}

fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The path as git-lfs sends it: bare when it holds only safe characters, shell-quoted otherwise (docs/git.md).
fn lfs_path(path: &str) -> String {
    let safe = !path.is_empty()
        && path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "/._-~+@:,=".contains(c));
    if safe { path.to_string() } else { quote(path) }
}

fn authenticate_command(path: &str, op: &str) -> String {
    format!("git-lfs-authenticate {} {op}", lfs_path(path))
}

/// The grant for `url`'s repository and `op` (`upload` or `download`), asked of the server unless a live one is kept.
pub(crate) fn authenticate(
    net: &Arc<Net>,
    url: &str,
    user: Option<&str>,
    op: &str,
) -> Result<Granted> {
    let addr = parse(url)?;
    let who = user
        .map(str::to_string)
        .or_else(|| addr.user.clone())
        .unwrap_or_else(|| "git".into());
    let key = format!("{who}@{}:{}{}#{op}", addr.host, addr.port, addr.path);
    if let Some(g) = net
        .lfs_auth
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(&key)
        && g.until > Instant::now()
    {
        return Ok(g.clone());
    }
    let reply = run(net, &addr, Some(who), authenticate_command(&addr.path, op))?;
    let a = read_grant(&reply.stdout, &reply.stderr, reply.status, &addr.host)?;
    let life = match (a.expires_in, a.expires_at.as_deref().and_then(epoch_of)) {
        (Some(s), _) => s,
        (None, Some(at)) => (at - now_epoch()).max(0) as u64,
        (None, None) => 60,
    };
    let g = Granted {
        href: a.href,
        header: a.header,
        until: Instant::now() + Duration::from_secs(life.saturating_sub(5)),
    };
    net.lfs_auth
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(key, g.clone());
    Ok(g)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn say(out: &str, err: &str, status: u32) -> Result<Answer> {
        read_grant(out.as_bytes(), err.as_bytes(), Some(status), "h")
    }

    #[test]
    fn the_repository_path_is_bare_unless_it_needs_quoting() {
        assert_eq!(
            authenticate_command("fredericlb/K7-berfin.git", "upload"),
            "git-lfs-authenticate fredericlb/K7-berfin.git upload"
        );
        assert_eq!(
            authenticate_command("/srv/a b.git", "download"),
            "git-lfs-authenticate '/srv/a b.git' download"
        );
    }

    #[test]
    fn a_grant_is_read_and_a_host_that_knows_no_command_has_no_lfs() {
        assert!(say(r#"{"href":"https://h/x","header":{}}"#, "", 0).is_ok());
        assert!(matches!(say("", "", 1), Err(Error::NoLfs { .. })));
        assert!(matches!(
            say("", "Invalid command: git-lfs-authenticate", 1),
            Err(Error::NoLfs { .. })
        ));
        assert!(matches!(
            say("", "sh: git-lfs-authenticate: command not found", 127),
            Err(Error::NoLfs { .. })
        ));
    }

    #[test]
    fn a_refusal_carries_the_hosts_own_line() {
        let e = say("", "ERROR: Repository not found.\nmore", 1).unwrap_err();
        assert_eq!(e, Error::LfsRefused("ERROR: Repository not found.".into()));
        assert!(matches!(
            say("", "Permission denied", 1),
            Err(Error::LfsRefused(_))
        ));
    }

    #[test]
    fn an_rfc3339_time_is_read_to_the_second() {
        assert_eq!(epoch_of("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(epoch_of("2024-02-29T12:30:45Z"), Some(1_709_209_845));
        assert_eq!(epoch_of("2024-02-29T14:30:45+02:00"), Some(1_709_209_845));
        assert_eq!(epoch_of("2024-02-29T12:30:45.123Z"), Some(1_709_209_845));
        assert_eq!(epoch_of("tomorrow"), None);
    }
}
