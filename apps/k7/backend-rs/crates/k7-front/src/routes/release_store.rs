//! release_store.rs — the builds published for the apps and the tray, kept on disk per channel, in exactly the
//! layout the previous release reads and writes too, so a rollback finds every build in place.
//! Files arrive in `<channel>/incoming/<stamp>/`; publish moves the folder into place and points `current` at it.
//! The one thing a reader would get wrong: `current` and `previous` are plain files naming a stamp, and the
//! modification time of `current` IS the build's publication time (docs/app-downloads.md).

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use http_body_util::BodyExt;
use serde::Serialize;
use tokio::io::AsyncWriteExt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Release,
    Snapshot,
}

impl Channel {
    pub fn name(self) -> &'static str {
        match self {
            Channel::Release => "release",
            Channel::Snapshot => "snapshot",
        }
    }

    pub fn parse(name: &str) -> Option<Channel> {
        match name {
            "release" => Some(Channel::Release),
            "snapshot" => Some(Channel::Snapshot),
            _ => None,
        }
    }
}

// An upload left in incoming/ this long is a cancelled run's, never one still arriving.
const STALE_INCOMING: Duration = Duration::from_secs(6 * 3600);

fn digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// The channel a stamp publishes to (`vX.Y.Z` or `snapshot-<hex>`), or None for anything that is not a CI stamp.
pub fn channel_of(stamp: &str) -> Option<Channel> {
    if let Some(version) = stamp.strip_prefix('v') {
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() == 3 && parts.iter().all(|p| digits(p)) {
            return Some(Channel::Release);
        }
    }
    let hex = stamp.strip_prefix("snapshot-")?;
    ((7..=40).contains(&hex.len()) && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
        .then_some(Channel::Snapshot)
}

/// No spaces, slashes or leading dot: a name is a path segment and a URL segment as is.
pub fn is_file_name(name: &str) -> bool {
    let ok = |b: u8| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-');
    name.len() <= 200
        && name
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphanumeric())
        && name.bytes().all(ok)
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ReleaseFile {
    pub name: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub channel: &'static str,
    pub stamp: String,
    pub published_at: String,
    pub files: Vec<ReleaseFile>,
    #[serde(skip)]
    pub published_ms: i64,
}

/// Why an upload stopped: past the ceiling (a 413), or the body or the disk failed.
#[derive(Debug)]
pub enum SaveError {
    TooLarge(String),
    Failed,
}

/// A file time as node's `Stats.mtime` holds it: its fractional milliseconds ROUNDED, where the ETag floors them.
fn millis(t: SystemTime) -> i64 {
    t.duration_since(UNIX_EPOCH).map_or(0, |d| {
        (d.as_secs() as f64 * 1000.0 + f64::from(d.subsec_nanos()) / 1e6).round() as i64
    })
}

/// `Date.prototype.toISOString`: `2026-11-02T09:05:00.123Z`.
pub fn iso(ms: i64) -> String {
    let (days, rest) = (ms.div_euclid(86_400_000), ms.rem_euclid(86_400_000));
    let (y, m, d) = civil(days);
    let s = rest / 1000;
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{:03}Z",
        s / 3600,
        s / 60 % 60,
        s % 60,
        rest % 1000
    )
}

