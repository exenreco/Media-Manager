//! Durable, resumable inverse operations. All journal paths are relative to the
//! selected library and every mutation uses the same atomic no-replace primitive.
use crate::rename::{scoped_folder, RenamePlan, RenameTask};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fingerprint {
    pub path: String,
    pub directory: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Step {
    pub source: String,
    pub target: String,
    pub fingerprint: Vec<Fingerprint>,
    pub applied: bool,
    pub undone: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryRecord {
    pub id: Uuid,
    pub title: String,
    pub created: u64,
    pub steps: Vec<Step>,
}
impl HistoryRecord {
    pub fn new(plan: &RenamePlan) -> Self {
        Self {
            id: plan.id,
            title: plan.show.name.clone(),
            created: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            steps: vec![],
        }
    }
    pub fn remaining(&self) -> usize {
        self.steps.iter().filter(|s| !s.undone).count()
    }
}
pub fn fingerprint(path: &Path) -> Result<Vec<Fingerprint>> {
    let mut result = vec![];
    let entries = walkdir::WalkDir::new(path).follow_links(false);
    for entry in entries {
        let entry = entry?;
        if entry.file_type().is_symlink() {
            bail!("Undo cannot follow symbolic links");
        }
        let metadata = entry.metadata()?;
        result.push(Fingerprint {
            path: entry
                .path()
                .strip_prefix(path)?
                .to_string_lossy()
                .replace('\\', "/"),
            directory: metadata.is_dir(),
            size: if metadata.is_file() {
                metadata.len()
            } else {
                0
            },
            modified: if metadata.is_file() {
                metadata.modified().ok()
            } else {
                None
            },
        });
        if result.len() > 20000 {
            bail!("Undo snapshot exceeds 20,000 entries");
        }
    }
    result.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(result)
}
fn journal_dir(root: &Path) -> Result<PathBuf> {
    let parent = root.join(".media-manager");
    if fs::symlink_metadata(&parent).is_ok_and(|m| m.file_type().is_symlink()) {
        bail!("History directory must not be a symbolic link");
    }
    fs::create_dir_all(&parent)?;
    let folder = parent.join("history");
    if fs::symlink_metadata(&folder).is_ok_and(|m| m.file_type().is_symlink()) {
        bail!("History directory must not be a symbolic link");
    }
    fs::create_dir_all(&folder)?;
    Ok(folder)
}
pub fn store(root: &Path, record: &HistoryRecord) -> Result<()> {
    let directory = journal_dir(root)?;
    let bytes = serde_json::to_vec_pretty(record)?;
    if bytes.len() > 16 * 1024 * 1024 {
        bail!("Rename journal exceeds the 16 MB limit; use a smaller batch");
    }
    let mut file = tempfile::NamedTempFile::new_in(&directory)?;
    file.write_all(&bytes)?;
    file.as_file().sync_all()?;
    file.persist(directory.join(format!("{}.json", record.id)))
        .map_err(|e| e.error)?;
    Ok(())
}
fn load(root: &Path, id: Uuid) -> Result<HistoryRecord> {
    let path = journal_dir(root)?.join(format!("{id}.json"));
    if fs::symlink_metadata(&path)?.file_type().is_symlink()
        || fs::metadata(&path)?.len() > 16 * 1024 * 1024
    {
        bail!("Invalid history file");
    }
    let record: HistoryRecord = serde_json::from_slice(&fs::read(path)?)?;
    if record.id != id {
        bail!("History ID mismatch");
    }
    Ok(record)
}
pub fn list(root: &Path) -> Result<Vec<HistoryRecord>> {
    let mut records = vec![];
    for entry in fs::read_dir(journal_dir(root)?)? {
        let path = entry?.path();
        if let Some(id) = path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| Uuid::parse_str(s).ok())
        {
            records.push(load(root, id)?);
        }
    }
    records.sort_by_key(|r| std::cmp::Reverse(r.created));
    records.truncate(100);
    Ok(records)
}
fn checked_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let path = Path::new(relative);
    if relative.is_empty()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        || relative.contains('\\')
    {
        bail!("Invalid path in rename history");
    }
    let parent = path.parent().unwrap_or(Path::new(""));
    scoped_folder(root, &parent.to_string_lossy())?;
    Ok(root.join(path))
}
pub fn prepare(root: &Path, task: &RenameTask) -> Result<Step> {
    Ok(Step {
        source: task
            .source
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/"),
        target: task
            .target
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/"),
        fingerprint: fingerprint(&task.source)?,
        applied: false,
        undone: false,
    })
}
/// Account for app-created artwork before a user later asks to undo folder moves.
pub fn finish(plan: &RenamePlan) -> Result<()> {
    if !plan
        .tasks
        .iter()
        .any(|t| t.status == crate::rename::TaskStatus::Applied)
    {
        return Ok(());
    }
    let mut record = load(&plan.root, plan.id)?;
    for (i, step) in record.steps.clone().iter().enumerate() {
        if !step.applied || step.undone || !step.fingerprint.first().is_some_and(|f| f.directory) {
            continue;
        }
        let mut path = PathBuf::from(&step.target);
        for later in record.steps.iter().skip(i + 1).filter(|s| s.applied) {
            if let Ok(suffix) = path.strip_prefix(&later.source) {
                path = Path::new(&later.target).join(suffix);
            }
        }
        record.steps[i].fingerprint = fingerprint(&plan.root.join(path))?;
    }
    store(&plan.root, &record)
}
pub fn undo(root: &Path, id: Uuid) -> Result<HistoryRecord> {
    let root = root.canonicalize()?;
    let mut record = load(&root, id)?;
    for index in (0..record.steps.len()).rev() {
        let step = &record.steps[index];
        if step.undone {
            continue;
        }
        let source = checked_path(&root, &step.source)?;
        let target = checked_path(&root, &step.target)?;
        if source.parent() != target.parent() {
            bail!("Invalid directory change in history");
        }
        // A crash between rename and journal update can leave an intent pending.
        if !step.applied && source.exists() {
            record.steps[index].undone = true;
            store(&root, &record)?;
            continue;
        }
        if source.exists() && !target.exists() && fingerprint(&source)? == step.fingerprint {
            record.steps[index].undone = true;
            store(&root, &record)?;
            continue;
        }
        if source.exists() {
            bail!(
                "Undo conflict: original destination already exists ({})",
                step.source
            );
        }
        if fingerprint(&target)? != step.fingerprint {
            bail!("Undo refused: files changed since rename ({})", step.target);
        }
        crate::move_file::no_replace(&target, &source)
            .context("Could not restore the original name without replacing files")?;
        record.steps[index].undone = true;
        store(&root, &record)?;
    }
    Ok(record)
}
