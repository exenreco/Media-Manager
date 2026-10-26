use std::fs;
use tempfile::tempdir;
use tv_manager_core::{
    health, history,
    rename::{self, TaskStatus},
    *,
};
fn show() -> Show {
    Show {
        name: "Anime".into(),
        year: Some(1999),
        episodes: vec![
            Episode {
                season: 0,
                number: 1,
                name: "Special".into(),
            },
            Episode {
                season: 1,
                number: 1,
                name: "Pilot".into(),
            },
            Episode {
                season: 1,
                number: 2,
                name: "Second".into(),
            },
            Episode {
                season: 2,
                number: 1,
                name: "Return".into(),
            },
        ],
        artwork: Default::default(),
    }
}
fn write(root: &std::path::Path, path: &str, bytes: &[u8]) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}
#[test]
fn absolute_numbers_preserve_gaps_and_specials_and_explicit_order() {
    let dir = tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    for name in [
        "Anime - 001.mkv",
        "Anime - 003.mkv",
        "Anime S00E01.mkv",
        "override.mkv",
    ] {
        write(&root, name, b"video");
    }
    let request = PlanRequest {
        season: None,
        options: LibraryOptions {
            numbering: Numbering::Absolute,
            absolute_start: 1,
            episode_map: "override.mkv|1|2".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    let files = rename::media_files(&root).unwrap();
    let plan = rename::build_plan(&root, &request, show(), &files).unwrap();
    assert!(plan.tasks.iter().any(
        |t| t.source.ends_with("Anime - 003.mkv") && t.target.ends_with("S02E01 - Return.mkv")
    ));
    assert!(plan
        .tasks
        .iter()
        .any(|t| t.source.ends_with("Anime S00E01.mkv")
            && t.target.ends_with("S00E01 - Special.mkv")));
    assert!(plan
        .tasks
        .iter()
        .any(|t| t.source.ends_with("override.mkv") && t.target.ends_with("S01E02 - Second.mkv")));
    assert!(plan.missing.is_empty());
}
#[test]
fn companion_files_follow_edits_and_undo_nested_directories_after_restart() {
    let dir = tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    write(&root, "s1/s2/01.mkv", b"video");
    write(&root, "s1/s2/01.en.forced.srt", b"subtitle");
    write(&root, "s1/s2/01.ja.mka", b"audio");
    let request = PlanRequest {
        season: Some(2),
        jellyfin: true,
        options: LibraryOptions {
            companions: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let files = rename::media_files(&root).unwrap();
    let mut plan = rename::build_plan(&root, &request, show(), &files).unwrap();
    let index = plan
        .tasks
        .iter()
        .position(|t| t.source.ends_with("01.mkv"))
        .unwrap();
    rename::edit_target(&mut plan, index, "Custom.mkv").unwrap();
    assert!(plan
        .tasks
        .iter()
        .any(|t| t.target.ends_with("Custom.en.forced.srt")));
    rename::apply(&mut plan).unwrap();
    assert!(root
        .join("Season 01/Season 02/Custom.en.forced.srt")
        .exists());
    history::finish(&plan).unwrap();
    let id = plan.id;
    drop(plan);
    let records = history::list(&root).unwrap();
    assert_eq!(records[0].remaining(), 5);
    let record = history::undo(&root, id).unwrap();
    assert_eq!(record.remaining(), 0);
    assert!(root.join("s1/s2/01.mkv").exists());
    assert!(root.join("s1/s2/01.en.forced.srt").exists());
    assert_eq!(history::undo(&root, id).unwrap().remaining(), 0);
}
#[test]
fn undo_preserves_conflicting_files_and_refuses_changed_content() {
    let dir = tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    write(&root, "01.mkv", b"video");
    let files = rename::media_files(&root).unwrap();
    let mut plan = rename::build_plan(&root, &PlanRequest::default(), show(), &files).unwrap();
    rename::apply(&mut plan).unwrap();
    write(&root, "01.mkv", b"new file");
    assert!(history::undo(&root, plan.id).is_err());
    assert_eq!(fs::read(root.join("01.mkv")).unwrap(), b"new file");
    fs::remove_file(root.join("01.mkv")).unwrap();
    write(&root, "S01E01 - Pilot.mkv", b"changed video");
    assert!(history::undo(&root, plan.id).is_err());
    assert!(!root.join("01.mkv").exists());
}
#[test]
fn health_reports_only_content_verified_duplicates() {
    let dir = tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    write(&root, "01.mkv", b"same");
    write(&root, "02.mkv", b"same");
    write(&root, "03.mkv", b"diff");
    let files = rename::media_files(&root).unwrap();
    let plan = rename::build_plan(&root, &PlanRequest::default(), show(), &files).unwrap();
    let report = health::scan(&plan).unwrap();
    assert_eq!(report.files, 3);
    assert_eq!(report.duplicates.len(), 1);
    assert_eq!(report.duplicates[0].len(), 2);
    assert_eq!(report.unmatched.len(), 1);
}
#[test]
fn companion_collision_blocks_the_whole_batch() {
    let dir = tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    write(&root, "01.mkv", b"video");
    write(&root, "01.en.srt", b"subtitle");
    write(&root, "S01E01 - Pilot.en.srt", b"existing");
    let request = PlanRequest {
        options: LibraryOptions {
            companions: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let files = rename::media_files(&root).unwrap();
    let mut plan = rename::build_plan(&root, &request, show(), &files).unwrap();
    assert!(plan.tasks.iter().any(|t| t.status == TaskStatus::Conflict));
    assert!(rename::apply(&mut plan).is_err());
    assert!(root.join("01.mkv").exists());
}

#[test]
fn undo_recovers_an_interrupted_journal_update_and_ignores_an_unexecuted_intent() {
    let dir = tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    write(&root, "01.mkv", b"video");
    let files = rename::media_files(&root).unwrap();
    let mut plan = rename::build_plan(&root, &PlanRequest::default(), show(), &files).unwrap();
    rename::apply(&mut plan).unwrap();
    let mut record = history::list(&root).unwrap().remove(0);
    record.steps[0].applied = false;
    history::store(&root, &record).unwrap();
    assert_eq!(history::undo(&root, plan.id).unwrap().remaining(), 0);
    assert!(root.join("01.mkv").exists());
    // Simulate a crash after the inverse move but before updating its journal.
    record.steps[0].applied = true;
    history::store(&root, &record).unwrap();
    assert_eq!(history::undo(&root, plan.id).unwrap().remaining(), 0);
    // A failed forward rename can leave a harmless pending intent.
    record.steps[0].applied = false;
    record.steps[0].undone = false;
    history::store(&root, &record).unwrap();
    assert_eq!(history::undo(&root, plan.id).unwrap().remaining(), 0);
}
