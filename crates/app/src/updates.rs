//! Signed GitHub release discovery and installer handoff. Never replaces running binaries.
use anyhow::{ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use dioxus::prelude::*;
use ring::{digest, signature};
use serde::{Deserialize, Serialize};
use std::{io::Write, path::PathBuf, time::Duration};

const REPO: &str = "https://api.github.com/repos/exenreco/Media-Manager/releases";
const DOWNLOAD_ROOT: &str = "https://github.com/exenreco/Media-Manager/releases/download/";
const CURRENT: &str = env!("CARGO_PKG_VERSION");
const MAX_METADATA: u64 = 256 * 1024;
const MAX_PACKAGE: u64 = 1024 * 1024 * 1024;
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    beta: bool,
    automatic: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            beta: semver::Version::parse(CURRENT).is_ok_and(|v| !v.pre.is_empty()),
            automatic: true,
        }
    }
}
#[derive(Clone, PartialEq, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
}
#[derive(Clone, PartialEq, Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}
#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
struct Package {
    platform: String,
    arch: String,
    kind: String,
    name: String,
    url: String,
    sha256: String,
    size: u64,
}
#[derive(Serialize, Deserialize)]
struct Manifest {
    schema: u32,
    version: String,
    packages: Vec<Package>,
}
#[derive(Clone, PartialEq)]
pub struct Available {
    version: String,
    package: Package,
}
#[derive(Clone, Copy, PartialEq)]
pub struct Updater {
    preferences: Signal<Preferences>,
    status: Signal<String>,
    available: Signal<Option<Available>>,
    downloaded: Signal<Option<PathBuf>>,
    working: Signal<bool>,
    notice: Signal<bool>,
    progress: Signal<u64>,
}
fn directory() -> Result<PathBuf> {
    Ok(crate::session::path()?
        .parent()
        .context("Settings directory unavailable")?
        .join("updates"))
}
fn load_preferences() -> Result<Preferences> {
    let p = directory()?.join("preferences.json");
    match std::fs::read(p) {
        Ok(data) => Ok(serde_json::from_slice(&data)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Preferences::default()),
        Err(e) => Err(e.into()),
    }
}
fn save_preferences(p: &Preferences) -> Result<()> {
    let dir = directory()?;
    std::fs::create_dir_all(&dir)?;
    let mut file = tempfile::NamedTempFile::new_in(&dir)?;
    file.write_all(&serde_json::to_vec(p)?)?;
    file.as_file().sync_all()?;
    file.persist(dir.join("preferences.json"))?;
    Ok(())
}
pub fn use_updater() -> Updater {
    let initial = use_hook(|| load_preferences().map_err(|e| format!("{e:#}")));
    let initial_preferences = initial.as_ref().cloned().unwrap_or_default();
    let mut updater = Updater {
        preferences: use_signal(|| initial_preferences),
        status: use_signal(|| {
            initial
                .err()
                .map(|e| format!("Could not load update preferences: {e}"))
                .unwrap_or_default()
        }),
        available: use_signal(|| None),
        downloaded: use_signal(|| None),
        working: use_signal(|| false),
        notice: use_signal(|| false),
        progress: use_signal(|| 0),
    };
    use_future(move || async move {
        tokio::time::sleep(Duration::from_secs(20)).await;
        loop {
            if updater.preferences.peek().automatic && !*updater.working.peek() {
                updater.check().await;
            }
            tokio::time::sleep(Duration::from_secs(12 * 60 * 60)).await;
        }
    });
    updater
}
impl Updater {
    async fn check(&mut self) {
        if *self.working.peek() {
            return;
        }
        self.working.set(true);
        self.status.set("Checking for updates…".into());
        self.notice.set(false);
        self.available.set(None);
        self.downloaded.set(None);
        let beta = self.preferences.peek().beta;
        match discover(beta).await {
            Ok(Some(update)) => {
                self.status.set(format!(
                    "Version {} is available. Download size: {} MB.",
                    update.version,
                    update.package.size.div_ceil(1024 * 1024)
                ));
                self.available.set(Some(update));
                self.notice.set(true);
            }
            Ok(None) => self
                .status
                .set("You are up to date for this channel.".into()),
            Err(e) => self.status.set(format!(
                "Could not check for updates: {e:#}. Your installed app is unchanged."
            )),
        }
        self.working.set(false);
    }
    fn preferences_changed(&mut self, preferences: Preferences) {
        match save_preferences(&preferences) {
            Ok(()) => {
                self.preferences.set(preferences);
                self.available.set(None);
                self.downloaded.set(None);
                self.notice.set(false);
                self.status.set(
                    "Update preferences saved. Check for updates to refresh this channel.".into(),
                );
            }
            Err(e) => self
                .status
                .set(format!("Could not save preferences: {e:#}")),
        }
    }
    async fn download(&mut self) {
        let Some(update) = self.available.peek().clone() else {
            return;
        };
        if *self.working.peek() {
            return;
        }
        self.working.set(true);
        self.progress.set(0);
        self.status.set("Downloading verified update…".into());
        match download(&update.package, self.progress).await {
            Ok(path) => {
                self.downloaded.set(Some(path));
                self.status.set("Download verified. Save your work, then open the installer. Follow its prompts and restart Media Manager after installation.".into());
            }
            Err(e) => self.status.set(format!(
                "Update download failed: {e:#}. Nothing was installed."
            )),
        }
        self.working.set(false);
    }
}
fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(600))
        .user_agent(concat!("Media-Manager/", env!("CARGO_PKG_VERSION")))
        .build()?)
}
async fn bytes(client: &reqwest::Client, url: &str, limit: u64) -> Result<Vec<u8>> {
    let mut response = client
        .get(url)
        .timeout(Duration::from_secs(30))
        .send()
        .await?
        .error_for_status()?;
    ensure!(
        response.content_length().is_none_or(|n| n <= limit),
        "Update metadata is too large"
    );
    let mut data = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(
            data.len() as u64 + chunk.len() as u64 <= limit,
            "Update metadata is too large"
        );
        data.extend_from_slice(&chunk);
    }
    Ok(data)
}
fn release_version(release: &Release, beta: bool) -> Option<semver::Version> {
    if release.draft || (!beta && release.prerelease) {
        return None;
    }
    let version = semver::Version::parse(release.tag_name.strip_prefix('v')?).ok()?;
    if !beta && !version.pre.is_empty() {
        return None;
    }
    Some(version)
}
fn platform() -> (&'static str, &'static str, &'static str) {
    let platform = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };
    let arch = std::env::consts::ARCH;
    let kind = if cfg!(windows) {
        "exe"
    } else if cfg!(target_os = "macos") {
        "pkg"
    } else if std::env::var_os("FLATPAK_ID").is_some()
        || !std::path::Path::new("/etc/debian_version").exists()
    {
        "flatpak"
    } else {
        "deb"
    };
    (platform, arch, kind)
}
fn verify_manifest(
    payload: &[u8],
    encoded: &[u8],
    public: &[u8],
    version: &str,
    target: (&str, &str, &str),
) -> Result<Available> {
    let sig = STANDARD.decode(std::str::from_utf8(encoded)?.trim())?;
    signature::UnparsedPublicKey::new(&signature::ED25519, public)
        .verify(payload, &sig)
        .map_err(|_| anyhow::anyhow!("Update signature is invalid"))?;
    let manifest: Manifest = serde_json::from_slice(payload)?;
    ensure!(
        manifest.schema == 1 && manifest.version == version,
        "Update metadata version mismatch"
    );
    let matches: Vec<_> = manifest
        .packages
        .into_iter()
        .filter(|p| (p.platform.as_str(), p.arch.as_str(), p.kind.as_str()) == target)
        .collect();
    ensure!(
        matches.len() == 1,
        "No unique compatible installer is available for this system"
    );
    let package = matches.into_iter().next().unwrap();
    ensure!(
        package.size > 0
            && package.size <= MAX_PACKAGE
            && package.sha256.len() == 64
            && package.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
        "Invalid installer metadata"
    );
    let suffix = format!("v{}/{}", manifest.version, package.name);
    ensure!(
        package.name.starts_with("Media-Manager-")
            && !package.name.contains(['/', '\\'])
            && !package.name.contains("..")
            && package.name.ends_with(&format!(".{}", package.kind)),
        "Invalid installer filename"
    );
    ensure!(
        package.url == format!("{DOWNLOAD_ROOT}{suffix}"),
        "Installer URL is outside the trusted release"
    );
    Ok(Available {
        version: manifest.version,
        package,
    })
}
async fn discover(beta: bool) -> Result<Option<Available>> {
    let client = client()?;
    let releases: Vec<Release> = serde_json::from_slice(
        &bytes(&client, &format!("{REPO}?per_page=100"), 2 * 1024 * 1024).await?,
    )?;
    let current = semver::Version::parse(CURRENT)?;
    let newest = releases
        .iter()
        .filter_map(|r| {
            release_version(r, beta)
                .filter(|v| v > &current)
                .map(|v| (v, r))
        })
        .max_by(|a, b| a.0.cmp(&b.0));
    let Some((version, release)) = newest else {
        return Ok(None);
    };
    let version = version.to_string();
    let get_asset = |name: &str| -> Result<&Asset> {
        release.assets.iter().find(|a| a.name == name).context(
            "This release has no signed updater metadata yet. Try again after its builds finish.",
        )
    };
    let manifest = get_asset("update-manifest.json")?;
    let signature = get_asset("update-manifest.sig")?;
    for asset in [manifest, signature] {
        ensure!(
            asset.browser_download_url
                == format!("{DOWNLOAD_ROOT}{}/{}", release.tag_name, asset.name)
                && asset.size <= MAX_METADATA,
            "Invalid metadata download location"
        );
    }
    let payload = bytes(&client, &manifest.browser_download_url, MAX_METADATA).await?;
    let signature = bytes(&client, &signature.browser_download_url, 1024).await?;
    let public = STANDARD.decode(include_str!("update-public-key.txt").trim())?;
    verify_manifest(&payload, &signature, &public, &version, platform()).map(Some)
}
async fn download(package: &Package, mut progress: Signal<u64>) -> Result<PathBuf> {
    let dir = directory()?;
    std::fs::create_dir_all(&dir)?;
    let mut file = tempfile::NamedTempFile::new_in(&dir)?;
    let mut response = client()?
        .get(&package.url)
        .send()
        .await?
        .error_for_status()?;
    ensure!(
        response.content_length().is_none_or(|n| n == package.size),
        "Installer size differs from signed metadata"
    );
    let mut digest = digest::Context::new(&digest::SHA256);
    let mut size = 0u64;
    let mut percent = 0;
    while let Some(chunk) = response.chunk().await? {
        size += chunk.len() as u64;
        ensure!(size <= package.size, "Installer exceeds signed size");
        file.write_all(&chunk)?;
        digest.update(&chunk);
        let next = size * 100 / package.size;
        if next != percent {
            percent = next;
            progress.set(next);
        }
    }
    ensure!(size == package.size, "Installer download was incomplete");
    let actual = digest
        .finish()
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    ensure!(
        actual.eq_ignore_ascii_case(&package.sha256),
        "Installer checksum mismatch"
    );
    file.as_file().sync_all()?;
    let path = dir.join(&package.name);
    file.persist(&path)?;
    Ok(path)
}
fn verify_download(path: &std::path::Path, package: &Package) -> Result<()> {
    let expected = directory()?.join(&package.name);
    ensure!(
        path == expected && path.is_file(),
        "Verified installer is missing. Download it again."
    );
    verify_contents(path, package)
}
fn verify_contents(path: &std::path::Path, package: &Package) -> Result<()> {
    // Recheck immediately before invoking another process; do not trust a stale verified flag.
    let mut file = std::fs::File::open(path)?;
    let mut context = digest::Context::new(&digest::SHA256);
    let mut buffer = [0u8; 65536];
    let mut size = 0;
    loop {
        let n = std::io::Read::read(&mut file, &mut buffer)?;
        if n == 0 {
            break;
        }
        size += n as u64;
        context.update(&buffer[..n]);
    }
    let actual = context
        .finish()
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    ensure!(
        size == package.size && actual.eq_ignore_ascii_case(&package.sha256),
        "Installer changed since verification. Download it again."
    );
    Ok(())
}
async fn open_installer(path: PathBuf, package: Package) -> Result<()> {
    let checked = path.clone();
    tokio::task::spawn_blocking(move || verify_download(&checked, &package)).await??;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new(&path)
            .arg("/RESTARTAPPLICATIONS")
            .creation_flags(0x08000000)
            .spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("/usr/bin/open")
            .arg(&path)
            .spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        let file = std::fs::File::open(&path)?;
        ashpd::desktop::open_uri::OpenFileRequest::default()
            .ask(true)
            .send_file(&file)
            .await
            .context("Open the downloaded package with your desktop software manager")?
            .response()?;
    }
    Ok(())
}
#[component]
pub fn UpdatePanel(mut updater: Updater, blocked: bool) -> Element {
    let preferences = updater.preferences.read().clone();
    let working = (updater.working)();
    let available = updater.available.read().clone();
    let downloaded = updater.downloaded.read().clone();
    rsx! {section {class:"settings-group update-settings",
        h2 {"App updates"} p {class:"hint","Installed version: {CURRENT}"}
        div {class:"update-preferences-row",
        label {class:"update-channel","Release channel" select {value:if preferences.beta{"beta"}else{"stable"},disabled:working,
            onchange:move |e|{let mut p=updater.preferences.peek().clone();p.beta=e.value()=="beta";updater.preferences_changed(p);},
            option {value:"stable","Stable"} option {value:"beta","Beta and stable"}}}
        label {class:"check",input {r#type:"checkbox",checked:preferences.automatic,disabled:working,onchange:move |e|{let mut p=updater.preferences.peek().clone();p.automatic=e.checked();updater.preferences_changed(p);}},"Check automatically every 12 hours"}
        }
        button {class:"secondary",disabled:working,onclick:move |_|{spawn(async move {updater.check().await;});},"Check for updates"}
        if let Some(update)=available {
            p {"Update {update.version} available"}
            if downloaded.is_none() {button {class:"primary",disabled:working,onclick:move |_|{spawn(async move {updater.download().await;});},"Download update"}}
            if let Some(path)=downloaded {button {class:"primary",disabled:working || blocked,onclick:move |_|{
                if blocked || *updater.working.peek(){return}
                let package=update.package.clone();let path=path.clone();updater.working.set(true);
                spawn(async move {let result=open_installer(path,package).await;
                    updater.status.set(match result {Ok(())=>"Installer opened. Complete its steps, then restart Media Manager.".into(),Err(e)=>format!("Could not start installer: {e:#}")});updater.working.set(false);});
            },"Open installer"}}
        }
        if working {p {class:"hint",role:"status","Working… {(updater.progress)()}%"}}
        if blocked {p {class:"hint","Finish the current media operation before opening an installer."}}
        p {class:"status",role:"status","{updater.status}"}
        p {class:"hint","Downloads are checked against signed release metadata. Your library and settings are kept by the installer."}
    }}
}
#[component]
pub fn UpdateNotice(mut updater: Updater, onopen: EventHandler) -> Element {
    if !(updater.notice)() {
        return rsx! {};
    }
    rsx! {aside {class:"update-notice",role:"status","A Media Manager update is available.",
        button {class:"secondary",onclick:move |_|{updater.notice.set(false);onopen.call(());},"View update"}
        button {class:"secondary",onclick:move |_|updater.notice.set(false),"Later"}
    }}
}
#[cfg(test)]
mod tests {
    use super::*;
    use ring::signature::KeyPair;
    fn package() -> Package {
        Package {
            platform: "windows".into(),
            arch: "x86_64".into(),
            kind: "exe".into(),
            name: "Media-Manager-2.0.0-windows-x86_64-setup.exe".into(),
            url: format!("{DOWNLOAD_ROOT}v2.0.0/Media-Manager-2.0.0-windows-x86_64-setup.exe"),
            sha256: "a".repeat(64),
            size: 123,
        }
    }
    fn signed(manifest: &Manifest) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let key = signature::Ed25519KeyPair::from_seed_unchecked(&[7; 32]).unwrap();
        let bytes = serde_json::to_vec(manifest).unwrap();
        let signature = STANDARD.encode(key.sign(&bytes).as_ref()).into_bytes();
        (bytes, signature, key.public_key().as_ref().to_vec())
    }
    fn verify(manifest: Manifest) -> Result<Available> {
        let (data, sig, key) = signed(&manifest);
        verify_manifest(&data, &sig, &key, "2.0.0", ("windows", "x86_64", "exe"))
    }
    #[test]
    fn verified_signature_is_required_and_bound_to_bytes() {
        let manifest = Manifest {
            schema: 1,
            version: "2.0.0".into(),
            packages: vec![package()],
        };
        let (mut data, sig, key) = signed(&manifest);
        assert!(verify_manifest(&data, &sig, &key, "2.0.0", ("windows", "x86_64", "exe")).is_ok());
        data[0] ^= 1;
        assert!(verify_manifest(&data, &sig, &key, "2.0.0", ("windows", "x86_64", "exe")).is_err());
        assert!(
            verify_manifest(&data, &sig, &[9; 32], "2.0.0", ("windows", "x86_64", "exe")).is_err()
        );
    }
    #[test]
    fn signed_manifest_cannot_redirect_to_another_host_or_escape_cache() {
        for url in [
            "http://github.com/exenreco/Media-Manager/releases/download/v2.0.0/test.exe",
            "https://example.com/update.exe",
        ] {
            let mut p = package();
            p.url = url.into();
            assert!(verify(Manifest {
                schema: 1,
                version: "2.0.0".into(),
                packages: vec![p]
            })
            .is_err());
        }
        for name in [
            "../update.exe",
            "Media-Manager-..-setup.exe",
            "Media-Manager-test/evil.exe",
        ] {
            let mut p = package();
            p.name = name.into();
            assert!(verify(Manifest {
                schema: 1,
                version: "2.0.0".into(),
                packages: vec![p]
            })
            .is_err());
        }
    }
    #[test]
    fn requires_one_compatible_asset_and_exact_version() {
        for packages in [vec![], vec![package(), package()]] {
            assert!(verify(Manifest {
                schema: 1,
                version: "2.0.0".into(),
                packages
            })
            .is_err());
        }
        let mut p = package();
        p.arch = "aarch64".into();
        assert!(verify(Manifest {
            schema: 1,
            version: "2.0.0".into(),
            packages: vec![p]
        })
        .is_err());
        assert!(verify(Manifest {
            schema: 1,
            version: "2.0.1".into(),
            packages: vec![package()]
        })
        .is_err());
        assert!(verify(Manifest {
            schema: 2,
            version: "2.0.0".into(),
            packages: vec![package()]
        })
        .is_err());
    }
    #[test]
    fn rejects_unbounded_or_invalid_package_metadata() {
        let mut p = package();
        p.size = MAX_PACKAGE + 1;
        assert!(verify(Manifest {
            schema: 1,
            version: "2.0.0".into(),
            packages: vec![p]
        })
        .is_err());
        let mut p = package();
        p.sha256 = "z".repeat(64);
        assert!(verify(Manifest {
            schema: 1,
            version: "2.0.0".into(),
            packages: vec![p]
        })
        .is_err());
    }
    #[test]
    fn stable_channel_never_selects_a_beta_or_draft() {
        let mut r = Release {
            tag_name: "v1.0.0-beta.10".into(),
            draft: false,
            prerelease: true,
            assets: vec![],
        };
        assert!(release_version(&r, false).is_none());
        assert!(release_version(&r, true).is_some());
        r.prerelease = false;
        assert!(release_version(&r, false).is_none());
        r.tag_name = "v1.0.0".into();
        assert!(release_version(&r, false).is_some());
        r.draft = true;
        assert!(release_version(&r, true).is_none());
        assert!(
            semver::Version::parse("1.0.0-beta.10").unwrap()
                > semver::Version::parse("1.0.0-beta.2").unwrap()
        );
    }
    #[test]
    fn modified_downloads_are_rejected_before_installer_handoff() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(b"verified installer").unwrap();
        let mut p = package();
        p.size = 18;
        p.sha256 = digest::digest(&digest::SHA256, b"verified installer")
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert!(verify_contents(file.path(), &p).is_ok());
        std::fs::write(file.path(), b"tampered installer").unwrap();
        assert!(verify_contents(file.path(), &p).is_err());
        p.size = 999;
        assert!(verify_contents(file.path(), &p).is_err());
    }
}
