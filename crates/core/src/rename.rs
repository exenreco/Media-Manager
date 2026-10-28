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
    let series = if request.plex || request.jellyfin {
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
    Ok(if request.jellyfin {
        format!("{} {name}", sanitize(&series))
    } else if request.plex {
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

/// Containers recognized by the media library; decoding depends on the selected engine.
pub fn video_mime(path: &Path) -> Option<&'static str> {
    Some(
        match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
            "mp4" | "m4v" => "video/mp4",
            "mkv" => "video/x-matroska",
            "avi" | "divx" => "video/x-msvideo",
            "mov" | "qt" => "video/quicktime",
            "webm" => "video/webm",
            "ts" | "m2ts" | "mts" => "video/mp2t",
            "mpeg" | "mpg" | "mpe" | "m2v" | "vob" => "video/mpeg",
            "wmv" | "asf" => "video/x-ms-asf",
            "flv" | "f4v" => "video/x-flv",
            "ogv" | "ogm" => "video/ogg",
            "3gp" => "video/3gpp",
            "3g2" => "video/3gpp2",
            "rm" | "rmvb" => "application/vnd.rn-realmedia",
            _ => return None,
        },
    )
}
pub fn is_video(path: &Path) -> bool {
    video_mime(path).is_some()
}
pub fn is_companion(path: &Path) -> bool {
    path.extension().and_then(|s| s.to_str()).is_some_and(|s| {
        [
            "srt", "ass", "ssa", "vtt", "sub", "idx", "mka", "aac", "flac", "nfo",
        ]
        .contains(&s.to_ascii_lowercase().as_str())
    })
}

pub fn media_files(folder: &Path) -> Result<Vec<PathBuf>> {
    let mut files = vec![];
    for entry in WalkDir::new(folder)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| entry.file_name() != ".media-manager")
    {
        let entry = entry.context("Could not read part of the media folder")?;
        if entry.file_type().is_file() {
            let extension = entry
                .path()
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if is_video(entry.path()) || extension == "txt" {
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
        bail!("No supported video or .txt files found");
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
            if request.plex || request.jellyfin {
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
            let mapped = explicit_episode(folder, file, &request.options.episode_map)?;
            let detected = detected_episode(file);
            let episode = if let Some((s, n)) = mapped.or_else(|| {
                if request.options.numbering != crate::Numbering::Sequence {
                    detected
                } else {
                    None
                }
            }) {
                show.episodes
                    .iter()
                    .find(|e| e.season == s && e.number == n)
            } else if request.options.numbering == crate::Numbering::Absolute {
                absolute_number(file)
                    .and_then(|number| number.checked_sub(request.options.absolute_start.max(1)))
                    .and_then(|index| {
                        show.episodes
                            .iter()
                            .filter(|e| e.season != 0)
                            .nth(index as usize)
                    })
            } else if request.options.numbering == crate::Numbering::Detected {
                None
            } else {
                season.and_then(|s| {
                    let index = counters.entry(s).or_default();
                    let episode = by_season.get(&s).and_then(|v| v.get(*index)).copied();
                    *index += 1;
                    episode
                })
            };
            task.season = episode.map(|e| e.season).or(season);
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
    if request.options.companions {
        append_companions(&mut tasks)?;
    }
    if (request.plex || request.jellyfin) && !request.movie {
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

fn detected_episode(path: &Path) -> Option<(u32, u32)> {
    let name = path.file_stem()?.to_str()?;
    let pattern = Regex::new(r"(?i)(?:^|[^a-z0-9])s(\d{1,4})[ ._-]*e(\d{1,4})(?:[^0-9]|$)").ok()?;
    let captures = pattern.captures(name)?;
    Some((captures[1].parse().ok()?, captures[2].parse().ok()?))
}
fn absolute_number(path: &Path) -> Option<u32> {
    let name = path.file_stem()?.to_str()?;
    let without_tags = Regex::new(r"\[[^\]]*\]")
        .ok()?
        .replace_all(name, "")
        .into_owned();
    let pattern = Regex::new(r"(?i)(?:^| - |\bep?)[ ._-]*(\d{1,5})(?:v\d)?(?:$|[ ._-])").ok()?;
    let captures = pattern.captures(without_tags.trim())?;
    captures[1].parse().ok()
}
fn explicit_episode(root: &Path, path: &Path, mapping: &str) -> Result<Option<(u32, u32)>> {
    let relative = path
        .strip_prefix(root)?
        .to_string_lossy()
        .replace('\\', "/");
    let mut selected = None;
    for line in mapping.lines().filter(|line| !line.trim().is_empty()) {
        let cells: Vec<_> = line.split('|').map(str::trim).collect();
        if cells.len() != 3 {
            bail!("Episode mapping must use path|season|episode on each line");
        }
        let season = cells[1].parse::<u32>().context("Invalid mapping season")?;
        let number = cells[2].parse::<u32>().context("Invalid mapping episode")?;
        if cells[0].replace('\\', "/") == relative {
            if selected.is_some() {
                bail!("Duplicate episode mapping for {relative}");
            }
            selected = Some((season, number));
        }
    }
    Ok(selected)
}
fn append_companions(tasks: &mut Vec<RenameTask>) -> Result<()> {
    let videos: Vec<_> = tasks
        .iter()
        .filter(|t| !t.directory && is_video(&t.source))
        .cloned()
        .collect();
    let mut seen = HashSet::new();
    for video in &videos {
        if video.status == TaskStatus::Skipped {
            continue;
        }
        let old = video.source.file_stem().unwrap().to_string_lossy();
        let new = video.target.file_stem().unwrap().to_string_lossy();
        for entry in fs::read_dir(video.source.parent().unwrap())? {
            let path = entry?.path();
            if !is_companion(&path) || fs::symlink_metadata(&path)?.file_type().is_symlink() {
                continue;
            }
            let filename = path.file_name().unwrap().to_string_lossy();
            let Some(suffix) = filename
                .strip_prefix(old.as_ref())
                .filter(|s| s.starts_with('.'))
            else {
                continue;
            };
            // Prefer the longest matching video stem, e.g. Pilot.extended.en.srt.
            if videos.iter().any(|other| {
                other.source.parent() == video.source.parent()
                    && other.source.file_stem().is_some_and(|stem| {
                        let stem = stem.to_string_lossy();
                        stem.len() > old.len()
                            && filename
                                .strip_prefix(stem.as_ref())
                                .is_some_and(|rest| rest.starts_with('.'))
                    })
            }) {
                continue;
            }
            if !seen.insert(path.clone()) {
                continue;
            }
            let target = path.with_file_name(format!("{new}{suffix}"));
            let metadata = fs::metadata(&path)?;
            let status = if path == target {
                TaskStatus::Unchanged
            } else if target.exists() {
                TaskStatus::Conflict
            } else {
                TaskStatus::Ready
            };
            tasks.push(RenameTask {
                source: path,
                target,
                status,
                message: "Companion file".into(),
                directory: false,
                season: video.season,
                length: metadata.len(),
                modified: metadata.modified().ok(),
            });
        }
    }
    Ok(())
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
    let original_source = task.source.clone();
    let previous_target = task.target.clone();
    task.target = task.source.with_file_name(filename);
    task.status = TaskStatus::Ready;
    task.message.clear();
    if is_video(&original_source) {
        let old = previous_target.file_stem().unwrap().to_string_lossy();
        let new = Path::new(filename).file_stem().unwrap().to_string_lossy();
        for companion in plan
            .tasks
            .iter_mut()
            .filter(|t| is_companion(&t.source) && t.source.parent() == original_source.parent())
        {
            if let Some(suffix) = companion
                .target
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|name| name.strip_prefix(old.as_ref()))
                .filter(|s| s.starts_with('.'))
            {
                companion.target = companion.target.with_file_name(format!("{new}{suffix}"));
            }
        }
    }
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
    if task.directory || !is_video(&task.source) {
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
    let mut journal = crate::history::HistoryRecord::new(plan);
    if plan
        .tasks
        .iter()
        .any(|task| task.status == TaskStatus::Ready)
    {
        crate::history::store(&plan.root, &journal)?;
    }
    for task in plan
        .tasks
        .iter_mut()
        .filter(|t| t.status == TaskStatus::Ready)
    {
        let result = (|| -> Result<()> {
            check_source(&plan.root, task)?;
            journal
                .steps
                .push(crate::history::prepare(&plan.root, task)?);
            crate::history::store(&plan.root, &journal)?;
            crate::move_file::no_replace(&task.source, &task.target)
                .context("Could not rename without replacing an existing destination")?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                task.status = TaskStatus::Applied;
                if let Some(step) = journal.steps.last_mut() {
                    step.applied = true;
                }
                crate::history::store(&plan.root, &journal)?;
            }
            Err(error) => {
                task.status = TaskStatus::Failed;
                task.message = format!("{error:#}");
            }
        }
    }
    Ok(())
}
