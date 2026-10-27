use anyhow::{bail, Context, Result};
use std::time::Duration;
use tv_manager_core::{rename::RenamePlan, PlanRequest};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use {std::path::PathBuf, tv_manager_core::rename};

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Connection {
    pub remote: bool,
    pub root: String,
    pub url: String,
    pub token: String,
    pub certificate: String,
}

impl Default for Connection {
    fn default() -> Self {
        Self {
            remote: false,
            root: String::new(),
            url: "http://127.0.0.1:8787".into(),
            token: String::new(),
            certificate: String::new(),
        }
    }
}

impl Connection {
    fn client(&self) -> Result<(reqwest::Client, reqwest::Url)> {
        let url = reqwest::Url::parse(self.url.trim()).context("Enter a valid server URL")?;
        let loopback = matches!(
            url.host_str(),
            Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
        );
        if url.scheme() != "https" && !(url.scheme() == "http" && loopback) {
            bail!("Remote connections require HTTPS (HTTP is allowed on localhost)");
        }
        if !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            bail!("Server URL must not contain credentials, query parameters, or fragments");
        }
        if self.token.trim().is_empty() {
            bail!("Enter the server bearer token");
        }
        let mut builder = reqwest::Client::builder();
        if !self.certificate.is_empty() {
            use base64::Engine;
            let der = base64::engine::general_purpose::STANDARD
                .decode(&self.certificate)
                .context("Invalid desktop pairing certificate")?;
            builder = builder
                .tls_built_in_root_certs(false)
                .add_root_certificate(reqwest::Certificate::from_der(&der)?);
        }
        let client = builder
            .timeout(Duration::from_secs(180))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok((client, url))
    }

    /// Check access without scanning media or requesting metadata.
    pub async fn validate(&self) -> Result<()> {
        if !self.remote {
            anyhow::ensure!(
                !self.root.trim().is_empty(),
                "Choose a media folder to continue."
            );
            #[cfg(any(target_os = "android", target_os = "ios"))]
            return crate::mobile_storage::check_folder(&self.root).await;
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                let root = self.root.clone();
                return tokio::task::spawn_blocking(move || -> Result<()> {
                    let path = std::path::Path::new(&root);
                    anyhow::ensure!(
                        path.is_dir(),
                        "The media folder is unavailable. Choose an existing folder."
                    );
                    std::fs::read_dir(path)
                        .context("The media folder cannot be read. Check its permissions.")?
                        .next()
                        .transpose()?;
                    Ok(())
                })
                .await?;
            }
        }
        let (client, url) = self.client()?;
        let mut response = client
            .get(url.join("/v1/health")?)
            .bearer_auth(&self.token)
            .timeout(Duration::from_secs(8))
            .send()
            .await
            .map_err(|_| {
                anyhow::anyhow!(
                    "Cannot reach the server. Check its address and network connection."
                )
            })?;
        anyhow::ensure!(
            response.status().is_success(),
            "The server refused the connection (HTTP {}). Check your bearer token.",
            response.status()
        );
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            anyhow::ensure!(
                bytes.len() + chunk.len() <= 65536,
                "Unexpected server response."
            );
            bytes.extend_from_slice(&chunk);
        }
        let health: serde_json::Value = serde_json::from_slice(&bytes)
            .context("This address did not return a Media Manager service.")?;
        anyhow::ensure!(
            health["service"] == "tv-manager",
            "This address is not a Media Manager server."
        );
        Ok(())
    }

    pub async fn preview(&self, request: &PlanRequest) -> Result<RenamePlan> {
        if !self.remote {
            #[cfg(any(target_os = "android", target_os = "ios"))]
            return crate::mobile_storage::preview(&self.root, request).await;
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                if self.root.trim().is_empty() {
                    bail!("Select a media root first");
                }
                return rename::preview(&PathBuf::from(&self.root), request).await;
            }
        }
        let (client, url) = self.client()?;
        decode(
            client
                .post(url.join("/v1/plans")?)
                .bearer_auth(&self.token)
                .json(request)
                .send()
                .await
                .map_err(|_| anyhow::anyhow!("Could not connect to server"))?,
        )
        .await
    }

    pub async fn apply(&self, plan: RenamePlan) -> Result<RenamePlan> {
        if !self.remote {
            #[cfg(any(target_os = "android", target_os = "ios"))]
            return crate::mobile_storage::apply(plan).await;
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                let mut plan = tokio::task::spawn_blocking(move || -> Result<RenamePlan> {
                    let mut plan = plan;
                    rename::apply(&mut plan)?;
                    Ok(plan)
                })
                .await??;
                tv_manager_core::posters::download(&mut plan).await;
                return tokio::task::spawn_blocking(move || {
                    tv_manager_core::history::finish(&plan)?;
                    Ok(plan)
                })
                .await?;
            }
        }
        let (client, url) = self.client()?;
        decode(
            client
                .post(url.join(&format!("/v1/plans/{}/apply", plan.id))?)
                .bearer_auth(&self.token)
                .send()
                .await
                .map_err(|_| {
                    anyhow::anyhow!(
                        "Connection interrupted; inspect folder or rebuild preview before retrying"
                    )
                })?,
        )
        .await
    }

    pub async fn edit(
        &self,
        mut plan: RenamePlan,
        index: usize,
        filename: String,
    ) -> Result<RenamePlan> {
        if !self.remote {
            tv_manager_core::rename::edit_target(&mut plan, index, &filename)?;
            return Ok(plan);
        }
        let (client, url) = self.client()?;
        decode(
            client
                .patch(url.join(&format!("/v1/plans/{}/tasks/{index}", plan.id))?)
                .bearer_auth(&self.token)
                .json(&tv_manager_core::rename::FilenameEdit { filename })
                .send()
                .await
                .map_err(|_| anyhow::anyhow!("Could not update the remote preview"))?,
        )
        .await
    }

    pub async fn search(&self, request: &PlanRequest) -> Result<Vec<tv_manager_core::ShowMatch>> {
        if !self.remote {
            return tv_manager_core::metadata::search(request).await;
        }
        let (client, url) = self.client()?;
        decode(
            client
                .post(url.join("/v1/search")?)
                .bearer_auth(&self.token)
                .json(request)
                .send()
                .await?,
        )
        .await
    }
    pub async fn history(
        &self,
        folder: &str,
    ) -> Result<Vec<tv_manager_core::history::HistoryRecord>> {
        if !self.remote {
            #[cfg(any(target_os = "android", target_os = "ios"))]
            return crate::mobile_storage::history(&self.root, folder).await;
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                let root = rename::scoped_folder(&PathBuf::from(&self.root), folder)?;
                return tokio::task::spawn_blocking(move || tv_manager_core::history::list(&root))
                    .await?;
            }
        }
        let (client, url) = self.client()?;
        decode(
            client
                .post(url.join("/v1/history")?)
                .bearer_auth(&self.token)
                .json(&serde_json::json!({"folder":folder}))
                .send()
                .await?,
        )
        .await
    }
    pub async fn undo(
        &self,
        folder: &str,
        id: uuid::Uuid,
    ) -> Result<tv_manager_core::history::HistoryRecord> {
        if !self.remote {
            #[cfg(any(target_os = "android", target_os = "ios"))]
            return crate::mobile_storage::undo(&self.root, folder, id).await;
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                let root = rename::scoped_folder(&PathBuf::from(&self.root), folder)?;
                return tokio::task::spawn_blocking(move || {
                    tv_manager_core::history::undo(&root, id)
                })
                .await?;
            }
        }
        let (client, url) = self.client()?;
        decode(
            client
                .post(url.join(&format!("/v1/history/{id}/undo"))?)
                .bearer_auth(&self.token)
                .json(&serde_json::json!({"folder":folder}))
                .send()
                .await?,
        )
        .await
    }
    pub async fn artwork(&self, plan: RenamePlan) -> Result<Vec<tv_manager_core::artwork::Poster>> {
        if !self.remote {
            #[cfg(any(target_os = "android", target_os = "ios"))]
            return crate::mobile_storage::artwork(&plan).await;
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            return tokio::task::spawn_blocking(move || tv_manager_core::artwork::inventory(&plan))
                .await?;
        }
        let (client, url) = self.client()?;
        decode(
            client
                .post(url.join(&format!("/v1/plans/{}/artwork", plan.id))?)
                .bearer_auth(&self.token)
                .send()
                .await?,
        )
        .await
    }
    pub async fn download_artwork(
        &self,
        plan: RenamePlan,
    ) -> Result<tv_manager_core::artwork::Batch> {
        if !self.remote {
            #[cfg(any(target_os = "android", target_os = "ios"))]
            return crate::mobile_storage::download_artwork(&plan).await;
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            return tv_manager_core::artwork::download_missing(&plan).await;
        }
        let (client, url) = self.client()?;
        decode(
            client
                .post(url.join(&format!("/v1/plans/{}/artwork/download", plan.id))?)
                .timeout(std::time::Duration::from_secs(1800))
                .bearer_auth(&self.token)
                .send()
                .await?,
        )
        .await
    }

    pub async fn save_artwork(
        &self,
        plan: RenamePlan,
        save: tv_manager_core::artwork::Save,
    ) -> Result<Vec<tv_manager_core::artwork::Poster>> {
        if !self.remote {
            #[cfg(any(target_os = "android", target_os = "ios"))]
            return crate::mobile_storage::save_artwork(&plan, &save).await;
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                tv_manager_core::artwork::validate_save(&plan, &save)?;
                let bytes = tv_manager_core::artwork::fetch(&save.url).await?;
                return tokio::task::spawn_blocking(move || {
                    tv_manager_core::artwork::write(&plan, &save, &bytes)?;
                    tv_manager_core::artwork::inventory(&plan)
                })
                .await?;
            }
        }
        let (client, url) = self.client()?;
        decode(
            client
                .post(url.join(&format!("/v1/plans/{}/artwork/save", plan.id))?)
                .bearer_auth(&self.token)
                .json(&save)
                .send()
                .await?,
        )
        .await
    }

    pub async fn health(&self, plan: RenamePlan) -> Result<tv_manager_core::health::HealthReport> {
        if !self.remote {
            #[cfg(any(target_os = "android", target_os = "ios"))]
            return crate::mobile_storage::health(&self.root, &plan).await;
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            return tokio::task::spawn_blocking(move || tv_manager_core::health::scan(&plan))
                .await?;
        }
        let (client, url) = self.client()?;
        decode(
            client
                .post(url.join(&format!("/v1/plans/{}/health", plan.id))?)
                .bearer_auth(&self.token)
                .send()
                .await?,
        )
        .await
    }
    #[cfg(any(not(any(target_os = "android", target_os = "ios")), test))]
    pub async fn pairing_code(&self) -> Result<String> {
        let (client, url) = self.client()?;
        let value: serde_json::Value = decode(
            client
                .post(url.join("/v1/pairing")?)
                .bearer_auth(&self.token)
                .send()
                .await?,
        )
        .await?;
        Ok(value["code"]
            .as_str()
            .context("Missing pairing code")?
            .into())
    }
    #[cfg(any(target_os = "android", target_os = "ios", test))]
    pub async fn redeem_pairing(invitation: crate::pairing::Invitation) -> Result<Self> {
        let code = invitation.code;
        // Reuse URL validation without ever transmitting the placeholder credential.
        let connection = Self {
            remote: true,
            url: invitation.url,
            certificate: invitation.certificate,
            token: "pairing-validation".into(),
            ..Default::default()
        };
        let (client, url) = connection.client()?;
        let value: serde_json::Value = decode(
            client
                .post(url.join("/v1/pairing/redeem")?)
                .json(&serde_json::json!({"code":code}))
                .send()
                .await?,
        )
        .await?;
        Ok(Self {
            token: value["token"]
                .as_str()
                .context("Missing pairing credential")?
                .into(),
            ..connection
        })
    }

    pub async fn preview_media(
        &self,
        plan: RenamePlan,
        index: usize,
    ) -> Result<crate::media_player::Media> {
        use crate::media_player::{Location, Media};
        let path = if self.remote {
            plan.tasks
                .get(index)
                .context("File selection is unavailable")?
                .source
                .clone()
        } else {
            tv_manager_core::rename::playback_path(&plan, index)?
        };
        let mime = crate::media_player::mime(&path);
        let id = uuid::Uuid::new_v4().to_string();
        if !self.remote {
            #[cfg(any(target_os = "android", target_os = "ios"))]
            {
                let (root, path) = crate::mobile_storage::media_document(&plan, index)?;
                let (size, _) = crate::mobile_storage::read_media(&root, &path, 0, 0).await?;
                return Ok(Media {
                    location: Location::Document(root, path),
                    size,
                    mime,
                    id,
                });
            }
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                let size = std::fs::metadata(&path)?.len();
                return Ok(Media {
                    location: Location::File(path),
                    size,
                    mime,
                    id,
                });
            }
        }
        let (client, url) = self.client()?;
        let response = client
            .post(url.join(&format!("/v1/plans/{}/tasks/{index}/play", plan.id))?)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|_| anyhow::anyhow!("Could not connect to the media server"))?;
        if !response.status().is_success() {
            bail!(
                "Could not start playback (HTTP {}). Refresh the library preview and try again.",
                response.status()
            );
        }
        let value: serde_json::Value =
            response.json().await.context("Invalid playback response")?;
        let ticket = value["ticket"]
            .as_str()
            .context("Missing playback ticket")?;
        if ticket.len() != 36 || !ticket.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
            bail!("Invalid playback ticket");
        }
        let stream = url.join(&format!("/v1/media/{ticket}"))?.to_string();
        let response = client.head(&stream).send().await?.error_for_status()?;
        let size = response
            .headers()
            .get("content-length")
            .context("Missing media size")?
            .to_str()?
            .parse()?;
        Ok(Media {
            location: Location::Remote(client, stream),
            size,
            mime,
            id,
        })
    }
}

