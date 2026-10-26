use crate::{
    rename::{scoped_folder, season_in_name, RenamePlan, TaskStatus},
    PlanRequest, Show,
};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};
use walkdir::WalkDir;

const MAX_IMAGE: usize = 10 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PosterTask {
    pub label: String,
    pub url: String,
    pub target: PathBuf,
    pub status: TaskStatus,
    pub message: String,
}

fn extension(url: &str) -> &'static str {
    let path = reqwest::Url::parse(url)
        .map(|u| u.path().to_ascii_lowercase())
        .unwrap_or_default();
    if path.ends_with(".png") {
        "png"
    } else if path.ends_with(".webp") {
        "webp"
    } else {
        "jpg"
    }
}

pub fn plan(folder: &Path, request: &PlanRequest, show: &Show) -> Result<Vec<PosterTask>> {
    let mut tasks = vec![];
    let root_season = folder
        .file_name()
        .and_then(|s| s.to_str())
        .and_then(season_in_name);
    let mut add = |label: String, url: &str, parent: &Path, basename: &str| {
        let target = parent.join(format!("{basename}.{}", extension(url)));
        let exists = fs::symlink_metadata(&target).is_ok();
        tasks.push(PosterTask {
            label,
            url: url.into(),
            target,
            status: if exists {
                TaskStatus::Unchanged
            } else {
                TaskStatus::Ready
            },
            message: if exists {
                "Existing poster will be kept".into()
            } else {
                String::new()
            },
        });
    };
    if let Some(url) = &show.artwork.poster {
        add(
            "Show poster".into(),
            url,
            folder,
            if root_season.is_some() {
                "show-poster"
            } else {
                "poster"
            },
        );
    }
    if !show.artwork.seasons.is_empty() {
        let folders = WalkDir::new(folder)
            .min_depth(1)
            .follow_links(false)
            .into_iter()
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for (&season, url) in &show.artwork.seasons {
            if request.season.is_some_and(|s| s != season) {
                continue;
            }
            let label = format!("Season {season:02} poster");
            let mut found = false;
            if root_season == Some(season) {
                add(label.clone(), url, folder, "poster");
                found = true;
            }
            for entry in &folders {
                if entry.file_type().is_dir()
                    && entry.file_name().to_str().and_then(season_in_name) == Some(season)
                {
                    add(label.clone(), url, entry.path(), "poster");
                    found = true;
                }
            }
            if !found {
                add(label, url, folder, &format!("season{season:02}-poster"));
            }
        }
    }
    Ok(tasks)
}

fn relocated(path: &Path, moves: &[(PathBuf, PathBuf)]) -> PathBuf {
    let mut path = path.to_path_buf();
    for (source, target) in moves {
        if let Ok(suffix) = path.strip_prefix(source) {
            path = target.join(suffix);
        }
    }
    path
}

