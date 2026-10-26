use crate::rename::{RenamePlan, TaskStatus};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HealthReport {
    pub files: usize,
    pub missing_episodes: usize,
    pub unmatched: Vec<String>,
    pub missing_posters: Vec<String>,
    pub duplicates: Vec<Vec<String>>,
    pub conflicts: usize,
}
/// Hash only groups with the same nonzero size; equal-size files alone are never
/// advertised as duplicates. Read in bounded chunks without loading media into RAM.
pub fn scan(plan: &RenamePlan) -> Result<HealthReport> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut report = HealthReport {
        missing_episodes: plan.missing.len(),
        ..Default::default()
    };
    let mut sizes = BTreeMap::<u64, Vec<String>>::new();
    for task in plan
        .tasks
        .iter()
        .filter(|t| !t.directory && crate::rename::is_video(&t.source))
    {
        let relative = task.source.strip_prefix(&plan.root)?;
        crate::rename::scoped_folder(
            &plan.root,
            &relative.parent().unwrap_or(Path::new("")).to_string_lossy(),
        )?;
        report.files += 1;
        if task.status == TaskStatus::Skipped {
            report.unmatched.push(task.source.to_string_lossy().into());
        }
        if task.status == TaskStatus::Conflict {
            report.conflicts += 1;
        }
        let metadata = fs::symlink_metadata(&task.source)?;
        if metadata.is_file() && !metadata.file_type().is_symlink() && metadata.len() > 0 {
            sizes
                .entry(metadata.len())
                .or_default()
                .push(task.source.to_string_lossy().into());
        }
    }
    if plan.show.artwork.poster.is_none() {
        report.missing_posters.push("Main poster".into());
    }
    let seasons: std::collections::BTreeSet<_> =
        plan.show.episodes.iter().map(|e| e.season).collect();
    for season in seasons {
        if !plan.show.artwork.seasons.contains_key(&season) {
            report.missing_posters.push(format!("Season {season:02}"));
        }
    }
    for paths in sizes.values().filter(|v| v.len() > 1) {
        let mut hashes = BTreeMap::<Vec<u8>, Vec<String>>::new();
        for path in paths {
            let mut file = fs::File::open(Path::new(path))?;
            let mut hash = Sha256::new();
            let mut buffer = [0u8; 65536];
            loop {
                let n = file.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                hash.update(&buffer[..n]);
            }
            hashes
                .entry(hash.finalize().to_vec())
                .or_default()
                .push(path.clone());
        }
        report
            .duplicates
            .extend(hashes.into_values().filter(|v| v.len() > 1));
    }
    Ok(report)
}
