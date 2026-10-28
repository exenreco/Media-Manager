//! Native document-provider access. The Rust planner sees a names-only mirror;
//! media bytes remain with the provider and every mutation is checked natively.
#![cfg_attr(test, allow(dead_code))]
use anyhow::{bail, Context, Result};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
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
    use_effect(|| {
        let _ = document::eval("const viewport = document.querySelector('meta[name=viewport]'); if (viewport) viewport.content = 'width=device-width, initial-scale=1, viewport-fit=cover';");
    });
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

#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn initialize_settings() -> Result<()> {
    let result = command(json!({"op":"appDirectories"})).await?;
    crate::session::initialize_mobile(PathBuf::from(
        result["state"]
            .as_str()
            .context("Native settings directory unavailable")?,
    ))
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn check_folder(root: &str) -> Result<()> {
    let response = command(json!({"op":"checkFolder","root":root})).await?;
    anyhow::ensure!(
        response["connected"] == true,
        "Folder permission is unavailable. Choose the folder again."
    );
    Ok(())
}

pub async fn pick() -> Result<String> {
    let result = command(json!({"op":"pick"})).await?;
    Ok(result["root"]
        .as_str()
        .context("No folder selected")?
        .into())
}

#[derive(Clone, Serialize, Deserialize)]
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
    state: PathBuf,
    folder: String,
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
        state: PathBuf::from(
            response["state"]
                .as_str()
                .context("Native state directory unavailable")?,
        ),
        folder: request.folder.clone(),
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
    let mut journal = MobileRecord {
        root: library.root.clone(),
        folder: library.folder.clone(),
        record: tv_manager_core::history::HistoryRecord::new(&plan),
    };
    save_mobile_record(&library.state, &journal)?;
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
        let before = native_entries(&library.root).await?;
        let target_path = relative(&library.mirror, &task.target)?;
        journal.record.steps.push(tv_manager_core::history::Step {
            source: path.clone(),
            target: target_path.clone(),
            fingerprint: native_fingerprint(&before, &path)?,
            applied: false,
            undone: false,
        });
        save_mobile_record(&library.state, &journal)?;
        let result = command(json!({"op":"rename","root":library.root,"path":path,"name":target,"size":entry.size,"modified":entry.modified,"dir":entry.dir,"identity":entry.identity})).await;
        match result {
            Ok(_) => {
                task.status = TaskStatus::Applied;
                journal.record.steps.last_mut().unwrap().applied = true;
                save_mobile_record(&library.state, &journal)?;
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
    let final_entries = native_entries(&library.root).await?;
    for (index, step) in journal.record.steps.clone().iter().enumerate() {
        if !step.applied {
            continue;
        }
        let mut path = std::path::PathBuf::from(&step.target);
        for later in journal
            .record
            .steps
            .iter()
            .skip(index + 1)
            .filter(|s| s.applied)
        {
            if let Ok(suffix) = path.strip_prefix(&later.source) {
                path = std::path::Path::new(&later.target).join(suffix);
            }
        }
        journal.record.steps[index].fingerprint =
            native_fingerprint(&final_entries, &path.to_string_lossy().replace('\\', "/"))?;
    }
    save_mobile_record(&library.state, &journal)?;
    // Retain the names-only cache for playback until the next preview.
    *LIBRARY.lock().unwrap() = Some(library);
    Ok(plan)
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn media_document(plan: &RenamePlan, index: usize) -> Result<(String, String)> {
    let guard = LIBRARY.lock().unwrap();
    let library = guard.as_ref().context("Build a preview first")?;
    if library.plan != plan.id.to_string() {
        bail!("Preview expired");
    }
    let path = rename::playback_path(plan, index)?;
    Ok((library.root.clone(), relative(&library.mirror, &path)?))
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn read_media(
    root: &str,
    path: &str,
    offset: u64,
    count: usize,
) -> Result<(u64, Vec<u8>)> {
    use base64::Engine;
    anyhow::ensure!(count <= 1024 * 1024, "Media read too large");
    let response =
        command(json!({"op":"readMedia","root":root,"path":path,"offset":offset,"count":count}))
            .await?;
    let size = response["size"].as_u64().context("Missing media size")?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(response["data"].as_str().unwrap_or(""))?;
    anyhow::ensure!(bytes.len() == count, "Incomplete media read");
    Ok((size, bytes))
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn scan_pairing() -> Result<String> {
    let response = command(json!({"op":"scanPairing"})).await?;
    Ok(response["text"]
        .as_str()
        .context("Scanner returned no pairing invitation")?
        .to_string())
}
#[derive(Serialize, Deserialize)]
struct MobileRecord {
    root: String,
    folder: String,
    record: tv_manager_core::history::HistoryRecord,
}
fn save_mobile_record(state: &std::path::Path, record: &MobileRecord) -> Result<()> {
    use std::io::Write;
    let directory = state.join("mobile-history");
    std::fs::create_dir_all(&directory)?;
    let mut file = tempfile::NamedTempFile::new_in(&directory)?;
    file.write_all(&serde_json::to_vec_pretty(record)?)?;
    file.as_file().sync_all()?;
    file.persist(directory.join(format!("{}.json", record.record.id)))
        .map_err(|e| e.error)?;
    Ok(())
}
async fn native_entries(root: &str) -> Result<Vec<Entry>> {
    let value = command(json!({"op":"list","root":root})).await?;
    Ok(serde_json::from_value(value["entries"].clone())?)
}
fn native_fingerprint(
    entries: &[Entry],
    path: &str,
) -> Result<Vec<tv_manager_core::history::Fingerprint>> {
    let entry = entries
        .iter()
        .find(|e| e.path == path)
        .context("Document is no longer available")?;
    let prefix = format!("{path}/");
    let mut result = vec![];
    for entry in entries
        .iter()
        .filter(|e| e.path == path || (entry.dir && e.path.starts_with(&prefix)))
    {
        result.push(tv_manager_core::history::Fingerprint {
            path: entry.path.strip_prefix(&prefix).unwrap_or("").into(),
            directory: entry.dir,
            size: if entry.dir { 0 } else { entry.size },
            modified: if !entry.dir && entry.modified > 0 {
                Some(
                    std::time::UNIX_EPOCH + std::time::Duration::from_millis(entry.modified as u64),
                )
            } else {
                None
            },
        });
    }
    result.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(result)
}
pub async fn history(
    root: &str,
    folder: &str,
) -> Result<Vec<tv_manager_core::history::HistoryRecord>> {
    let response = command(json!({"op":"list","root":root})).await?;
    let state = PathBuf::from(
        response["state"]
            .as_str()
            .context("Native state directory unavailable")?,
    );
    let directory = state.join("mobile-history");
    if !directory.exists() {
        return Ok(vec![]);
    }
    let mut records = vec![];
    for item in std::fs::read_dir(directory)? {
        let path = item?.path();
        if path.extension().is_none_or(|s| s != "json") {
            continue;
        }
        let saved: MobileRecord = serde_json::from_slice(&std::fs::read(path)?)?;
        if saved.root == root && saved.folder == folder {
            records.push(saved.record);
        }
    }
    records.sort_by_key(|r| std::cmp::Reverse(r.created));
    records.truncate(100);
    Ok(records)
}
pub async fn undo(
    root: &str,
    folder: &str,
    id: uuid::Uuid,
) -> Result<tv_manager_core::history::HistoryRecord> {
    let response = command(json!({"op":"list","root":root})).await?;
    let state = PathBuf::from(
        response["state"]
            .as_str()
            .context("Native state directory unavailable")?,
    );
    let mut saved: MobileRecord = serde_json::from_slice(&std::fs::read(
        state.join("mobile-history").join(format!("{id}.json")),
    )?)?;
    if saved.root != root || saved.folder != folder || saved.record.id != id {
        bail!("History belongs to a different library");
    }
    for index in (0..saved.record.steps.len()).rev() {
        let step = &saved.record.steps[index];
        if step.undone {
            continue;
        }
        let entries = native_entries(root).await?;
        let original = entries.iter().find(|e| e.path == step.source);
        let target = entries.iter().find(|e| e.path == step.target);
        if !step.applied && original.is_some() {
            saved.record.steps[index].undone = true;
            save_mobile_record(&state, &saved)?;
            continue;
        }
        if original.is_some() && target.is_none() {
            if step.applied && native_fingerprint(&entries, &step.source)? != step.fingerprint {
                bail!("Undo conflict: original path already exists");
            }
            saved.record.steps[index].undone = true;
            save_mobile_record(&state, &saved)?;
            continue;
        }
        if original.is_some() {
            bail!("Undo conflict: original destination exists");
        }
        let entry = target.context("Renamed document is missing")?;
        if native_fingerprint(&entries, &step.target)? != step.fingerprint {
            bail!("Document changed since rename; Undo stopped");
        }
        let source_path = std::path::Path::new(&step.source);
        let target_path = std::path::Path::new(&step.target);
        if source_path.parent() != target_path.parent() {
            bail!("Invalid history path");
        }
        let name = source_path
            .file_name()
            .context("Invalid original name")?
            .to_string_lossy();
        command(json!({"op":"rename","root":root,"path":step.target,"name":name,"size":entry.size,"modified":entry.modified,"dir":entry.dir,"identity":entry.identity})).await?;
        saved.record.steps[index].undone = true;
        save_mobile_record(&state, &saved)?;
    }
    *LIBRARY.lock().unwrap() = None;
    Ok(saved.record)
}
pub async fn health(
    root: &str,
    plan: &RenamePlan,
) -> Result<tv_manager_core::health::HealthReport> {
    let mut report = tv_manager_core::health::scan(plan)?;
    let entries = native_entries(root).await?;
    let sizes = {
        let guard = LIBRARY.lock().unwrap();
        let library = guard.as_ref().context("Build a preview first")?;
        if library.plan != plan.id.to_string() {
            bail!("Preview expired");
        }
        let mut sizes = std::collections::BTreeMap::<u64, Vec<String>>::new();
        for task in plan
            .tasks
            .iter()
            .filter(|t| !t.directory && rename::is_video(&t.source))
        {
            let path = relative(&library.mirror, &task.source)?;
            if let Some(entry) = entries
                .iter()
                .find(|e| e.path == path && !e.dir && e.size > 0)
            {
                sizes.entry(entry.size).or_default().push(path);
            }
        }
        sizes
    };
    for paths in sizes.values().filter(|v| v.len() > 1) {
        let mut hashes = std::collections::BTreeMap::<String, Vec<String>>::new();
        for path in paths {
            let result = command(json!({"op":"hash","root":root,"path":path})).await?;
            let hash = result["sha256"].as_str().context("Missing content hash")?;
            hashes.entry(hash.into()).or_default().push(path.clone());
        }
        report
            .duplicates
            .extend(hashes.into_values().filter(|v| v.len() > 1));
    }
    Ok(report)
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

#[cfg(any(target_os = "android", target_os = "ios"))]
fn artwork_context(plan: &RenamePlan) -> Result<(String, PathBuf)> {
    let guard = LIBRARY.lock().unwrap();
    let library = guard.as_ref().context("Build a preview first")?;
    anyhow::ensure!(library.plan == plan.id.to_string(), "Preview expired");
    Ok((library.root.clone(), library.mirror.clone()))
}
#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn artwork(plan: &RenamePlan) -> Result<Vec<tv_manager_core::artwork::Poster>> {
    let (root, mirror) = artwork_context(plan)?;
    let mut slots = tv_manager_core::artwork::slots(plan)?;
    // Poster paths are relative to the scoped preview folder, not the tree root.
    let folder = LIBRARY.lock().unwrap().as_ref().unwrap().folder.clone();
    let mut total = 0;
    for slot in &mut slots {
        if let Some(path) = &slot.local_path {
            let path = std::path::Path::new(&folder)
                .join(path)
                .to_string_lossy()
                .replace('\\', "/");
            let (size, _) = read_media(&root, &path, 0, 0).await?;
            anyhow::ensure!(size <= 10 * 1024 * 1024, "Poster exceeds 10 MB");
            total += size;
            anyhow::ensure!(total <= 64 * 1024 * 1024, "Poster gallery exceeds 64 MB");
            let mut bytes = Vec::new();
            while bytes.len() < (size as usize) {
                let count = (size as usize - bytes.len()).min(1024 * 1024);
                let (_, chunk) = read_media(&root, &path, bytes.len() as u64, count).await?;
                bytes.extend(chunk);
            }
            slot.fingerprint = Some(tv_manager_core::artwork::fingerprint(&bytes)?);
            if let Ok((uri, hash)) = tv_manager_core::artwork::image_data(&bytes) {
                slot.local = Some(uri);
                slot.fingerprint = Some(hash);
            }
        }
    }
    let _ = mirror;
    Ok(slots)
}
#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn save_artwork(
    plan: &RenamePlan,
    save: &tv_manager_core::artwork::Save,
) -> Result<Vec<tv_manager_core::artwork::Poster>> {
    use base64::Engine;
    let (root, mirror) = artwork_context(plan)?;
    let slot = tv_manager_core::artwork::validate_save(plan, save)?;
    let bytes = tv_manager_core::artwork::fetch(&save.url).await?;
    let relative_path = slot.local_path.as_ref().unwrap_or(&slot.id);
    let bytes = tv_manager_core::artwork::encode_for_path(&bytes, relative_path)?;
    let folder = LIBRARY.lock().unwrap().as_ref().unwrap().folder.clone();
    let path = std::path::Path::new(&folder)
        .join(relative_path)
        .to_string_lossy()
        .replace('\\', "/");
    command(json!({"op":"savePoster","root":root,"path":path,"data":base64::engine::general_purpose::STANDARD.encode(&bytes),"replace":save.replace,"expected":save.expected})).await?;
    // Retain existence in the names-only mirror for subsequent gallery reads.
    let marker = mirror.join(&folder).join(relative_path);
    std::fs::create_dir_all(marker.parent().unwrap())?;
    std::fs::write(marker, [])?;
    artwork(plan).await
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn download_artwork(plan: &RenamePlan) -> Result<tv_manager_core::artwork::Batch> {
    let mut result = tv_manager_core::artwork::Batch {
        posters: Vec::new(),
        downloaded: 0,
        kept: 0,
        failures: Vec::new(),
    };
    for slot in artwork(plan).await? {
        if slot.local_path.is_some() {
            result.kept += 1;
            continue;
        }
        let Some(url) = slot.provider else {
            continue;
        };
        let save = tv_manager_core::artwork::Save {
            slot: slot.id,
            url,
            replace: false,
            expected: None,
        };
        match save_artwork(plan, &save).await {
            Ok(_) => result.downloaded += 1,
            Err(e) => result.failures.push(format!("{}: {e:#}", slot.title)),
        }
    }
    result.posters = artwork(plan).await?;
    Ok(result)
}

#[cfg(target_os = "android")]
pub async fn native_preview(root: &str, path: &str, mime: &str) -> Result<(String, u64)> {
    let value =
        command(json!({"op":"prepareMediaPreview","root":root,"path":path,"mime":mime})).await?;
    preview_token(value)
}
#[cfg(target_os = "android")]
pub async fn remote_preview(
    url: &str,
    certificate: &str,
    size: u64,
    mime: &str,
) -> Result<(String, u64)> {
    let value=command(json!({"op":"prepareRemotePreview","url":url,"certificate":certificate,"size":size,"mime":mime})).await?;
    preview_token(value)
}
#[cfg(target_os = "android")]
fn preview_token(value: serde_json::Value) -> Result<(String, u64)> {
    let token = value["token"].as_str().context("Missing preview token")?;
    uuid::Uuid::parse_str(token).context("Invalid preview token")?;
    Ok((
        format!("/native-media-preview/{token}"),
        value["size"].as_u64().context("Missing media size")?,
    ))
}

#[cfg(target_os = "android")]
pub async fn play_compatible_preview(source: String) -> Result<()> {
    command(json!({"op":"playCompatiblePreview","source":source})).await?;
    Ok(())
}

#[cfg(target_os = "ios")]
pub async fn play_compatible_document(plan: &RenamePlan, index: usize) -> Result<()> {
    let (root, path) = media_document(plan, index)?;
    command(json!({"op":"playCompatiblePreview","root":root,"path":path})).await?;
    Ok(())
}
