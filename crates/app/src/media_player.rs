//! Same-origin, bounded video reads. No file paths or bearer credentials enter the DOM.
use anyhow::{bail, Context, Result};
#[cfg(feature = "desktop")]
use dioxus::desktop as native;
#[cfg(all(feature = "mobile", not(feature = "desktop")))]
use dioxus::mobile as native;
use dioxus::prelude::*;
use native::wry::http::{Request, Response};

const CHUNK: u64 = 1024 * 1024;

#[derive(Clone)]
pub enum Location {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    File(std::path::PathBuf),
    Remote(reqwest::Client, String),
    #[cfg(any(target_os = "android", target_os = "ios"))]
    Document(String, String),
}

#[derive(Clone)]
pub struct Media {
    pub location: Location,
    pub size: u64,
    pub mime: &'static str,
    pub id: String,
}

pub fn mime(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "ts" | "m2ts" => "video/mp2t",
        _ => "video/x-msvideo",
    }
}

fn range(value: Option<&str>, size: u64) -> Result<(u64, u64)> {
    if size == 0 {
        bail!("Empty media file");
    }
    let (start, end) = match value {
        None => (0, size - 1),
        Some(value) => {
            let (a, b) = value
                .strip_prefix("bytes=")
                .context("Invalid range")?
                .split_once('-')
                .context("Invalid range")?;
            if b.contains(',') {
                bail!("Multiple ranges unsupported");
            }
            if a.is_empty() {
                let count = b.parse::<u64>()?.min(size);
                if count == 0 {
                    bail!("Invalid suffix");
                }
                (size - count, size - 1)
            } else {
                (
                    a.parse()?,
                    if b.is_empty() {
                        size - 1
                    } else {
                        b.parse::<u64>()?.min(size - 1)
                    },
                )
            }
        }
    };
    if start >= size || start > end {
        bail!("Range unavailable");
    }
    Ok((start, end.min(start.saturating_add(CHUNK - 1))))
}

impl Media {
    async fn bytes(&self, start: u64, end: u64) -> Result<Vec<u8>> {
        let count = (end - start + 1) as usize;
        match &self.location {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            Location::File(path) => {
                use std::io::{Read, Seek};
                let path = path.clone();
                tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
                    let mut file = std::fs::File::open(path)?;
                    file.seek(std::io::SeekFrom::Start(start))?;
                    let mut data = vec![0; count];
                    file.read_exact(&mut data)?;
                    Ok(data)
                })
                .await?
            }
            Location::Remote(client, url) => {
                let mut response = client
                    .get(url)
                    .header("Range", format!("bytes={start}-{end}"))
                    .send()
                    .await?;
                if response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
                    bail!("Server cannot seek this stream");
                }
                let expected = format!("bytes {start}-{end}/{}", self.size);
                if response
                    .headers()
                    .get("content-range")
                    .and_then(|h| h.to_str().ok())
                    != Some(expected.as_str())
                {
                    bail!("Invalid media range");
                }
                let mut bytes = Vec::with_capacity(count);
                while let Some(chunk) = response.chunk().await? {
                    if bytes.len() + chunk.len() > count {
                        bail!("Media response exceeded range");
                    }
                    bytes.extend_from_slice(&chunk);
                }
                if bytes.len() != count {
                    bail!("Incomplete media response");
                }
                Ok(bytes)
            }
            #[cfg(any(target_os = "android", target_os = "ios"))]
            Location::Document(root, path) => {
                crate::mobile_storage::read_media(root, path, start, count)
                    .await
                    .map(|(_, bytes)| bytes)
            }
        }
    }

    pub(crate) async fn response(&self, request: Request<Vec<u8>>) -> Result<Response<Vec<u8>>> {
        if !matches!(request.method().as_str(), "GET" | "HEAD") {
            return Ok(Response::builder().status(405).body(vec![])?);
        }
        let range_header = request
            .headers()
            .get("range")
            .map(|h| h.to_str())
            .transpose()?;
        let Ok((start, end)) = range(range_header, self.size) else {
            return Ok(Response::builder()
                .status(416)
                .header("Content-Range", format!("bytes */{}", self.size))
                .body(vec![])?);
        };
        let bytes = if request.method() == "HEAD" {
            vec![]
        } else {
            self.bytes(start, end).await?
        };
        Ok(Response::builder()
            .status(206)
            .header("Content-Type", self.mime)
            .header("Accept-Ranges", "bytes")
            .header(
                "Content-Range",
                format!("bytes {start}-{end}/{}", self.size),
            )
            .header("Content-Length", end - start + 1)
            .header("Cache-Control", "no-store")
            .header("Referrer-Policy", "no-referrer")
            .body(bytes)?)
    }
}

/// A fresh opaque ID revokes the previous selection's URL, including queued reads.
pub fn use_media_handler(media: Signal<Option<Media>>) {
    native::use_asset_handler("media-preview", move |request, responder| {
        let current = media.read().clone();
        spawn(async move {
            let response = match current {
                Some(value) if request.uri().path() == format!("/media-preview/{}", value.id) => {
                    value.response(request).await
                }
                _ => Response::builder()
                    .status(404)
                    .body(vec![])
                    .map_err(Into::into),
            };
            responder.respond(
                response.unwrap_or_else(|_| Response::builder().status(503).body(vec![]).unwrap()),
            );
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seeks_are_bounded_and_invalid_ranges_are_rejected() {
        assert_eq!(range(None, 8_000_000).unwrap(), (0, CHUNK - 1));
        assert_eq!(range(Some("bytes=20-39"), 100).unwrap(), (20, 39));
        assert_eq!(range(Some("bytes=-10"), 100).unwrap(), (90, 99));
        for r in [
            "bytes=100-",
            "bytes=8-2",
            "bytes=-0",
            "bytes=0-2,4-6",
            "bad",
        ] {
            assert!(range(Some(r), 100).is_err());
        }
        assert!(range(None, 0).is_err());
    }
    #[tokio::test]
    async fn reads_selected_file_ranges_without_loading_the_whole_video() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), b"0123456789").unwrap();
        let media = Media {
            location: Location::File(file.path().into()),
            size: 10,
            mime: "video/mp4",
            id: "test".into(),
        };
        let response = media
            .response(
                Request::builder()
                    .header("Range", "bytes=3-6")
                    .body(vec![])
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 206);
        assert_eq!(response.headers()["Content-Range"], "bytes 3-6/10");
        assert_eq!(response.body(), b"3456");
    }
}
