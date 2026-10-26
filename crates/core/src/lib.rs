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
    pub seasons: std::collections::BTreeMap<u32, String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Source {
    #[default]
    TvMaze,
    Omdb {
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
    pub source: Source,
    #[serde(default)]
    pub download_posters: bool,
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
            source: Source::TvMaze,
            download_posters: false,
        }
    }
}