async fn decode<T: serde::de::DeserializeOwned>(response: reqwest::Response) -> Result<T> {
    if !response.status().is_success() {
        let status = response.status();
        let value: serde_json::Value = response.json().await.unwrap_or_default();
        bail!(
            "{}",
            value["error"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("Server returned HTTP {status}"))
        );
    }
    response.json().await.context("Unexpected server response")
}

#[cfg(test)]
mod local_pairing_tests {
    #[tokio::test]
    async fn download_all_keeps_existing_posters_without_fetching_or_renaming_media() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let video = root.join("01.mkv");
        std::fs::write(&video, b"original video").unwrap();
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image::RgbImage::new(2, 3))
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        std::fs::write(root.join("poster.png"), bytes.get_ref()).unwrap();
        let show = tv_manager_core::Show {
            name: "Movie".into(),
            year: None,
            episodes: Vec::new(),
            artwork: tv_manager_core::Artwork {
                poster: Some("https://example.invalid/poster.png".into()),
                ..Default::default()
            },
        };
        let request = tv_manager_core::PlanRequest {
            movie: true,
            ..Default::default()
        };
        let plan = tv_manager_core::rename::build_plan(
            &root,
            &request,
            show,
            std::slice::from_ref(&video),
        )
        .unwrap();
        let result = super::Connection {
            root: root.to_string_lossy().into(),
            ..Default::default()
        }
        .download_artwork(plan)
        .await
        .unwrap();
        assert_eq!(result.downloaded, 0);
        assert_eq!(result.kept, 1);
        assert!(result.failures.is_empty());
        assert_eq!(
            std::fs::read(root.join("poster.png")).unwrap(),
            *bytes.get_ref()
        );
        assert_eq!(std::fs::read(video).unwrap(), b"original video");
    }
    use super::*;
    use base64::Engine;
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "Manual simulated-camera harness: starts a temporary LAN host with disposable data"]
    async fn camera_pairing_fixture_host() {
        let ip: std::net::IpAddr = std::env::var("MEDIA_MANAGER_CAMERA_HOST_IP")
            .expect("Set the test PC LAN IP")
            .parse()
            .unwrap();
        assert!(crate::pairing::is_local(ip));
        let output = std::path::PathBuf::from(
            std::env::var("MEDIA_MANAGER_CAMERA_IMAGE").expect("Set a test camera PNG output path"),
        );
        let folder = tempfile::tempdir().unwrap();
        std::fs::write(
            folder.path().join("camera-test.mkv"),
            b"Disposable camera pairing fixture; not playable media",
        )
        .unwrap();
        let host = tv_manager_server::host_library(folder.path().into(), ip)
            .await
            .unwrap();
        let owner = Connection {
            remote: true,
            url: host.url.clone(),
            token: host.token.clone(),
            certificate: base64::engine::general_purpose::STANDARD.encode(&host.certificate),
            ..Connection::default()
        };
        let invite = crate::pairing::Invitation {
            version: 2,
            url: owner.url.clone(),
            code: owner.pairing_code().await.unwrap(),
            certificate: owner.certificate.clone(),
            desktop: "Disposable camera test".into(),
            mode: "local".into(),
        };
        let text = serde_json::to_string(&invite).unwrap();
        let code = qrcode::QrCode::new(text).unwrap();
        let width = code.width();
        let side = (width + 8) * 2;
        let image = image::RgbImage::from_fn(1280, 720, |x, y| {
            // The tested Android imagefile camera exposes a centered 720px source plane.
            let left = (720 - side) / 2;
            let top = (720 - side) / 2;
            let x = x as usize;
            let y = y as usize;
            let dark = if x >= left && y >= top && x < left + side && y < top + side {
                let cx = (x - left) / 2;
                let cy = (y - top) / 2;
                cx >= 4
                    && cy >= 4
                    && cx < width + 4
                    && cy < width + 4
                    && code[(cx - 4, cy - 4)] == qrcode::Color::Dark
            } else {
                false
            };
            image::Rgb([if dark { 0 } else { 255 }; 3])
        });
        image.save(output).unwrap();
        println!(
            "Disposable TLS camera pairing host ready at {} for 300 seconds",
            owner.url
        );
        tokio::time::sleep(Duration::from_secs(300)).await;
    }

    #[tokio::test]
    async fn connection_validation_checks_local_folder_access_without_scanning_media() {
        assert!(Connection::default().validate().await.is_err());
        let folder = tempfile::tempdir().unwrap();
        let mut connection = Connection {
            root: folder.path().to_string_lossy().into(),
            ..Default::default()
        };
        connection.validate().await.unwrap();
        let file = folder.path().join("file.mp4");
        std::fs::write(&file, b"test").unwrap();
        connection.root = file.to_string_lossy().into();
        assert!(connection.validate().await.is_err());
        connection.root = folder.path().join("missing").to_string_lossy().into();
        assert!(connection.validate().await.is_err());
        assert_eq!(std::fs::read(file).unwrap(), b"test");
    }

    #[tokio::test]
    async fn desktop_tls_pairing_requires_qr_certificate_and_is_single_use() {
        let folder = tempfile::tempdir().unwrap();
        let host =
            tv_manager_server::host_library(folder.path().into(), "127.0.0.1".parse().unwrap())
                .await
                .unwrap();
        let owner = Connection {
            remote: true,
            url: host.url.clone(),
            token: host.token.clone(),
            certificate: base64::engine::general_purpose::STANDARD.encode(&host.certificate),
            ..Connection::default()
        };
        owner.validate().await.unwrap();
        let unauthorized = Connection {
            token: "incorrect-token".into(),
            ..owner.clone()
        };
        assert!(unauthorized.validate().await.is_err());
        std::fs::write(folder.path().join("Test.S01E01.mp4"), b"0123456789").unwrap();
        let plan = owner
            .preview(&PlanRequest {
                name: "Test".into(),
                season: Some(1),
                source: tv_manager_core::Source::Manual {
                    titles: "Pilot".into(),
                },
                ..Default::default()
            })
            .await
            .unwrap();
        let media = owner.preview_media(plan, 0).await.unwrap();
        let response = media
            .response(
                dioxus::desktop::wry::http::Request::builder()
                    .header("Range", "bytes=2-5")
                    .body(vec![])
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 206);
        assert_eq!(response.body(), b"2345");
        let code = owner.pairing_code().await.unwrap();
        let invite = crate::pairing::Invitation {
            version: 2,
            url: owner.url.clone(),
            code,
            certificate: owner.certificate.clone(),
            desktop: "Test desktop".into(),
            mode: "local".into(),
        };
        let encoded = serde_json::to_string(&invite).unwrap();
        assert!(crate::pairing::qr_uri(&encoded).is_ok());
        let parsed = crate::pairing::Invitation::parse(&encoded).unwrap();
        let paired = Connection::redeem_pairing(parsed.clone()).await.unwrap();
        assert_eq!(paired.token, owner.token);
        let (client, url) = paired.client().unwrap();
        assert!(client
            .get(url.join("/v1/health").unwrap())
            .bearer_auth(&paired.token)
            .send()
            .await
            .unwrap()
            .status()
            .is_success());
        assert!(Connection::redeem_pairing(parsed).await.is_err());
        let untrusted = Connection {
            certificate: String::new(),
            ..owner.clone()
        };
        assert!(untrusted.pairing_code().await.is_err());
        let other =
            tv_manager_server::host_library(folder.path().into(), "127.0.0.1".parse().unwrap())
                .await
                .unwrap();
        let wrong = Connection {
            certificate: base64::engine::general_purpose::STANDARD.encode(&other.certificate),
            ..owner.clone()
        };
        assert!(wrong.pairing_code().await.is_err());
        drop(host);
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(paired.pairing_code().await.is_err());
    }
}
