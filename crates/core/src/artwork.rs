//! Poster inventory and explicit saves, independent of rename previews.
use crate::{
    rename::{self, RenamePlan},
    PlanRequest,
};
use anyhow::{bail, Context, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
};
const LIMIT: usize = 10 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Poster {
    pub id: String,
    pub title: String,
    pub provider: Option<String>,
    pub local_path: Option<String>,
    pub local: Option<String>,
    pub fingerprint: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Save {
    pub slot: String,
    pub url: String,
    pub replace: bool,
    pub expected: Option<String>,
}
fn relative(root: &Path, path: &Path) -> Result<String> {
    Ok(path
        .strip_prefix(root)?
        .to_string_lossy()
        .replace('\\', "/"))
}
pub fn slots(plan: &RenamePlan) -> Result<Vec<Poster>> {
    let mut show = plan.show.clone();
    show.artwork.poster.get_or_insert(String::new());
    for episode in &show.episodes {
        show.artwork.seasons.entry(episode.season).or_default();
    }
    let mut slots = Vec::new();
    for task in crate::posters::plan(&plan.root, &PlanRequest::default(), &show)? {
        let parent = task.target.parent().context("Invalid poster folder")?;
        let stem = task
            .target
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy();
        let mut paths = vec![task.target.clone()];
        for ext in ["jpg", "jpeg", "png", "webp"] {
            paths.push(parent.join(format!("{stem}.{ext}")));
            if stem == "poster" {
                paths.push(parent.join(format!("folder.{ext}")));
            }
        }
        let local = paths.into_iter().find(|path| {
            fs::symlink_metadata(path).is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
        });
        slots.push(Poster {
            id: relative(&plan.root, &task.target)?,
            title: task.label,
            provider: (!task.url.is_empty()).then_some(task.url),
            local_path: local.map(|p| relative(&plan.root, &p)).transpose()?,
            local: None,
            fingerprint: None,
        });
    }
    // Include local posters even if the provider has no metadata for that season.
    for entry in walkdir::WalkDir::new(&plan.root)
        .follow_links(false)
        .max_depth(20)
    {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let ext = entry
            .path()
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !["jpg", "jpeg", "png", "webp"].contains(&ext.as_str()) {
            continue;
        }
        let stem = entry
            .path()
            .file_stem()
            .and_then(|v| v.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if stem != "poster"
            && stem != "folder"
            && stem != "show-poster"
            && !(stem.starts_with("season") && stem.ends_with("-poster"))
        {
            continue;
        }
        let path = relative(&plan.root, entry.path())?;
        if slots.iter().any(|s| s.local_path.as_ref() == Some(&path)) {
            continue;
        }
        let season = rename::season_in_name(&stem).or_else(|| {
            entry
                .path()
                .ancestors()
                .skip(1)
                .take_while(|p| p.starts_with(&plan.root))
                .find_map(|p| {
                    p.file_name()
                        .and_then(|v| v.to_str())
                        .and_then(rename::season_in_name)
                })
        });
        let title = season.map_or_else(
            || "Show poster".to_string(),
            |n| format!("Season {n:02} poster"),
        );
        let provider = season
            .and_then(|n| plan.show.artwork.seasons.get(&n).cloned())
            .or_else(|| {
                season
                    .is_none()
                    .then(|| plan.show.artwork.poster.clone())
                    .flatten()
            });
        if let Some(slot) = slots.iter_mut().find(|s| {
            s.local_path.is_none()
                && s.title == title
                && Path::new(&s.id).parent() == Path::new(&path).parent()
        }) {
            slot.local_path = Some(path);
            continue;
        }
        slots.push(Poster {
            id: path.clone(),
            title,
            provider,
            local_path: Some(path),
            local: None,
            fingerprint: None,
        });
        anyhow::ensure!(slots.len() <= 1000, "Too many poster files");
    }
    Ok(slots)
}
fn scoped(root: &Path, path: &str) -> Result<PathBuf> {
    let target = root.join(path);
    let parent = rename::scoped_folder(
        root,
        &target
            .parent()
            .context("Invalid poster path")?
            .strip_prefix(root)?
            .to_string_lossy(),
    )?;
    let target = parent.join(target.file_name().context("Invalid poster filename")?);
    if fs::symlink_metadata(&target).is_ok_and(|m| m.file_type().is_symlink()) {
        bail!("Poster cannot be a symbolic link");
    }
    Ok(target)
}
pub fn fingerprint(bytes: &[u8]) -> Result<String> {
    anyhow::ensure!(bytes.len() <= LIMIT, "Poster exceeds 10 MB");
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
pub fn image_data(bytes: &[u8]) -> Result<(String, String)> {
    anyhow::ensure!(bytes.len() <= LIMIT, "Poster exceeds 10 MB");
    let format = image::guess_format(bytes)?;
    let mime = match format {
        image::ImageFormat::Jpeg => "image/jpeg",
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::WebP => "image/webp",
        _ => bail!("Unsupported poster image"),
    };
    Ok((
        format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ),
        fingerprint(bytes)?,
    ))
}
pub fn inventory(plan: &RenamePlan) -> Result<Vec<Poster>> {
    let mut slots = slots(plan)?;
    let mut total = 0;
    for slot in &mut slots {
        if let Some(path) = &slot.local_path {
            let file = fs::File::open(scoped(&plan.root, path)?)?;
            let mut bytes = Vec::new();
            file.take((LIMIT + 1) as u64).read_to_end(&mut bytes)?;
            total += bytes.len();
            anyhow::ensure!(total <= 64 * 1024 * 1024, "Poster gallery exceeds 64 MB");
            slot.fingerprint = Some(fingerprint(&bytes)?);
            if let Ok((uri, hash)) = image_data(&bytes) {
                slot.local = Some(uri);
                slot.fingerprint = Some(hash);
            }
        }
    }
    Ok(slots)
}
pub fn validate_save(plan: &RenamePlan, save: &Save) -> Result<Poster> {
    let slot = slots(plan)?
        .into_iter()
        .find(|s| s.id == save.slot)
        .context("Poster destination no longer available")?;
    let art = &plan.show.artwork;
    anyhow::ensure!(
        art.poster.as_ref() == Some(&save.url)
            || art.seasons.values().any(|u| u == &save.url)
            || art.alternatives.iter().any(|a| a.url == save.url),
        "Choose a poster from this preview's provider results"
    );
    anyhow::ensure!(
        slot.local_path.is_none() || save.replace,
        "Confirm Replace existing poster first"
    );
    Ok(slot)
}
pub async fn fetch(url: &str) -> Result<Vec<u8>> {
    let url = reqwest::Url::parse(url)?;
    anyhow::ensure!(
        url.scheme() == "https" && url.username().is_empty() && url.password().is_none(),
        "Poster URL must use HTTPS without credentials"
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(25))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let mut response = client
        .get(url)
        .send()
        .await
        .context("Could not fetch poster")?
        .error_for_status()?;
    anyhow::ensure!(
        !response.content_length().is_some_and(|n| n > LIMIT as u64),
        "Poster exceeds 10 MB"
    );
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        anyhow::ensure!(bytes.len() + chunk.len() <= LIMIT, "Poster exceeds 10 MB");
        bytes.extend_from_slice(&chunk);
    }
    image_data(&bytes)?;
    Ok(bytes)
}
pub fn encode_for_path(bytes: &[u8], path: &str) -> Result<Vec<u8>> {
    let format = image::ImageFormat::from_path(path)?;
    if image::guess_format(bytes)? == format {
        return Ok(bytes.to_vec());
    }
    let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode()?;
    let mut output = Cursor::new(Vec::new());
    image.write_to(&mut output, format)?;
    anyhow::ensure!(
        output.get_ref().len() <= LIMIT,
        "Converted poster exceeds 10 MB"
    );
    Ok(output.into_inner())
}
pub fn write(plan: &RenamePlan, save: &Save, bytes: &[u8]) -> Result<()> {
    let slot = validate_save(plan, save)?;
    let path = slot.local_path.as_ref().unwrap_or(&slot.id);
    let target = scoped(&plan.root, path)?;
    let bytes = encode_for_path(bytes, path)?;
    let mut temp = tempfile::NamedTempFile::new_in(target.parent().unwrap())?;
    temp.write_all(&bytes)?;
    temp.as_file().sync_all()?;
    if slot.local_path.is_some() {
        let mut existing = Vec::new();
        fs::File::open(&target)?
            .take((LIMIT + 1) as u64)
            .read_to_end(&mut existing)?;
        let hash = fingerprint(&existing)?;
        anyhow::ensure!(
            save.expected.as_ref() == Some(&hash),
            "Existing poster changed; select it again before replacing"
        );
        temp.persist(&target).map_err(|e| e.error)?;
    } else {
        temp.persist_noclobber(&target).map_err(|e| e.error)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn png(color: u8) -> Vec<u8> {
        let image = image::RgbImage::from_pixel(2, 3, image::Rgb([color, 0, 0]));
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    }
    fn fixture() -> (tempfile::TempDir, RenamePlan) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        fs::create_dir(root.join("Season 01")).unwrap();
        fs::write(root.join("Season 01/01.mkv"), b"video").unwrap();
        let show = crate::Show {
            name: "Demo".into(),
            year: None,
            episodes: vec![crate::Episode {
                season: 1,
                number: 1,
                name: "Pilot".into(),
            }],
            artwork: crate::Artwork {
                poster: Some("https://example.test/main.png".into()),
                seasons: std::collections::BTreeMap::from([(
                    1,
                    "https://example.test/season.png".into(),
                )]),
                ..Default::default()
            },
        };
        let plan = rename::build_plan(
            &root,
            &PlanRequest::default(),
            show,
            &[root.join("Season 01/01.mkv")],
        )
        .unwrap();
        (dir, plan)
    }
    #[test]
    fn inventory_prefers_local_artwork_and_keeps_provider_fallback_for_missing_posters() {
        let (_dir, plan) = fixture();
        let bytes = png(25);
        fs::write(plan.root.join("Season 01/poster.jpg"), &bytes).unwrap();
        let items = inventory(&plan).unwrap();
        let season = items
            .iter()
            .find(|p| p.title == "Season 01 poster")
            .unwrap();
        assert!(season
            .local
            .as_ref()
            .unwrap()
            .starts_with("data:image/png;base64,"));
        assert!(season.fingerprint.is_some());
        assert!(season.provider.is_some());
        let main = items.iter().find(|p| p.title == "Show poster").unwrap();
        assert!(main.local.is_none());
        assert!(main.provider.is_some());
    }
    #[test]
    fn explicit_save_needs_replace_consent_and_rejects_changed_existing_images() {
        let (_dir, plan) = fixture();
        let original = png(25);
        fs::write(plan.root.join("poster.png"), &original).unwrap();
        let items = inventory(&plan).unwrap();
        let main = items.iter().find(|p| p.title == "Show poster").unwrap();
        let mut save = Save {
            slot: main.id.clone(),
            url: main.provider.clone().unwrap(),
            replace: false,
            expected: main.fingerprint.clone(),
        };
        let replacement = png(100);
        assert!(write(&plan, &save, &replacement).is_err());
        assert_eq!(fs::read(plan.root.join("poster.png")).unwrap(), original);
        save.replace = true;
        fs::write(plan.root.join("poster.png"), png(50)).unwrap();
        assert!(write(&plan, &save, &replacement).is_err());
        fs::write(plan.root.join("poster.png"), &original).unwrap();
        write(&plan, &save, &replacement).unwrap();
        assert_eq!(fs::read(plan.root.join("poster.png")).unwrap(), replacement);
        save.slot = "../outside.png".into();
        assert!(write(&plan, &save, &original).is_err());
    }
    #[test]
    fn missing_poster_save_is_noclobber_and_does_not_touch_video_files() {
        let (_dir, plan) = fixture();
        let main = inventory(&plan)
            .unwrap()
            .into_iter()
            .find(|p| p.title == "Show poster")
            .unwrap();
        let save = Save {
            slot: main.id.clone(),
            url: main.provider.unwrap(),
            replace: false,
            expected: None,
        };
        write(&plan, &save, &png(5)).unwrap();
        assert!(write(&plan, &save, &png(7)).is_err());
        assert_eq!(
            fs::read(plan.root.join("Season 01/01.mkv")).unwrap(),
            b"video"
        );
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Batch {
    pub posters: Vec<Poster>,
    pub downloaded: usize,
    pub kept: usize,
    pub failures: Vec<String>,
}
/// One main/season poster per missing destination; existing files are never replaced.
pub async fn download_missing(plan: &RenamePlan) -> Result<Batch> {
    let mut result = Batch {
        posters: Vec::new(),
        downloaded: 0,
        kept: 0,
        failures: Vec::new(),
    };
    for slot in slots(plan)? {
        if slot.local_path.is_some() {
            result.kept += 1;
            continue;
        }
        let Some(url) = slot.provider else {
            continue;
        };
        let save = Save {
            slot: slot.id.clone(),
            url,
            replace: false,
            expected: None,
        };
        let outcome = async {
            let bytes = fetch(&save.url).await?;
            write(plan, &save, &bytes)
        }
        .await;
        match outcome {
            Ok(()) => result.downloaded += 1,
            Err(error) => {
                if slots(plan)?
                    .iter()
                    .any(|p| p.id == slot.id && p.local_path.is_some())
                {
                    result.kept += 1;
                } else {
                    result.failures.push(format!("{}: {error:#}", slot.title));
                }
            }
        }
    }
    result.posters = inventory(plan)?;
    Ok(result)
}
