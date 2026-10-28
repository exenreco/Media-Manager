//! Bounded preview conversion. Original library files are never modified.
use anyhow::{bail, Context, Result};
use std::{path::Path, sync::Arc, time::Duration};
use tokio::process::Command;

#[derive(Clone)]
pub struct Preview {
    pub directory: Arc<tempfile::TempDir>,
    pub path: std::path::PathBuf,
}

pub async fn convert(path: &Path) -> Result<Preview> {
    static JOB: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);
    let _slot = JOB
        .try_acquire()
        .context("Another compatibility preview is being prepared. Try again shortly.")?;
    let directory = Arc::new(tempfile::tempdir()?);
    let output = directory.path().join("preview.mp4");
    let mut command =
        Command::new(std::env::var_os("MEDIA_MANAGER_FFMPEG").unwrap_or_else(|| "ffmpeg".into()));
    command.kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    command
        .args([
            "-nostdin",
            "-hide_banner",
            "-loglevel",
            "error",
            "-protocol_whitelist",
            "file,pipe",
            "-i",
        ])
        .arg(path)
        .args([
            "-t",
            "30",
            "-map",
            "0:v:0",
            "-map",
            "0:a:0?",
            "-sn",
            "-dn",
            "-vf",
            "scale=960:540:force_original_aspect_ratio=decrease:force_divisible_by=2",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-crf",
            "25",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-ac",
            "2",
            "-b:a",
            "128k",
            "-movflags",
            "+faststart",
            "-threads",
            "2",
            "-fs",
            "67108864",
            "-y",
        ])
        .arg(&output)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let mut child = command.spawn().context("Compatibility preview requires FFmpeg on this computer. Install FFmpeg or set MEDIA_MANAGER_FFMPEG to its executable.")?;
    let status = match tokio::time::timeout(Duration::from_secs(60), child.wait()).await {
        Ok(status) => status?,
        Err(_) => {
            let _ = child.kill().await;
            bail!("Compatibility preview timed out. The original file was not changed.")
        }
    };
    if !status.success()
        || tokio::fs::metadata(&output)
            .await
            .map(|m| m.len())
            .unwrap_or(0)
            == 0
    {
        bail!("Could not decode this file. It may be damaged, encrypted, or use a codec unavailable in this FFmpeg build.");
    }
    Ok(Preview {
        directory,
        path: output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn converts_non_browser_codec_and_preserves_original() {
        if std::process::Command::new("ffmpeg")
            .arg("-version")
            .output()
            .is_err()
        {
            return;
        }
        let folder = tempfile::tempdir().unwrap();
        let input = folder.path().join("source.avi");
        let result = Command::new("ffmpeg")
            .args([
                "-nostdin",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=96x64:rate=5",
                "-t",
                "1",
                "-c:v",
                "mpeg4",
                "-y",
            ])
            .arg(&input)
            .output()
            .await
            .unwrap();
        assert!(result.status.success());
        let before = std::fs::read(&input).unwrap();
        let preview = convert(&input).await.unwrap();
        assert_eq!(std::fs::read(&input).unwrap(), before);
        let bytes = std::fs::read(&preview.path).unwrap();
        assert!(bytes.windows(4).any(|s| s == b"ftyp"));
        let path = preview.path.clone();
        drop(preview);
        assert!(!path.exists());
    }
}
