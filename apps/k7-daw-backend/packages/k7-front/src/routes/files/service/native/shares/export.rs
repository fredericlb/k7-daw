//! export.rs — `/export`: a project's mixdown rendered into a private folder and streamed, the folder gone when the body is (2520 to 2524).
//! What a reader gets wrong: the folder belongs to `Mixed`; dropping it, unread, hung up or failed, is the whole cleanup.

use std::path::{Path, PathBuf};

use axum::body::Body;
use axum::response::Response;
use hyper::body::{Bytes, Frame};
use hyper::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE, HeaderValue};
use k7_file_service::shares::names::content_disposition;
use k7_file_service::shares::{MP3_NEEDS_FFMPEG, names};
use tokio::io::AsyncReadExt;

use super::disk::Folder;
use super::{Account, Ports, encode};

const CHUNK: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Wav,
    Mp3,
}

/// A rendered mixdown and the folder that holds it.
pub struct Mixed {
    pub path: PathBuf,
    pub filename: String,
    pub mime: &'static str,
    folder: Folder,
}

impl Mixed {
    /// A file already written in `folder`, which goes when the body does.
    pub fn packed(path: PathBuf, filename: String, mime: &'static str, folder: Folder) -> Mixed {
        Mixed {
            path,
            filename,
            mime,
            folder,
        }
    }

    /// The download answer: the file streamed under its name, the folder gone with the body.
    pub async fn into_response(self) -> Result<Response, String> {
        let (filename, mime) = (self.filename.clone(), self.mime);
        let (body, len) = self.into_body().await?;
        let mut r = Response::new(body);
        let h = r.headers_mut();
        h.insert(CONTENT_TYPE, HeaderValue::from_static(mime));
        if let Ok(v) = HeaderValue::from_str(&content_disposition(&filename)) {
            h.insert(CONTENT_DISPOSITION, v);
        }
        h.insert(CONTENT_LENGTH, HeaderValue::from(len));
        Ok(r)
    }

    /// The file's length and its body, read in 256 KiB chunks; the folder is removed when the body ends or is dropped (2524).
    pub async fn into_body(self) -> Result<(Body, u64), String> {
        let file = tokio::fs::File::open(&self.path)
            .await
            .map_err(|e| e.to_string())?;
        let len = file.metadata().await.map_err(|e| e.to_string())?.len();
        let stream =
            futures_util::stream::unfold((file, self.folder), |(mut f, folder)| async move {
                let mut buf = vec![0u8; CHUNK];
                match f.read(&mut buf).await {
                    Ok(0) => None,
                    Ok(n) => {
                        buf.truncate(n);
                        Some((
                            Ok::<_, std::io::Error>(Frame::data(Bytes::from(buf))),
                            (f, folder),
                        ))
                    }
                    Err(e) => Some((Err(e), (f, folder))),
                }
            });
        Ok((Body::new(http_body_util::StreamBody::new(stream)), len))
    }
}

/// Render the mixdown of the project at `dir`, or of one `timeline` of it, in a folder of its own.
pub async fn export_mix(
    acct: &Account,
    ports: &Ports,
    dir: &Path,
    format: Format,
    timeline: Option<&str>,
) -> Result<Mixed, String> {
    if !ports.mixdown.available() {
        return Err(k7_file_service::shares::no_engine("mixdown"));
    }
    let mixdoc = ports.views.mix_doc(dir, timeline).await?;
    let folder = Folder::make(&acct.tmp, "tbmix-").map_err(|e| e.to_string())?;
    let manifest = ports
        .mixdown
        .render(&mixdoc.doc, folder.path(), false)
        .await?;
    let wav = folder.path().join(&manifest.mix);
    let name = names::mix_name(&mixdoc.name, &dir.to_string_lossy(), timeline);
    let (path, ext, mime) = match format {
        Format::Wav => (wav, "wav", "audio/wav"),
        Format::Mp3 => {
            let out = folder.path().join("mix.mp3");
            if !encode::mp3(acct, &wav, &out).await? {
                return Err(if k7_mediatek::programs::NONE {
                    format!(
                        "mp3 needs ffmpeg, and {}: use Export WAV",
                        k7_mediatek::programs::WHY
                    )
                } else {
                    MP3_NEEDS_FFMPEG.into()
                });
            }
            (out, "mp3", "audio/mpeg")
        }
    };
    Ok(Mixed {
        path,
        filename: format!("{name}.{ext}"),
        mime,
        folder,
    })
}
