//! Native document-provider access. The Rust planner sees a names-only mirror;
//! media bytes remain with the provider and every mutation is checked natively.
#![cfg_attr(test, allow(dead_code))]
use anyhow::{bail, Context, Result};
use dioxus::prelude::*;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::HashMap, path::PathBuf, sync::Mutex};
use tv_manager_core::{
    rename::{self, RenamePlan, TaskStatus},
    PlanRequest,
};

#[cfg(target_os = "ios")]
use crate::ads::{attach_storage, detach_storage, MediaManagerStorage};

#[component]
pub fn Bridge() -> Element {
    #[cfg(target_os = "ios")]
    let native = use_hook(|| {
        use dioxus::mobile::tao::platform::ios::WindowExtIOS;
        let plugin = MediaManagerStorage::new().ok();
        if let Some(plugin) = &plugin {
            let controller = dioxus::mobile::window().window.ui_view_controller();
            let _ = attach_storage(plugin, controller as u64);
        }
        std::rc::Rc::new(plugin)
    });
    #[cfg(target_os = "ios")]
    use_drop(move || {
        if let Some(plugin) = native.as_ref() {
            let _ = detach_storage(plugin);
        }
    });
    rsx! {}
}

async fn command(value: Value) -> Result<Value> {
    let mut eval = document::eval(include_str!("mobile-storage.js"));
    eval.send(value)?;
    let response: Value = eval.recv().await?;
    if let Some(error) = response.get("error").and_then(Value::as_str) {
        bail!("{error}");
    }
    Ok(response)
}

pub async fn pick() -> Result<String> {
    let result = command(json!({"op":"pick"})).await?;
    Ok(result["root"]
        .as_str()
        .context("No folder selected")?
        .into())
}

#[derive(Clone, Deserialize)]
struct Entry {
    path: String,
    dir: bool,
    size: u64,
    modified: i64,
    identity: String,
}
struct Library {
    _directory: tempfile::TempDir,
    mirror: PathBuf,
    root: String,
    plan: String,
    entries: HashMap<String, Entry>,
}
static LIBRARY: Mutex<Option<Library>> = Mutex::new(None);

