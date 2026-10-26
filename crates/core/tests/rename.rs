use std::{fs, path::Path};
use tempfile::tempdir;
use tv_manager_core::{
    metadata::parse_guide,
    rename::{self, TaskStatus},
    Episode, PlanRequest, Show, Source,
};

fn show() -> Show {
    Show {
        name: "Example".into(),
        year: Some(2020),
        artwork: Default::default(),
        episodes: vec![
            Episode {
                season: 1,
                number: 1,
                name: "Pilot".into(),
            },
            Episode {
                season: 1,
                number: 3,
                name: "A: New / Start?".into(),
            },
            Episode {
                season: 2,
                number: 1,
                name: "Return".into(),
            },
        ],
    }
}
fn write(path: &Path, content: &str) {
    fs::write(path, content).unwrap();
}

#[test]
fn edited_filenames_are_preview_only_and_collisions_can_be_resolved() {
    let directory = tempdir().unwrap();
    write(&directory.path().join("1.mp4"), "one");
    write(&directory.path().join("2.mp4"), "two");
    let files = rename::media_files(directory.path()).unwrap();
    let mut plan =
        rename::build_plan(directory.path(), &PlanRequest::default(), show(), &files).unwrap();
    rename::edit_target(&mut plan, 0, "Custom Pilot.mp4").unwrap();
    assert!(files[0].exists());
    assert!(!directory.path().join("Custom Pilot.mp4").exists());
    rename::edit_target(&mut plan, 1, "Custom Pilot.mp4").unwrap();
    assert!(plan.tasks.iter().all(|t| t.status == TaskStatus::Conflict));
    rename::edit_target(&mut plan, 1, "Next.mp4").unwrap();
    assert!(plan.tasks.iter().all(|t| t.status == TaskStatus::Ready));
    for invalid in [
        "../escape.mp4",
        "folder\\escape.mp4",
        "bad?.mp4",
        "CON.mp4",
        "movie.exe",
    ] {
        assert!(rename::edit_target(&mut plan, 0, invalid).is_err());
    }
    rename::apply(&mut plan).unwrap();
    assert_eq!(
        fs::read_to_string(directory.path().join("Custom Pilot.mp4")).unwrap(),
        "one"
    );
    assert!(rename::edit_target(&mut plan, 0, "Again.mp4").is_err());
}

#[test]
fn playback_paths_follow_applied_file_and_season_folder_renames() {
    let directory = tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    fs::create_dir(root.join("s1")).unwrap();
    write(&root.join("s1/1.mp4"), "video");
    write(&root.join("notes.txt"), "notes");
    let request = PlanRequest {
        season: None,
        plex: true,
        ..Default::default()
    };
    let files = rename::media_files(&root).unwrap();
    let mut plan = rename::build_plan(&root, &request, show(), &files).unwrap();
    let video = plan
        .tasks
        .iter()
        .position(|t| t.source.ends_with("1.mp4"))
        .unwrap();
    let text = plan
        .tasks
        .iter()
        .position(|t| t.source.ends_with("notes.txt"))
        .unwrap();
    assert!(rename::playback_path(&plan, video)
        .unwrap()
        .ends_with("s1/1.mp4"));
    assert!(rename::playback_path(&plan, text).is_err());
    rename::apply(&mut plan).unwrap();
    assert!(rename::playback_path(&plan, video)
        .unwrap()
        .ends_with("Season 01/Example (2020) - S01E01 - Pilot.mp4"));
}

#[test]
fn natural_order_preserves_episode_number_gaps_and_extensions() {
    let dir = tempdir().unwrap();
    write(&dir.path().join("10.MKV"), "ten");
    write(&dir.path().join("2.mp4"), "two");
    let files = rename::media_files(dir.path()).unwrap();
    let plan = rename::build_plan(dir.path(), &PlanRequest::default(), show(), &files).unwrap();
    assert!(plan.tasks[0].source.ends_with("2.mp4"));
    assert_eq!(plan.tasks[0].season, Some(1));
    assert!(plan.tasks[0].target.ends_with("S01E01 - Pilot.mp4"));
    assert!(plan.tasks[1].target.ends_with("S01E03 - A New  Start.MKV"));
    assert_eq!(plan.missing, vec![show().episodes[2].clone()]);
}

