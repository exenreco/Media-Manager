use std::collections::BTreeMap;
use tv_manager_core::{rename::RenameTask, Episode};

#[derive(Default)]
pub struct SeasonGroup {
    pub season: Option<u32>,
    pub rows: Vec<(usize, RenameTask)>,
    pub missing: Vec<Episode>,
}

impl SeasonGroup {
    pub fn title(&self, movie: bool) -> String {
        if movie {
            "Movies".into()
        } else {
            self.season
                .map_or_else(|| "Unassigned files".into(), |s| format!("Season {s:02}"))
        }
    }

    pub fn file_count(&self) -> usize {
        self.rows.iter().filter(|(_, r)| !r.directory).count()
    }
}

pub fn group_by_season(rows: &[RenameTask], missing: &[Episode], movie: bool) -> Vec<SeasonGroup> {
    let mut groups = BTreeMap::<Option<u32>, SeasonGroup>::new();
    for (index, row) in rows.iter().enumerate() {
        let season = if movie { None } else { row.season };
        let group = groups.entry(season).or_default();
        group.season = season;
        group.rows.push((index, row.clone()));
    }
    for episode in missing {
        let season = if movie { None } else { Some(episode.season) };
        let group = groups.entry(season).or_default();
        group.season = season;
        group.missing.push(episode.clone());
    }
    let mut groups: Vec<_> = groups.into_values().collect();
    for group in &mut groups {
        group.missing.sort_by_key(|e| (e.season, e.number));
    }
    groups.sort_by_key(|g| (g.season.is_none(), g.season));
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(season: Option<u32>) -> RenameTask {
        // A custom naming template need not contain a season token.
        serde_json::from_value(serde_json::json!({
            "source": "random.mkv", "target": "Pilot.mkv", "status": "Ready",
            "message": "", "directory": false, "season": season,
        }))
        .unwrap()
    }

    #[test]
    fn includes_wholly_missing_seasons_and_orders_seasons_numerically() {
        let rows = vec![row(Some(10)), row(Some(2)), row(None)];
        let missing = vec![
            Episode {
                season: 1,
                number: 1,
                name: "Pilot".into(),
            },
            Episode {
                season: 2,
                number: 3,
                name: "Next".into(),
            },
        ];
        let groups = group_by_season(&rows, &missing, false);
        assert_eq!(
            groups.iter().map(|g| g.season).collect::<Vec<_>>(),
            vec![Some(1), Some(2), Some(10), None]
        );
        assert_eq!(groups[0].file_count(), 0);
        assert_eq!(groups[0].missing.len(), 1);
        assert_eq!(groups[1].file_count(), 1);
        assert_eq!(groups[1].missing[0].number, 3);
        assert_eq!(groups[1].rows[0].0, 1);
        assert_eq!(groups[3].rows[0].0, 2);
        assert_eq!(groups[3].title(false), "Unassigned files");
    }

    #[test]
    fn movie_files_have_one_section_without_season_headings() {
        let groups = group_by_season(&[row(None), row(Some(2))], &[], true);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].title(true), "Movies");
        assert_eq!(groups[0].file_count(), 2);
    }
}