fn image_extension(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("jpg")
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

fn save(root: &Path, target: &Path, bytes: &[u8]) -> Result<()> {
    let relative = target
        .strip_prefix(root)
        .context("Poster is outside the selected folder")?;
    let parent = scoped_folder(
        root,
        &relative.parent().unwrap_or(Path::new("")).to_string_lossy(),
    )?;
    if fs::symlink_metadata(target).is_ok() {
        bail!("Destination already exists; existing poster kept");
    }
    let format =
        image_extension(bytes).context("Provider did not return a supported poster image")?;
    if target.extension().and_then(|e| e.to_str()) != Some(format) {
        bail!("Poster format differs from its URL; image was not saved");
    }
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    crate::move_file::no_replace(temporary.path(), target)
        .context("Could not save poster without replacing an existing file")?;
    Ok(())
}

/// Execute after renames so season posters follow any successful Plex folder move.
/// Image failures are reported separately and do not undo successful renames.
pub async fn download(plan: &mut RenamePlan) {
    let moves: Vec<_> = plan
        .tasks
        .iter()
        .filter(|t| t.directory && t.status == TaskStatus::Applied)
        .map(|t| (t.source.clone(), t.target.clone()))
        .collect();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(25))
        .redirect(reqwest::redirect::Policy::none())
        .build();
    for task in &mut plan.posters {
        task.target = relocated(&task.target, &moves);
        if task.status != TaskStatus::Ready {
            continue;
        }
        if fs::symlink_metadata(&task.target).is_ok() {
            task.status = TaskStatus::Unchanged;
            task.message = "Existing poster kept".into();
            continue;
        }
        let result = async {
            let url = reqwest::Url::parse(&task.url).context("Invalid poster URL")?;
            if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
                bail!("Poster URL must use HTTPS without credentials");
            }
            let mut response = client
                .as_ref()
                .map_err(|_| anyhow::anyhow!("Could not create image client"))?
                .get(url)
                .send()
                .await
                .map_err(|_| anyhow::anyhow!("Could not fetch poster"))?;
            if !response.status().is_success() {
                bail!("Poster provider returned HTTP {}", response.status());
            }
            if response
                .content_length()
                .is_some_and(|n| n > MAX_IMAGE as u64)
            {
                bail!("Poster exceeds the 10 MB limit");
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| anyhow::anyhow!("Poster download interrupted"))?
            {
                if bytes.len() + chunk.len() > MAX_IMAGE {
                    bail!("Poster exceeds the 10 MB limit");
                }
                bytes.extend_from_slice(&chunk);
            }
            save(&plan.root, &task.target, &bytes)
        }
        .await;
        match result {
            Ok(()) => task.status = TaskStatus::Applied,
            Err(error) => {
                task.status = TaskStatus::Failed;
                task.message = format!("{error:#}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{metadata::tvmaze_artwork, Artwork};
    use serde_json::json;

    #[test]
    fn parses_optional_artwork_without_inventing_season_posters() {
        let art = tvmaze_artwork(
            &json!({"image":{"original":"https://images.example/show.jpg"}}),
            &json!([{"number":1,"image":{"medium":"https://images.example/s1.jpg"}}, {"number":2,"image":null}]),
        );
        assert!(art.poster.is_some());
        assert!(art.seasons.contains_key(&1));
        assert!(!art.seasons.contains_key(&2));
        assert_eq!(
            tvmaze_artwork(&json!({"image":null}), &json!([])),
            Artwork::default()
        );
    }

    #[test]
    fn plans_season_folders_and_missing_seasons_without_overwriting_posters() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        fs::create_dir(root.join("s1")).unwrap();
        fs::write(root.join("poster.jpg"), b"existing").unwrap();
        let show = Show {
            artwork: Artwork {
                poster: Some("https://images.example/show.jpg".into()),
                seasons: [
                    (1, "https://images.example/s1.jpg".into()),
                    (2, "https://images.example/s2.jpg".into()),
                ]
                .into(),
            },
            ..Default::default()
        };
        let tasks = plan(
            &root,
            &PlanRequest {
                season: None,
                ..Default::default()
            },
            &show,
        )
        .unwrap();
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0].status, TaskStatus::Unchanged);
        assert!(tasks[1].target.ends_with("s1/poster.jpg"));
        assert!(tasks[2].target.ends_with("season02-poster.jpg"));
        assert!(!root.join("Season 02").exists());
    }

    #[test]
    fn poster_paths_follow_nested_moves_and_saves_never_replace_files() {
        let root = PathBuf::from("library");
        let moves = vec![
            (root.join("s1/s2"), root.join("s1/Season 02")),
            (root.join("s1"), root.join("Season 01")),
        ];
        assert_eq!(
            relocated(&root.join("s1/s2/poster.jpg"), &moves),
            root.join("Season 01/Season 02/poster.jpg")
        );
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let target = root.join("poster.jpg");
        assert!(save(&root, &target, b"not an image").is_err());
        assert!(!target.exists());
        save(&root, &target, &[0xff, 0xd8, 0xff, 0xd9]).unwrap();
        assert!(save(&root, &target, &[0xff, 0xd8, 0xff, 1]).is_err());
        assert_eq!(fs::read(target).unwrap(), [0xff, 0xd8, 0xff, 0xd9]);
    }
}