#[test]
fn portable_sanitization_handles_reserved_names_and_traversal() {
    assert_eq!(rename::sanitize(" ../A:B?*. "), "..AB");
    assert_eq!(rename::sanitize("CON.txt"), "_CON.txt");
    assert_eq!(rename::sanitize(".."), "Untitled");
    assert_eq!(rename::sanitize("\0\n"), "Untitled");
}

#[test]
fn rejects_paths_outside_root() {
    let dir = tempdir().unwrap();
    assert!(rename::scoped_folder(dir.path(), "..").is_err());
    assert!(rename::scoped_folder(dir.path(), &std::env::temp_dir().to_string_lossy()).is_err());
    assert!(rename::scoped_folder(dir.path(), "").is_ok());
}

#[test]
fn never_overwrites_a_destination_created_after_preview() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("1.mkv");
    write(&source, "original");
    let mut plan = rename::build_plan(
        dir.path(),
        &PlanRequest::default(),
        show(),
        std::slice::from_ref(&source),
    )
    .unwrap();
    let target = plan.tasks[0].target.clone();
    write(&target, "unrelated");
    assert!(rename::apply(&mut plan).is_err());
    assert_eq!(fs::read_to_string(source).unwrap(), "original");
    assert_eq!(fs::read_to_string(target).unwrap(), "unrelated");
}

#[test]
fn refuses_sources_modified_since_preview() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("1.mkv");
    write(&source, "old");
    let mut plan = rename::build_plan(
        dir.path(),
        &PlanRequest::default(),
        show(),
        std::slice::from_ref(&source),
    )
    .unwrap();
    write(&source, "replacement with a different size");
    assert!(rename::apply(&mut plan).is_err());
    assert!(!plan.tasks[0].target.exists());
}

#[test]
fn duplicate_movie_targets_block_the_entire_batch() {
    let dir = tempdir().unwrap();
    write(&dir.path().join("1.mkv"), "one");
    write(&dir.path().join("2.mkv"), "two");
    let request = PlanRequest {
        movie: true,
        ..Default::default()
    };
    let files = rename::media_files(dir.path()).unwrap();
    let mut plan = rename::build_plan(dir.path(), &request, show(), &files).unwrap();
    assert!(plan.tasks.iter().all(|t| t.status == TaskStatus::Conflict));
    assert!(rename::apply(&mut plan).is_err());
    assert!(files.iter().all(|p| p.exists()));
}

#[test]
fn all_seasons_and_nested_plex_folders_are_applied_in_correct_order() {
    let dir = tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    fs::create_dir(root.join("s1")).unwrap();
    fs::create_dir(root.join("s1/s2")).unwrap();
    write(&root.join("s1/1.mkv"), "one");
    write(&root.join("s1/s2/1.mkv"), "two");
    write(&root.join("unknown.mkv"), "unknown");
    let request = PlanRequest {
        season: None,
        plex: true,
        ..Default::default()
    };
    let files = rename::media_files(&root).unwrap();
    let mut plan = rename::build_plan(&root, &request, show(), &files).unwrap();
    assert_eq!(plan.tasks.iter().filter(|t| t.season == Some(1)).count(), 2);
    assert_eq!(plan.tasks.iter().filter(|t| t.season == Some(2)).count(), 2);
    assert_eq!(plan.tasks.iter().filter(|t| t.season.is_none()).count(), 1);
    let serialized = serde_json::to_string(&plan).unwrap();
    let remote_plan: rename::RenamePlan = serde_json::from_str(&serialized).unwrap();
    assert_eq!(
        remote_plan
            .tasks
            .iter()
            .map(|t| t.season)
            .collect::<Vec<_>>(),
        plan.tasks.iter().map(|t| t.season).collect::<Vec<_>>()
    );
    rename::apply(&mut plan).unwrap();
    assert!(
        plan.tasks.iter().all(|t| matches!(
            t.status,
            TaskStatus::Applied | TaskStatus::Skipped | TaskStatus::Unchanged
        )),
        "{plan:#?}"
    );
    assert!(root
        .join("Season 01/Example (2020) - S01E01 - Pilot.mkv")
        .exists());
    assert!(root
        .join("Season 01/Season 02/Example (2020) - S02E01 - Return.mkv")
        .exists());
    assert!(root.join("unknown.mkv").exists());
    assert_eq!(
        plan.tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Applied)
            .count(),
        4
    );
}

