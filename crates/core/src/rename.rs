use crate::{metadata, Episode, PlanRequest, Show, Source};
use anyhow::{bail, Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Component, Path, PathBuf},
    time::SystemTime,
};
use uuid::Uuid;
use walkdir::WalkDir;

pub fn sanitize(title: &str) -> String {
    let clean: String = title
        .chars()
        .filter(|c| !c.is_control() && !r#"\/:*?"<>|"#.contains(*c))
        .collect();
    let clean = clean.trim().trim_end_matches(['.', ' ']);
    if clean.is_empty() {
        return "Untitled".into();
    }
    let stem = clean.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'));
    if reserved {
        format!("_{clean}")
    } else {
        clean.into()
    }
}

pub fn season_in_name(name: &str) -> Option<u32> {
    let pattern = Regex::new(r"(?i)(?:\bseason\s*(\d+))|(?:\bs\s?(\d{1,3})\b)").unwrap();
    pattern
        .captures(name)
        .and_then(|m| m.get(1).or_else(|| m.get(2)))
        .and_then(|m| m.as_str().parse().ok())
}

pub fn episode_name(request: &PlanRequest, show: &Show, episode: &Episode) -> Result<String> {
    let series = if request.plex {
        show.year.map_or_else(
            || show.name.clone(),
            |year| format!("{} ({year})", show.name),
        )
    } else {
        show.name.clone()
    };
    if let Source::Batch { template } = &request.source {
        let name = template
            .replace("{Series Name}", &sanitize(&series))
            .replace("{Season:02d}", &format!("{:02}", episode.season))
            .replace("{Episode:02d}", &format!("{:02}", episode.number))
            .replace("{Season}", &episode.season.to_string())
            .replace("{Episode}", &episode.number.to_string());
        if name.contains(['{', '}']) {
            bail!("Unknown or incomplete template token");
        }
        if name.trim().is_empty() {
            bail!("Template produces an empty name");
        }
        return Ok(sanitize(&name));
    }
    let name = format!(
        "S{:02}E{:02} - {}",
        episode.season,
        episode.number,
        sanitize(&episode.name)
    );
    Ok(if request.plex {
        format!("{} - {name}", sanitize(&series))
    } else {
        name
    })
}

/// Only relative paths can be supplied by a remote client. Symlinks are rejected
/// at every level, including links which happen to resolve inside the root.
pub fn scoped_folder(root: &Path, relative: &str) -> Result<PathBuf> {
    let root = root.canonicalize().context("Media root is unavailable")?;
    let relative = Path::new(relative);
    let mut folder = root.clone();
    for component in relative.components() {
        match component {
            Component::CurDir => (),
            Component::Normal(name) => {
                folder.push(name);
                if fs::symlink_metadata(&folder)?.file_type().is_symlink() {
                    bail!("Symbolic links are not supported");
                }
            }
            _ => bail!("Choose a folder inside the configured media root"),
        }
    }
    let folder = folder.canonicalize()?;
    if !folder.starts_with(&root) || !folder.is_dir() {
        bail!("Folder is outside the media root or is not a directory");
    }
    Ok(folder)
}

pub fn media_files(folder: &Path) -> Result<Vec<PathBuf>> {
    let mut files = vec![];
    for entry in WalkDir::new(folder).follow_links(false) {
        let entry = entry.context("Could not read part of the media folder")?;
        if entry.file_type().is_file() {
            let extension = entry
                .path()
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if ["mp4", "mkv", "avi", "txt"].contains(&extension.as_str()) {
                files.push(entry.into_path());
            }
        }
    }
    files.sort_by(|a, b| natord::compare_ignore_case(&a.to_string_lossy(), &b.to_string_lossy()));
    Ok(files)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TaskStatus {
    Ready,
    Unchanged,
    Skipped,
    Conflict,
    Applied,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RenameTask {
    pub source: PathBuf,
    pub target: PathBuf,
    pub status: TaskStatus,
    pub message: String,
    pub directory: bool,
    /// Matched or detected season; independent of filename templates.
    #[serde(default)]
    pub season: Option<u32>,
    // These fields are captured during preview and never accepted from a client.
    #[serde(skip)]
    length: u64,
    #[serde(skip)]
    modified: Option<SystemTime>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RenamePlan {
    pub id: Uuid,
    pub show: Show,
    pub tasks: Vec<RenameTask>,
    pub missing: Vec<Episode>,
    #[serde(default)]
    pub posters: Vec<crate::posters::PosterTask>,
    #[serde(skip)]
    pub(crate) root: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilenameEdit {
    pub filename: String,
}

pub async fn preview(root: &Path, request: &PlanRequest) -> Result<RenamePlan> {
    let folder = scoped_folder(root, &request.folder)?;
    let files = media_files(&folder)?;
    if files.is_empty() {
        bail!("No supported files found (.mp4, .mkv, .avi, .txt)");
    }
    let show = metadata::lookup(request, files.len()).await?;
    build_plan(&folder, request, show, &files)
}

pub fn build_plan(
    folder: &Path,
    request: &PlanRequest,
    show: Show,
    files: &[PathBuf],
) -> Result<RenamePlan> {
    let canonical_folder = folder.canonicalize()?;
    let folder = canonical_folder.as_path();
    let mut by_season = BTreeMap::<u32, Vec<&Episode>>::new();
    for episode in &show.episodes {
        by_season.entry(episode.season).or_default().push(episode);
    }
    for episodes in by_season.values_mut() {
        episodes.sort_by_key(|e| e.number);
    }
    let mut counters = BTreeMap::<u32, usize>::new();
    let mut matched = HashSet::new();
    let mut tasks = vec![];
    for file in files {
        if fs::symlink_metadata(file)?.file_type().is_symlink() {
            bail!("Symbolic links are not supported");
        }
        let canonical_file = file.canonicalize()?;
        let file = &canonical_file;
        if !file.starts_with(folder) {
            bail!("Source is outside the selected folder");
        }
        let mut task = RenameTask {
            source: file.clone(),
            target: file.clone(),
            status: TaskStatus::Ready,
            message: String::new(),
            directory: false,
            season: None,
            length: fs::metadata(file)?.len(),
            modified: fs::metadata(file)?.modified().ok(),
        };
        let base = if request.movie {
            if request.plex {
                show.year.map_or_else(
                    || sanitize(&show.name),
                    |y| format!("{} ({y})", sanitize(&show.name)),
                )
            } else {
                sanitize(&show.name)
            }
        } else {
            let season = request.season.or_else(|| {
                file.strip_prefix(folder)
                    .ok()?
                    .parent()?
                    .ancestors()
                    .filter_map(|p| p.file_name()?.to_str().and_then(season_in_name))
                    .next()
                    .or_else(|| folder.file_name()?.to_str().and_then(season_in_name))
            });
            let episode = season.and_then(|s| {
                let index = counters.entry(s).or_default();
                let episode = by_season.get(&s).and_then(|v| v.get(*index)).copied();
                *index += 1;
                episode
            });
            task.season = season;
            let Some(episode) = episode else {
                task.status = TaskStatus::Skipped;
                task.message = "No matching episode or season folder".into();
                tasks.push(task);
                continue;
            };
            matched.insert((episode.season, episode.number));
            episode_name(request, &show, episode)?
        };
        let extension = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        let filename = format!("{base}.{extension}");
        if filename.len() > 240 {
            bail!("Generated filename is too long; shorten the title or template");
        }
        task.target = file.with_file_name(filename);
        if task.target == task.source {
            task.status = TaskStatus::Unchanged;
        } else if fs::symlink_metadata(&task.target).is_ok() {
            task.status = TaskStatus::Conflict;
            task.message = "Destination already exists".into();
        }
        tasks.push(task);
    }
    if request.plex && !request.movie {
        let mut folders = WalkDir::new(folder)
            .min_depth(1)
            .follow_links(false)
            .into_iter()
            .collect::<std::result::Result<Vec<_>, _>>()?;
        folders.sort_by_key(|e| std::cmp::Reverse(e.depth()));
        for entry in folders.into_iter().filter(|e| e.file_type().is_dir()) {
            let Some(season) = entry.file_name().to_str().and_then(season_in_name) else {
                continue;
            };
            let source = entry.into_path();
            let target = source.with_file_name(format!("Season {season:02}"));
            let status = if source == target {
                TaskStatus::Unchanged
            } else if fs::symlink_metadata(&target).is_ok() {
                TaskStatus::Conflict
            } else {
                TaskStatus::Ready
            };
            tasks.push(RenameTask {
                source,
                target,
                status,
                directory: true,
                season: Some(season),
                message: String::new(),
                length: 0,
                modified: None,
            });
        }
    }
    // Treat names case-insensitively everywhere for portable previews.
    let mut counts = BTreeMap::<String, usize>::new();
    for task in &tasks {
        if task.status != TaskStatus::Skipped {
            *counts
                .entry(task.target.to_string_lossy().to_lowercase())
                .or_default() += 1;
        }
    }
    for task in &mut tasks {
        if task.status != TaskStatus::Skipped
            && counts
                .get(&task.target.to_string_lossy().to_lowercase())
                .copied()
                .unwrap_or(0)
                > 1
        {
            task.status = TaskStatus::Conflict;
            task.message = "Multiple files map to this destination".into();
        }
    }
    let missing = show
        .episodes
        .iter()
        .filter(|e| !matched.contains(&(e.season, e.number)))
        .cloned()
        .collect();
    let posters = if request.download_posters {
        crate::posters::plan(folder, request, &show)?
    } else {
        vec![]
    };
    Ok(RenamePlan {
        id: Uuid::new_v4(),
        show,
        tasks,
        missing,
        posters,
        root: folder.canonicalize()?,
    })
}

fn check_source(root: &Path, task: &RenameTask) -> Result<()> {
    let relative = task
        .source
        .strip_prefix(root)
        .context("Source is outside the media root")?;
    scoped_folder(
        root,
        &relative.parent().unwrap_or(Path::new("")).to_string_lossy(),
    )?;
    if task.target.parent() != task.source.parent() {
        bail!("Target must stay in the source directory");
    }
    let metadata = fs::symlink_metadata(&task.source).context("Source is no longer available")?;
    if metadata.file_type().is_symlink()
        || (task.directory && !metadata.is_dir())
        || (!task.directory && !metadata.is_file())
    {
        bail!("Source type changed since preview");
    }
    if !task.directory
        && (metadata.len() != task.length || metadata.modified().ok() != task.modified)
    {
        bail!("Source changed since preview; build a new preview");
    }
    if fs::symlink_metadata(&task.target).is_ok() {
        bail!("Destination now exists; build a new preview");
    }
    Ok(())
}

/// Edit a filename in a preview only; paths and source fingerprints remain owned
/// by the backend. Resolve every affected collision again after an edit.
pub fn edit_target(plan: &mut RenamePlan, index: usize, filename: &str) -> Result<()> {
    if plan
        .tasks
        .iter()
        .any(|t| matches!(t.status, TaskStatus::Applied | TaskStatus::Failed))
    {
        bail!("Build a fresh preview before editing an applied plan");
    }
    if filename.is_empty()
        || filename.len() > 240
        || sanitize(filename) != filename
        || filename.contains(['/', '\\'])
    {
        bail!("Enter a valid filename without folder paths or reserved characters (maximum 240 bytes)");
    }
    let task = plan
        .tasks
        .get_mut(index)
        .context("File is not in this preview")?;
    if task.directory {
        bail!("Only file names can be edited");
    }
    let extension = Path::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let original = task
        .source
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if !extension.eq_ignore_ascii_case(original) {
        bail!("Keep the original .{original} file extension");
    }
    task.target = task.source.with_file_name(filename);
    task.status = TaskStatus::Ready;
    task.message.clear();
    let mut counts = BTreeMap::<String, usize>::new();
    for task in &plan.tasks {
        if task.status != TaskStatus::Skipped {
            *counts
                .entry(task.target.to_string_lossy().to_lowercase())
                .or_default() += 1;
        }
    }
    for task in &mut plan.tasks {
        if task.status == TaskStatus::Skipped {
            continue;
        }
        task.message.clear();
        task.status = if task.source == task.target {
            TaskStatus::Unchanged
        } else {
            TaskStatus::Ready
        };
        if counts
            .get(&task.target.to_string_lossy().to_lowercase())
            .copied()
            .unwrap_or(0)
            > 1
        {
            task.status = TaskStatus::Conflict;
            task.message = "Multiple files map to this destination".into();
        } else if task.source != task.target && fs::symlink_metadata(&task.target).is_ok() {
            task.status = TaskStatus::Conflict;
            task.message = "Destination already exists".into();
        }
    }
    Ok(())
}

pub fn playback_path(plan: &RenamePlan, index: usize) -> Result<PathBuf> {
    let task = plan
        .tasks
        .get(index)
        .context("File is not in this preview")?;
    let extension = task
        .source
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if task.directory || !["mp4", "mkv", "avi"].contains(&extension.as_str()) {
        bail!("Playback is only available for video files");
    }
    let mut path = if task.status == TaskStatus::Applied {
        task.target.clone()
    } else {
        task.source.clone()
    };
    for folder in plan
        .tasks
        .iter()
        .filter(|t| t.directory && t.status == TaskStatus::Applied)
    {
        if let Ok(relative) = path.strip_prefix(&folder.source) {
            path = folder.target.join(relative);
        }
    }
    let relative = path
        .strip_prefix(&plan.root)
        .context("Media is outside the library")?;
    scoped_folder(
        &plan.root,
        &relative.parent().unwrap_or(Path::new("")).to_string_lossy(),
    )?;
    let metadata = fs::symlink_metadata(&path)
        .context("Media file is no longer available; refresh the preview")?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        bail!("Media is not a regular file");
    }
    Ok(path)
}

/// Uses platform atomic no-replace rename operations. Unsupported filesystems
/// fail explicitly; there is no overwrite fallback. Files precede directories,
/// and directories run deepest-first so child paths remain valid.
/// A batch can partially succeed. Every result is retained in the returned plan.
pub fn apply(plan: &mut RenamePlan) -> Result<()> {
    if plan.tasks.iter().any(|t| t.status == TaskStatus::Conflict) {
        bail!("Resolve preview conflicts before applying");
    }
    for task in plan.tasks.iter().filter(|t| t.status == TaskStatus::Ready) {
        check_source(&plan.root, task)?;
    }
    for task in plan
        .tasks
        .iter_mut()
        .filter(|t| t.status == TaskStatus::Ready)
    {
        let result = (|| -> Result<()> {
            check_source(&plan.root, task)?;
            crate::move_file::no_replace(&task.source, &task.target)
                .context("Could not rename without replacing an existing destination")?;
            Ok(())
        })();
        match result {
            Ok(()) => task.status = TaskStatus::Applied,
            Err(error) => {
                task.status = TaskStatus::Failed;
                task.message = format!("{error:#}");
            }
        }
    }
    Ok(())
}