/// The proleptic Gregorian date of a day count since 1970-01-01 (Howard Hinnant's `civil_from_days`).
pub fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// One workflow's store under `root`, created as it is needed.
#[derive(Clone, Debug)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn new(root: PathBuf) -> Self {
        Store { root }
    }

    fn dir_of(&self, channel: Channel) -> PathBuf {
        self.root.join(channel.name())
    }

    fn incoming_of(&self, channel: Channel) -> PathBuf {
        self.dir_of(channel).join("incoming")
    }

    fn read_pointer(&self, channel: Channel, which: &str) -> Option<String> {
        let s = std::fs::read_to_string(self.dir_of(channel).join(which)).ok()?;
        let s = s.trim();
        (channel_of(s) == Some(channel)).then(|| s.to_string())
    }

    // Write then rename, so a reader sees the old pointer or the new one, never half of one.
    fn write_pointer(&self, channel: Channel, which: &str, stamp: &str) -> std::io::Result<()> {
        let target = self.dir_of(channel).join(which);
        let tmp = self.dir_of(channel).join(format!("{which}.tmp"));
        std::fs::write(&tmp, stamp)?;
        std::fs::rename(&tmp, &target)
    }

    /// Streams `body` into `incoming/<stamp>/<name>`, beside its name then renamed, refused past `max_bytes`.
    pub async fn save_file(
        &self,
        stamp: &str,
        name: &str,
        body: axum::body::Body,
        max_bytes: f64,
    ) -> Result<ReleaseFile, SaveError> {
        let channel = channel_of(stamp).ok_or(SaveError::Failed)?;
        if !is_file_name(name) {
            return Err(SaveError::Failed);
        }
        let dir = self.incoming_of(channel).join(stamp);
        std::fs::create_dir_all(&dir).map_err(|_| SaveError::Failed)?;
        let target = dir.join(name);
        let tmp = dir.join(format!("{name}.part"));
        let written = write_body(&tmp, body, max_bytes).await;
        let saved = written.and_then(|bytes| {
            std::fs::rename(&tmp, &target)
                .map(|_| bytes)
                .map_err(|_| SaveError::Failed)
        });
        let _ = std::fs::remove_file(&tmp);
        Ok(ReleaseFile {
            name: name.to_string(),
            bytes: saved?,
        })
    }

    /// Makes the build uploaded for `stamp` its channel's current one; None when nothing was uploaded for it.
    pub fn publish(&self, stamp: &str) -> std::io::Result<Option<Release>> {
        let Some(channel) = channel_of(stamp) else {
            return Ok(None);
        };
        let from = self.incoming_of(channel).join(stamp);
        let uploaded = std::fs::read_dir(&from).is_ok_and(|mut d| d.next().is_some());
        if !uploaded {
            return Ok(None);
        }
        let dir = self.dir_of(channel);
        let before = self.read_pointer(channel, "current");
        let to = dir.join(stamp);
        remove_any(&to);
        std::fs::rename(&from, &to)?;
        self.write_pointer(channel, "current", stamp)?;
        if let Some(before) = before.filter(|b| b != stamp) {
            self.write_pointer(channel, "previous", &before)?;
        }
        let previous = self.read_pointer(channel, "previous");
        for entry in std::fs::read_dir(&dir)?.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let kept = name == "incoming" || name == stamp || previous.as_deref() == Some(&name);
            if entry.file_type().is_ok_and(|t| t.is_dir()) && !kept {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
        // Snapshot stamps do not sort, so a cancelled run's leftovers are told by their age.
        let now = SystemTime::now();
        for entry in std::fs::read_dir(self.incoming_of(channel))?.flatten() {
            let age = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| now.duration_since(t).ok());
            if age.is_some_and(|a| a > STALE_INCOMING) {
                remove_any(&entry.path());
            }
        }
        Ok(self.current(channel))
    }

    /// The build a channel serves, its files sorted by name.
    pub fn current(&self, channel: Channel) -> Option<Release> {
        let stamp = self.read_pointer(channel, "current")?;
        let dir = self.dir_of(channel).join(&stamp);
        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .ok()?
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| is_file_name(n))
            .collect();
        names.sort();
        let mut files = Vec::with_capacity(names.len());
        for name in names {
            let bytes = std::fs::metadata(dir.join(&name)).ok()?.len();
            files.push(ReleaseFile { name, bytes });
        }
        let published = std::fs::metadata(self.dir_of(channel).join("current"))
            .and_then(|m| m.modified())
            .ok()?;
        let published_ms = millis(published);
        Some(Release {
            channel: channel.name(),
            stamp,
            published_at: iso(published_ms),
            files,
            published_ms,
        })
    }

    /// A file of the channel's current build, else of its previous one.
    pub fn file_path(&self, channel: Channel, name: &str) -> Option<PathBuf> {
        if !is_file_name(name) {
            return None;
        }
        ["current", "previous"].iter().find_map(|which| {
            let stamp = self.read_pointer(channel, which)?;
            let file = self.dir_of(channel).join(stamp).join(name);
            file.is_file().then_some(file)
        })
    }
}

fn remove_any(path: &Path) {
    if path.is_dir() {
        let _ = std::fs::remove_dir_all(path);
    } else {
        let _ = std::fs::remove_file(path);
    }
}

async fn write_body(
    tmp: &Path,
    mut body: axum::body::Body,
    max_bytes: f64,
) -> Result<u64, SaveError> {
    let mut file = tokio::fs::File::create(tmp)
        .await
        .map_err(|_| SaveError::Failed)?;
    let mut bytes: u64 = 0;
    while let Some(frame) = body.frame().await {
        let Ok(data) = frame.map_err(|_| SaveError::Failed)?.into_data() else {
            continue;
        };
        bytes += data.len() as u64;
        if bytes as f64 > max_bytes {
            return Err(SaveError::TooLarge(format!(
                "file exceeds {} bytes",
                js_number(max_bytes)
            )));
        }
        file.write_all(&data).await.map_err(|_| SaveError::Failed)?;
    }
    file.flush().await.map_err(|_| SaveError::Failed)?;
    Ok(bytes)
}

/// A number as JavaScript prints it, for the integers and simple fractions a ceiling is.
fn js_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e21 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_and_names_follow_the_ts_patterns() {
        assert_eq!(channel_of("v0.1.0"), Some(Channel::Release));
        assert_eq!(channel_of("v10.20.30"), Some(Channel::Release));
        assert_eq!(channel_of("snapshot-9f2c1ab"), Some(Channel::Snapshot));
        for bad in [
            "v1.2",
            "v1.2.3.4",
            "v1..2",
            "1.2.3",
            "snapshot-9F2C1AB",
            "snapshot-abc",
            "tray",
            "v1.2.3\n",
        ] {
            assert_eq!(channel_of(bad), None, "{bad:?}");
        }
        assert!(channel_of(&format!("snapshot-{}", "a".repeat(40))).is_some());
        assert!(channel_of(&format!("snapshot-{}", "a".repeat(41))).is_none());
        assert!(is_file_name("k7_v0.1.0_windows-x64.exe") && is_file_name("SHA256SUMS"));
        for bad in [".hidden", "a b", "a/b", "", "-x", "é"] {
            assert!(!is_file_name(bad), "{bad:?}");
        }
        assert!(is_file_name(&"a".repeat(200)) && !is_file_name(&"a".repeat(201)));
    }

    #[test]
    fn dates_read_as_javascript_writes_them() {
        assert_eq!(iso(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso(1_793_610_300_123), "2026-11-02T09:05:00.123Z");
        assert_eq!(iso(951_782_400_000), "2000-02-29T00:00:00.000Z");
        let at = |nanos| UNIX_EPOCH + Duration::new(1_793_610_300, nanos);
        assert_eq!(
            millis(at(134_600_000)),
            1_793_610_300_135,
            "rounded, as node's Stats.mtime"
        );
        assert_eq!(millis(at(134_400_000)), 1_793_610_300_134);
        assert_eq!(js_number(64.0), "64");
        assert_eq!(js_number(1_572_864.0), "1572864");
    }
}
