//! encode.rs — a mixdown's MP3 encode by the account's ffmpeg (2523).
//! What a reader gets wrong: every child runs under the account's environment, never the front's own.

use std::path::Path;
use std::time::Duration;

use tokio::process::Command;

use super::Account;

/// A mixdown's encode is seconds of work; past this the encoder is stuck and is stopped.
pub const ENCODE_LIMIT: Duration = Duration::from_secs(10 * 60);

/// The WAV as a ~190 kbit/s MP3 by ffmpeg; `Ok(false)` when the account has none (`toMp3`).
pub async fn mp3(acct: &Account, wav: &Path, out: &Path) -> Result<bool, String> {
    if k7_mediatek::programs::NONE {
        return Ok(false);
    }
    let named = acct.var("FFMPEG_BIN").filter(|v| !v.is_empty());
    let Some(bin) = super::disk::which(
        named.as_deref().unwrap_or("ffmpeg"),
        acct.var("PATH").as_deref(),
    ) else {
        return Ok(false);
    };
    let mut cmd = Command::new(bin);
    cmd.env_clear()
        .envs(acct.env_pairs())
        .args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-protocol_whitelist",
            "file,pipe",
        ])
        .args(["-format_whitelist", k7_mediatek::ffmpeg::FORMATS, "-i"])
        .arg(format!("file:{}", wav.display()))
        .args(["-c:a", "libmp3lame", "-q:a", "2"])
        .arg(out)
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    let run = tokio::time::timeout(ENCODE_LIMIT, cmd.output()).await;
    match run {
        Err(_) => Err("mp3 encode timed out".into()),
        Ok(Err(e)) => Err(format!("mp3 encode: {e}")),
        Ok(Ok(o)) if !o.status.success() => Err(format!(
            "mp3 encode failed: {}",
            String::from_utf8_lossy(&o.stderr).trim()
        )),
        Ok(Ok(_)) => Ok(out.exists()),
    }
}
