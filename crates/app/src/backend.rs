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
}

impl Default for Connection {
    fn default() -> Self {
        Self {
            remote: false,
            root: String::new(),
            url: "http://127.0.0.1:8787".into(),
            token: String::new(),
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
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(180))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok((client, url))
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
    pub async fn redeem_pairing(url: String, code: String) -> Result<Self> {
        // Reuse URL validation without ever transmitting the placeholder credential.
        let connection = Self {
            remote: true,
            url,
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

    /// None means the default desktop player was opened; Some is a remote stream.
    pub async fn play(&self, plan: RenamePlan, index: usize) -> Result<Option<String>> {
        if !self.remote {
            #[cfg(any(target_os = "android", target_os = "ios"))]
            return crate::mobile_storage::play(&plan, index).await;
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                let path = rename::playback_path(&plan, index)?;
                tokio::task::spawn_blocking(move || open_local(&path)).await??;
                return Ok(None);
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
        Ok(Some(url.join(&format!("/v1/media/{ticket}"))?.to_string()))
    }
}

#[cfg(windows)]
fn open_local(path: &std::path::Path) -> Result<()> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    let raw = path.to_string_lossy();
    let normalized = if let Some(unc) = raw.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        raw.strip_prefix(r"\\?\").unwrap_or(&raw).to_string()
    };
    let wide: Vec<u16> = normalized.encode_utf16().chain(Some(0)).collect();
    let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            wide.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };
    if result as isize <= 32 {
        bail!("Windows could not open the media file. Install or choose a default video player.");
    }
    Ok(())
}

#[cfg(all(not(windows), not(any(target_os = "android", target_os = "ios"))))]
fn open_local(path: &std::path::Path) -> Result<()> {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(program)
        .arg(path)
        .spawn()
        .context("Could not open the default media player")?;
    Ok(())
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
