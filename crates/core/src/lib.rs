pub mod artwork;
pub mod health;
pub mod history;
pub mod metadata;
mod move_file;
pub mod posters;
pub mod rename;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Episode {
    pub season: u32,
    pub number: u32,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Show {
    pub name: String,
    pub year: Option<u32>,
    pub episodes: Vec<Episode>,
    #[serde(default)]
    pub artwork: Artwork,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Artwork {
    pub poster: Option<String>,
    #[serde(default)]
    pub alternatives: Vec<ArtworkChoice>,
    #[serde(default)]
    pub warnings: Vec<String>,
    pub seasons: std::collections::BTreeMap<u32, String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Source {
    #[default]
    TvMaze,
    Omdb {
        key: String,
    },
    Tmdb {
        key: String,
    },
    Custom {
        base_url: String,
        key: String,
    },
    Manual {
        titles: String,
    },
    Batch {
        template: String,
    },
    Web {
        url: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanRequest {
    /// Relative to the folder selected locally or the root configured on the server.
    pub folder: String,
    pub name: String,
    pub year: Option<u32>,
    /// None means all seasons, inferred from ancestor directory names.
    pub season: Option<u32>,
    pub movie: bool,
    pub plex: bool,
    #[serde(default)]
    pub jellyfin: bool,
    pub source: Source,
    #[serde(default)]
    pub download_posters: bool,
    #[serde(default)]
    pub options: LibraryOptions,
}

impl Default for PlanRequest {
    fn default() -> Self {
        Self {
            folder: String::new(),
            name: String::new(),
            year: None,
            season: Some(1),
            movie: false,
            plex: false,
            jellyfin: false,
            source: Source::TvMaze,
            download_posters: false,
            options: LibraryOptions::default(),
        }
    }
}

/// Additional options default safely for older app/server requests.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LibraryOptions {
    pub selected_id: String,
    pub numbering: Numbering,
    /// The absolute number assigned to the first regular episode in provider order.
    pub absolute_start: u32,
    /// Explicit relative-path -> season/episode overrides, one per line: path|season|episode.
    pub episode_map: String,
    pub companions: bool,
    pub tmdb_key: String,
    pub poster_override: Option<String>,
    pub season_overrides: std::collections::BTreeMap<u32, String>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Numbering {
    #[default]
    Sequence,
    Detected,
    Absolute,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtworkChoice {
    pub label: String,
    pub url: String,
    pub season: Option<u32>,
    pub provider: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShowMatch {
    pub id: String,
    pub name: String,
    pub year: Option<u32>,
    pub poster: Option<String>,
    pub provider: String,
    pub kind: String,
}
