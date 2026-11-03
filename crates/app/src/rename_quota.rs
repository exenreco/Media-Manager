//! Persistent desktop free-tier accounting, independent of purchase transport.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

pub const FREE_LIMIT: u32 = 2;
pub const COOLDOWN_SECONDS: u64 = 2 * 60 * 60;

#[cfg(any(target_os = "macos", windows))]
pub fn path() -> Result<PathBuf> {
    Ok(crate::session::path()?.with_file_name("rename-usage.json"))
}
#[cfg(any(target_os = "macos", windows))]
pub fn now() -> Result<u64> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs())
}
pub fn title_key(show: &tv_manager_core::Show) -> Result<String> {
    let title = show
        .name
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    ensure!(
        !title.is_empty(),
        "A matched movie or TV show title is required"
    );
    Ok(format!(
        "{title}:{}",
        show.year.map(|year| year.to_string()).unwrap_or_default()
    ))
}
#[cfg(any(target_os = "macos", windows))]
pub fn summary() -> Result<String> {
    let mut usage: Usage = match fs::read(path()?) {
        Ok(bytes) => serde_json::from_slice(&bytes)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Usage::default(),
        Err(e) => return Err(e.into()),
    };
    let now = usage.refresh(now()?);
    Ok(if usage.cooldown_until != 0 {
        format!(
            "Cooldown: {} minutes remaining.",
            usage.cooldown_until.saturating_sub(now).div_ceil(60)
        )
    } else {
        format!(
            "{} of 2 distinct titles used. Episodes of the same show share one title.",
            usage.titles.len()
        )
    })
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct Usage {
    titles: Vec<String>,
    cooldown_until: u64,
    last_seen: u64,
}

impl Usage {
    fn refresh(&mut self, now: u64) -> u64 {
        // Moving the system clock backwards must not immediately reset usage.
        let now = now.max(self.last_seen);
        self.last_seen = now;
        if self.cooldown_until != 0 && now >= self.cooldown_until {
            self.titles.clear();
            self.cooldown_until = 0;
        }
        now
    }

    fn reserve(&mut self, title: &str, now: u64) -> Result<bool> {
        let now = self.refresh(now);
        ensure!(
            self.cooldown_until == 0,
            "Free plan cooldown: try again in {} minutes, or go Pro for unlimited titles.",
            self.cooldown_until.saturating_sub(now).div_ceil(60)
        );
        ensure!(
            !title.is_empty(),
            "A matched movie or TV show title is required"
        );
        if self.titles.iter().any(|entry| entry == title) {
            return Ok(false);
        }
        self.titles.push(title.into());
        if self.titles.len() == FREE_LIMIT as usize {
            self.cooldown_until = now.saturating_add(COOLDOWN_SECONDS);
        }
        Ok(true)
    }
}

/// Holds a cross-process lock until the rename result is recorded. Reserving
/// before I/O prevents concurrent windows or a restart from exceeding the limit.
pub struct Reservation {
    _lock: File,
    path: PathBuf,
    usage: Usage,
    new_title: bool,
}

fn save(path: &Path, usage: &Usage) -> Result<()> {
    let parent = path.parent().context("Invalid usage directory")?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(&serde_json::to_vec(usage)?)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|e| e.error)?;
    Ok(())
}

impl Reservation {
    pub fn acquire(path: PathBuf, title: &str, now: u64) -> Result<Self> {
        fs::create_dir_all(path.parent().context("Invalid usage directory")?)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path.with_extension("lock"))?;
        lock.try_lock()
            .context("Another rename is in progress; try again when it finishes")?;
        let mut usage: Usage = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).context("Could not read rename usage")?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Usage::default(),
            Err(e) => return Err(e.into()),
        };
        ensure!(
            usage.titles.len() <= FREE_LIMIT as usize,
            "Invalid rename usage"
        );
        let new_title = usage.reserve(title, now)?;
        save(&path, &usage)?;
        Ok(Self {
            _lock: lock,
            path,
            usage,
            new_title,
        })
    }

    /// Known failed/skipped operations are refunded. An interrupted operation
    /// retains its reservation because its filesystem outcome is unknown.
    pub fn finish(mut self, successful: bool) -> Result<()> {
        if !successful && self.new_title {
            self.usage.titles.pop();
            self.usage.cooldown_until = 0;
        }
        save(&self.path, &self.usage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_rename_starts_cooldown_and_exact_boundary_reopens() {
        let mut usage = Usage::default();
        usage.reserve("show-a", 100).unwrap();
        assert!(!usage.reserve("show-a", 150).unwrap());
        assert_eq!(usage.cooldown_until, 0);
        usage.reserve("movie-b", 200).unwrap();
        assert!(usage.reserve("show-a", 7399).is_err());
        usage.reserve("show-c", 7400).unwrap();
        assert_eq!(usage.titles, ["show-c"]);
    }

    #[test]
    fn clock_rollback_does_not_bypass_limit() {
        let mut usage = Usage::default();
        usage.reserve("a", 100).unwrap();
        usage.reserve("b", 100).unwrap();
        assert!(usage.reserve("c", 0).is_err());
    }

    #[test]
    fn title_identity_ignores_episode_lists_case_and_spacing_but_keeps_year() {
        let mut show = tv_manager_core::Show {
            name: "  Example   SHOW ".into(),
            year: Some(2020),
            ..Default::default()
        };
        let original = title_key(&show).unwrap();
        show.name = "example show".into();
        assert_eq!(title_key(&show).unwrap(), original);
        show.year = Some(2021);
        assert_ne!(title_key(&show).unwrap(), original);
        show.name.clear();
        assert!(title_key(&show).is_err());
    }

    #[test]
    fn usage_survives_restart_and_only_successful_results_count() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("usage.json");
        Reservation::acquire(path.clone(), "failed", 90)
            .unwrap()
            .finish(false)
            .unwrap();
        Reservation::acquire(path.clone(), "a", 100)
            .unwrap()
            .finish(true)
            .unwrap();
        let reservation = Reservation::acquire(path.clone(), "b", 200).unwrap();
        assert!(Reservation::acquire(path.clone(), "c", 200).is_err());
        drop(reservation); // Simulate interruption: retain the reserved slot.
        assert!(Reservation::acquire(path.clone(), "c", 201).is_err());
        Reservation::acquire(path, "c", 7400)
            .unwrap()
            .finish(true)
            .unwrap();
    }

    #[test]
    fn corrupted_usage_fails_closed() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("usage.json");
        fs::write(&path, "invalid").unwrap();
        assert!(Reservation::acquire(path, "a", 100).is_err());
    }
}