#[test]
fn template_tokens_are_sanitized_and_unknown_tokens_are_rejected() {
    let mut request = PlanRequest {
        source: Source::Batch {
            template: "../{Series Name} - {Season:02d}x{Episode:02d}".into(),
        },
        ..Default::default()
    };
    assert_eq!(
        rename::episode_name(&request, &show(), &show().episodes[1]).unwrap(),
        "..Example - 01x03"
    );
    request.source = Source::Batch {
        template: "{Unknown}".into(),
    };
    assert!(rename::episode_name(&request, &show(), &show().episodes[0]).is_err());
}

#[test]
fn guide_parsing_preserves_seasons_and_decodes_entities() {
    let episodes = parse_guide(
        r#"<h3>Season 1</h3><table><tr><td class="summary">A &amp; B</td></tr></table><h2>Season 2</h2><table><tr><td class="summary">Return</td></tr></table>"#,
        9,
    );
    assert_eq!(episodes.len(), 2);
    assert_eq!(
        episodes[0],
        Episode {
            season: 1,
            number: 1,
            name: "A & B".into()
        }
    );
    assert_eq!(episodes[1].season, 2);
    assert_eq!(episodes[1].number, 1);
}

#[test]
fn generic_guide_tables_work_without_season_headings() {
    let episodes = parse_guide(
        "<table><tr><td>1</td><td>Pilot</td></tr><tr><td>2</td><td>Next</td></tr></table>",
        4,
    );
    assert_eq!(episodes.len(), 2);
    assert_eq!(episodes[1].season, 4);
}

#[test]
fn an_already_correct_file_is_unchanged() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("S01E01 - Pilot.mkv");
    write(&source, "original");
    let mut plan = rename::build_plan(
        dir.path(),
        &PlanRequest::default(),
        show(),
        std::slice::from_ref(&source),
    )
    .unwrap();
    assert_eq!(plan.tasks[0].status, TaskStatus::Unchanged);
    rename::apply(&mut plan).unwrap();
    assert!(source.exists());
}

#[test]
fn naming_formats_preserve_none_and_plex_and_support_jellyfin() {
    let show = show();
    let mut request = PlanRequest::default();
    assert_eq!(
        rename::episode_name(&request, &show, &show.episodes[0]).unwrap(),
        "S01E01 - Pilot"
    );
    request.plex = true;
    assert_eq!(
        rename::episode_name(&request, &show, &show.episodes[0]).unwrap(),
        "Example (2020) - S01E01 - Pilot"
    );
    // Requests saved before the dropdown was added retain their Plex setting.
    let mut legacy = serde_json::to_value(&request).unwrap();
    legacy.as_object_mut().unwrap().remove("jellyfin");
    let restored: PlanRequest = serde_json::from_value(legacy).unwrap();
    assert!(restored.plex);
    assert!(!restored.jellyfin);
    request.plex = false;
    request.jellyfin = true;
    assert_eq!(
        rename::episode_name(&request, &show, &show.episodes[0]).unwrap(),
        "Example (2020) S01E01 - Pilot"
    );
    let mut no_year = show.clone();
    no_year.year = None;
    assert_eq!(
        rename::episode_name(&request, &no_year, &show.episodes[0]).unwrap(),
        "Example S01E01 - Pilot"
    );
    let directory = tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    fs::create_dir(root.join("s1")).unwrap();
    write(&root.join("s1/1.mkv"), "video");
    let files = rename::media_files(&root).unwrap();
    let mut plan = rename::build_plan(&root, &request, show, &files).unwrap();
    rename::apply(&mut plan).unwrap();
    assert!(root
        .join("Season 01/Example (2020) S01E01 - Pilot.mkv")
        .exists());
}