fn relative(root: &std::path::Path, path: &std::path::Path) -> Result<String> {
    Ok(path
        .strip_prefix(root)?
        .iter()
        .map(|p| p.to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

fn mirror(
    entries: &[Entry],
    name: &str,
    cache: Option<&std::path::Path>,
) -> Result<(tempfile::TempDir, PathBuf)> {
    if entries.len() > 20000 {
        bail!("Folder exceeds 20,000 entries");
    }
    let directory = match cache {
        Some(path) => tempfile::tempdir_in(path)?,
        None => tempfile::tempdir()?,
    };
    // Preserve the selected folder name for season detection.
    let mirror = directory.path().join(rename::sanitize(name));
    std::fs::create_dir(&mirror)?;
    let mirror = mirror.canonicalize()?;
    for entry in entries {
        let path = std::path::Path::new(&entry.path);
        if path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
            || entry.path.is_empty()
            || entry.path.contains('\\')
        {
            bail!("Provider returned an unsafe path");
        }
        let target = mirror.join(path);
        if entry.dir {
            std::fs::create_dir_all(target)?;
        } else {
            std::fs::create_dir_all(target.parent().unwrap())?;
            std::fs::File::create(target)?;
        }
    }
    Ok((directory, mirror))
}

pub async fn preview(root: &str, request: &PlanRequest) -> Result<RenamePlan> {
    if root.is_empty() {
        bail!("Choose a media folder first");
    }
    let response = command(json!({"op":"list", "root":root})).await?;
    let entries: Vec<Entry> = serde_json::from_value(response["entries"].clone())?;
    let (directory, mirror) = mirror(
        &entries,
        response["name"].as_str().unwrap_or("Library"),
        Some(std::path::Path::new(
            response["cache"]
                .as_str()
                .context("Native cache directory unavailable")?,
        )),
    )?;
    let plan = rename::preview(&mirror, request).await?;
    *LIBRARY.lock().unwrap() = Some(Library {
        mirror,
        _directory: directory,
        root: root.into(),
        plan: plan.id.to_string(),
        entries: entries.into_iter().map(|e| (e.path.clone(), e)).collect(),
    });
    Ok(plan)
}

pub async fn apply(mut plan: RenamePlan) -> Result<RenamePlan> {
    if plan.tasks.iter().any(|t| t.status == TaskStatus::Conflict) {
        bail!("Resolve preview conflicts first");
    }
    let library = LIBRARY
        .lock()
        .unwrap()
        .take()
        .context("Build a new preview before applying")?;
    if library.plan != plan.id.to_string() {
        bail!("Preview expired; build a new preview");
    }
    for task in &mut plan.tasks {
        if task.status != TaskStatus::Ready {
            continue;
        }
        let path = relative(&library.mirror, &task.source)?;
        let target = task
            .target
            .file_name()
            .context("Invalid filename")?
            .to_string_lossy();
        let entry = library
            .entries
            .get(&path)
            .context("Source not found in document snapshot")?;
        let result = command(json!({"op":"rename","root":library.root,"path":path,"name":target,"size":entry.size,"modified":entry.modified,"dir":entry.dir,"identity":entry.identity})).await;
        match result {
            Ok(_) => {
                task.status = TaskStatus::Applied;
                // Update only names in the mirror so poster paths follow season moves.
                if let Err(error) = std::fs::rename(&task.source, &task.target) {
                    task.message =
                        format!("Renamed on device; preview cache update failed: {error}");
                }
            }
            Err(error) => {
                task.status = TaskStatus::Failed;
                task.message = error.to_string();
            }
        }
    }
    tv_manager_core::posters::download(&mut plan).await;
    use base64::Engine;
    for poster in &mut plan.posters {
        if poster.status != TaskStatus::Applied {
            continue;
        }
        let path = relative(&library.mirror, &poster.target)?;
        let bytes = std::fs::read(&poster.target)?;
        match command(json!({"op":"write","root":library.root,"path":path,"data":base64::engine::general_purpose::STANDARD.encode(bytes)})).await {
            Ok(response) if response["kept"] == true => { poster.status=TaskStatus::Unchanged; poster.message="Existing poster kept".into(); }
            Ok(_) => (),
            Err(error) => { poster.status=TaskStatus::Failed; poster.message=error.to_string(); }
        }
    }
    // Retain the names-only cache for playback until the next preview.
    *LIBRARY.lock().unwrap() = Some(library);
    Ok(plan)
}

pub async fn play(plan: &RenamePlan, index: usize) -> Result<Option<String>> {
    let (root, path) = {
        let guard = LIBRARY.lock().unwrap();
        let library = guard.as_ref().context("Build a preview first")?;
        if library.plan != plan.id.to_string() {
            bail!("Preview expired");
        }
        let path = rename::playback_path(plan, index)?;
        (library.root.clone(), relative(&library.mirror, &path)?)
    };
    command(json!({"op":"play","root":root,"path":path})).await?;
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(path: &str, dir: bool) -> Entry {
        Entry {
            path: path.into(),
            dir,
            size: 900_000_000,
            modified: 123,
            identity: path.into(),
        }
    }
    #[test]
    fn provider_paths_cannot_escape_cache() {
        for path in [
            "../outside.mkv",
            "/outside.mkv",
            "Season 1/../../outside.mkv",
            "Season 1\\outside.mkv",
            "",
        ] {
            assert!(
                mirror(&[entry(path, false)], "Show", None).is_err(),
                "{path}"
            );
        }
    }
    #[tokio::test]
    async fn names_only_preview_preserves_seasons_conflicts_and_missing_episodes() {
        let entries = [
            entry("Season 1", true),
            entry("Season 1/01.mkv", false),
            entry("Season 1/S01E01 - Pilot.mkv", false),
        ];
        let (_dir, root) = mirror(&entries, "Show", None).unwrap();
        assert_eq!(
            std::fs::metadata(root.join("Season 1/01.mkv"))
                .unwrap()
                .len(),
            0
        );
        let request = PlanRequest {
            name: "Show".into(),
            season: Some(1),
            source: tv_manager_core::Source::Manual {
                titles: "Pilot\nSecond\nMissing".into(),
            },
            ..Default::default()
        };
        let plan = rename::preview(&root, &request).await.unwrap();
        assert!(plan.tasks.iter().any(|t| t.status == TaskStatus::Conflict));
        assert_eq!(plan.missing.len(), 1);
        assert!(plan
            .tasks
            .iter()
            .filter(|t| !t.directory)
            .all(|t| t.season == Some(1)));
    }
}
