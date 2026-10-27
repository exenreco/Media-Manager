use crate::{backend::Connection, Form};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

const VERSION: u32 = 1;
const MAX_SIZE: usize = 2 * 1024 * 1024;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    version: u32,
    pub form: Form,
    pub connection: Connection,
    pub left_width: f64,
    pub right_width: f64,
    pub had_preview: bool,
    #[serde(default)]
    pub credentials_omitted: bool,
}

impl Session {
    pub fn new(
        form: Form,
        connection: Connection,
        left_width: f64,
        right_width: f64,
        had_preview: bool,
    ) -> Self {
        Self {
            version: VERSION,
            form,
            connection,
            left_width,
            right_width,
            had_preview,
            credentials_omitted: false,
        }
    }

    pub fn has_work(&self) -> bool {
        self.form != Form::default()
            || self.connection != Connection::default()
            || self.left_width != 340.0
            || self.right_width != 300.0
    }
}

#[cfg(any(target_os = "android", target_os = "ios"))]
static MOBILE_STATE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn initialize_mobile(path: PathBuf) -> Result<()> {
    anyhow::ensure!(path.is_absolute(), "Native settings directory is invalid");
    let _ = MOBILE_STATE.set(path);
    Ok(())
}

pub(crate) fn path() -> Result<PathBuf> {
    if let Some(directory) = std::env::var_os("MEDIA_MANAGER_STATE_DIR") {
        return Ok(PathBuf::from(directory).join(filename()));
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let directory = MOBILE_STATE
        .get()
        .context("Native settings directory is not ready")?
        .clone();
    #[cfg(windows)]
    let directory = PathBuf::from(
        std::env::var_os("LOCALAPPDATA").context("Local app data directory unavailable")?,
    )
    .join("MediaManager");
    #[cfg(target_os = "macos")]
    let directory = PathBuf::from(std::env::var_os("HOME").context("Home directory unavailable")?)
        .join("Library/Application Support/MediaManager");
    #[cfg(not(any(windows, target_os = "macos", target_os = "android", target_os = "ios")))]
    let directory = if let Some(state) = std::env::var_os("XDG_STATE_HOME") {
        PathBuf::from(state).join("MediaManager")
    } else {
        PathBuf::from(std::env::var_os("HOME").context("App data directory unavailable")?)
            .join(".local/state/MediaManager")
    };
    Ok(directory.join(filename()))
}

fn filename() -> &'static str {
    if cfg!(windows) {
        "workspace.bin"
    } else {
        "workspace.json"
    }
}

pub fn load() -> Result<Option<Session>> {
    read(&path()?)
}
pub fn save(session: &Session) -> Result<()> {
    write(&path()?, session)
}
pub fn clear() -> Result<()> {
    match fs::remove_file(path()?) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn read(path: &Path) -> Result<Option<Session>> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    if metadata.len() > (MAX_SIZE + 65536) as u64 {
        bail!("Saved workspace is too large");
    }
    let bytes = unprotect(&fs::read(path)?)?;
    let mut session: Session =
        serde_json::from_slice(&bytes).context("Saved workspace is invalid")?;
    if session.version != VERSION {
        bail!("Saved workspace has an unsupported version");
    }
    session.left_width = session.left_width.clamp(280.0, 520.0);
    session.right_width = session.right_width.clamp(260.0, 480.0);
    Ok(session.has_work().then_some(session))
}

fn write(path: &Path, session: &Session) -> Result<()> {
    #[cfg(not(windows))]
    let session = {
        let mut copy = session.clone();
        copy.credentials_omitted = !copy.form.key.is_empty()
            || !copy.form.options.tmdb_key.is_empty()
            || !copy.connection.token.is_empty();
        copy.form.key.clear();
        copy.form.options.tmdb_key.clear();
        copy.connection.token.clear();
        copy
    };
    let bytes = serde_json::to_vec(&session)?;
    if bytes.len() > MAX_SIZE {
        bail!("Workspace settings exceed the 2 MB limit");
    }
    let protected = protect(&bytes)?;
    let parent = path.parent().context("Invalid settings directory")?;
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(&protected)?;
    temporary.as_file().sync_all()?;
    // Atomic replacement prevents a partial save from corrupting the prior session.
    temporary.persist(path).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(windows)]
fn crypt(bytes: &[u8], encrypt: bool) -> Result<Vec<u8>> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        },
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len().try_into()?,
        pbData: bytes.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let success = unsafe {
        if encrypt {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    };
    if success == 0 {
        return Err(std::io::Error::last_os_error())
            .context("Windows could not protect or read the saved workspace");
    }
    if output.pbData.is_null() {
        bail!("Windows returned an empty settings buffer");
    }
    let result =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        LocalFree(output.pbData.cast());
    }
    Ok(result)
}

#[cfg(windows)]
fn protect(bytes: &[u8]) -> Result<Vec<u8>> {
    crypt(bytes, true)
}
#[cfg(windows)]
fn unprotect(bytes: &[u8]) -> Result<Vec<u8>> {
    crypt(bytes, false)
}
#[cfg(not(windows))]
fn protect(bytes: &[u8]) -> Result<Vec<u8>> {
    Ok(bytes.to_vec())
}
#[cfg(not(windows))]
fn unprotect(bytes: &[u8]) -> Result<Vec<u8>> {
    Ok(bytes.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example() -> Session {
        Session::new(
            Form {
                name: "Example".into(),
                season: "2".into(),
                download_posters: true,
                key: "private-api-key".into(),
                ..Default::default()
            },
            Connection {
                root: "library".into(),
                token: "private-server-token".into(),
                ..Default::default()
            },
            410.0,
            320.0,
            true,
        )
    }

    #[test]
    fn round_trip_restores_settings_and_panel_sizes_without_plaintext_credentials() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(filename());
        let original = example();
        write(&path, &original).unwrap();
        let restored = read(&path).unwrap().unwrap();
        assert_eq!(restored.form.name, "Example");
        assert!(restored.form.download_posters);
        assert_eq!(restored.left_width, 410.0);
        assert!(restored.had_preview);
        #[cfg(windows)]
        assert!(restored == original);
        #[cfg(not(windows))]
        {
            assert!(restored.form.key.is_empty());
            assert!(restored.connection.token.is_empty());
            assert!(restored.credentials_omitted);
        }
        let disk = fs::read(path).unwrap();
        assert!(!disk
            .windows(b"private-api-key".len())
            .any(|w| w == b"private-api-key"));
        assert!(!disk
            .windows(b"private-server-token".len())
            .any(|w| w == b"private-server-token"));
    }

    #[test]
    fn fresh_install_and_default_workspace_do_not_offer_resume() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(filename());
        assert!(read(&path).unwrap().is_none());
        let defaults = Session::new(Form::default(), Connection::default(), 340.0, 300.0, false);
        assert!(!defaults.has_work());
        write(&path, &defaults).unwrap();
        assert!(read(&path).unwrap().is_none());
    }

    #[test]
    fn oversized_save_preserves_the_previous_workspace() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(filename());
        let mut original = example();
        write(&path, &original).unwrap();
        original.form.titles = "x".repeat(MAX_SIZE + 1);
        assert!(write(&path, &original).is_err());
        assert_eq!(read(&path).unwrap().unwrap().form.name, "Example");
    }

    #[test]
    fn corrupt_settings_are_reported_and_preserved() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(filename());
        fs::write(&path, b"damaged settings").unwrap();
        assert!(read(&path).is_err());
        assert_eq!(fs::read(path).unwrap(), b"damaged settings");
    }
}
