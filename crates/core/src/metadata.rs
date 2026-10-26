use crate::{Artwork, Episode, PlanRequest, Show, Source};
use anyhow::{bail, Context, Result};
use regex::Regex;
use reqwest::{Client, Url};
use scraper::{Html, Selector};
use serde_json::Value;
use std::{collections::BTreeMap, time::Duration};

fn client() -> Result<Client> {
    Ok(Client::builder().timeout(Duration::from_secs(25)).build()?)
}

async fn json(client: &Client, url: Url) -> Result<Value> {
    // Do not forward URLs or response bodies to the UI: they may contain API keys.
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|_| anyhow::anyhow!("Could not reach metadata provider"))?;
    if !response.status().is_success() {
        bail!("Metadata provider returned HTTP {}", response.status());
    }
    response
        .json()
        .await
        .map_err(|_| anyhow::anyhow!("Unexpected metadata response"))
}

pub async fn lookup(request: &PlanRequest, file_count: usize) -> Result<Show> {
    if request.name.trim().is_empty() {
        bail!("Enter a show or movie name");
    }
    if request.movie {
        return Ok(Show {
            name: request.name.trim().into(),
            year: request.year,
            episodes: vec![],
            artwork: Artwork::default(),
        });
    }
    let season = request.season.unwrap_or(1);
    let mut show = match &request.source {
        Source::Manual { titles } => {
            if request.season.is_none() {
                bail!("Choose one season for manual titles");
            }
            Show {
                name: request.name.clone(),
                year: request.year,
                artwork: Artwork::default(),
                episodes: titles
                    .lines()
                    .filter(|s| !s.trim().is_empty())
                    .enumerate()
                    .map(|(i, s)| Episode {
                        season,
                        number: i as u32 + 1,
                        name: s.trim().to_string(),
                    })
                    .collect(),
            }
        }
        Source::Batch { .. } => {
            if request.season.is_none() {
                bail!("Choose one season for batch numbering");
            }
            Show {
                name: request.name.clone(),
                year: request.year,
                artwork: Artwork::default(),
                episodes: (1..=file_count as u32)
                    .map(|number| Episode {
                        season,
                        number,
                        name: String::new(),
                    })
                    .collect(),
            }
        }
        Source::Web { url } => scrape(url, &request.name, season).await?,
        Source::TvMaze => tvmaze(request, "https://api.tvmaze.com", "").await?,
        Source::Custom { base_url, key } => tvmaze(request, base_url, key).await?,
        Source::Omdb { key } => omdb(request, key).await?,
    };
    if let Some(season) = request.season {
        show.episodes.retain(|e| e.season == season);
        show.artwork.seasons.retain(|s, _| *s == season);
    }
    show.episodes.sort_by_key(|e| (e.season, e.number));
    if show.episodes.is_empty() {
        bail!("No numbered episodes found for this selection");
    }
    show.year = show.year.or(request.year);
    Ok(show)
}

async fn tvmaze(request: &PlanRequest, base: &str, key: &str) -> Result<Show> {
    let client = client()?;
    let base = base.trim_end_matches('/');
    let mut url =
        Url::parse(&format!("{base}/singlesearch/shows")).context("Invalid provider URL")?;
    if url.scheme() != "https" {
        bail!("Metadata provider must use HTTPS");
    }
    url.query_pairs_mut().append_pair("q", &request.name);
    if !key.is_empty() {
        url.query_pairs_mut().append_pair("apikey", key);
    }
    let result = json(&client, url).await?;
    let id = result["id"].as_u64().context("Show not found")?;
    let mut url = Url::parse(&format!("{base}/shows/{id}/episodes"))?;
    if !key.is_empty() {
        url.query_pairs_mut().append_pair("apikey", key);
    }
    let episodes = json(&client, url).await?;
    let episodes = episodes
        .as_array()
        .context("Invalid episode list")?
        .iter()
        .filter_map(|v| {
            Some(Episode {
                season: u32::try_from(v["season"].as_u64()?).ok()?,
                number: u32::try_from(v["number"].as_u64()?).ok()?,
                name: v["name"].as_str()?.into(),
            })
        })
        .collect();
    let mut seasons_url = Url::parse(&format!("{base}/shows/{id}/seasons"))?;
    if !key.is_empty() {
        seasons_url.query_pairs_mut().append_pair("apikey", key);
    }
    // Artwork is optional; an older mirror can still provide episode metadata.
    let seasons = json(&client, seasons_url).await.unwrap_or_default();
    Ok(Show {
        artwork: tvmaze_artwork(&result, &seasons),
        name: result["name"].as_str().unwrap_or(&request.name).into(),
        year: result["premiered"]
            .as_str()
            .and_then(|s| s.get(..4))
            .and_then(|s| s.parse().ok()),
        episodes,
    })
}

async fn omdb(request: &PlanRequest, key: &str) -> Result<Show> {
    if key.trim().is_empty() {
        bail!("Enter your OMDb API key");
    }
    let client = client()?;
    let url = Url::parse_with_params(
        "https://www.omdbapi.com/",
        &[
            ("apikey", key),
            ("t", request.name.as_str()),
            ("type", "series"),
        ],
    )?;
    let result = json(&client, url).await?;
    if result["Response"] == "False" {
        bail!("OMDb could not find the series; check the name and API key");
    }
    let id = result["imdbID"]
        .as_str()
        .context("Missing OMDb series ID")?;
    let count: u32 = result["totalSeasons"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let seasons = request
        .season
        .map_or_else(|| (1..=count.min(200)).collect::<Vec<_>>(), |s| vec![s]);
    let mut episodes = vec![];
    for season in seasons {
        let number = season.to_string();
        let url = Url::parse_with_params(
            "https://www.omdbapi.com/",
            &[("apikey", key), ("i", id), ("Season", number.as_str())],
        )?;
        let response = json(&client, url).await?;
        if let Some(items) = response["Episodes"].as_array() {
            episodes.extend(items.iter().filter_map(|v| {
                Some(Episode {
                    season,
                    number: v["Episode"].as_str()?.parse().ok()?,
                    name: v["Title"].as_str()?.into(),
                })
            }));
        }
    }
    Ok(Show {
        artwork: Artwork {
            poster: poster_url(&result["Poster"]),
            ..Default::default()
        },
        name: result["Title"].as_str().unwrap_or(&request.name).into(),
        year: result["Year"]
            .as_str()
            .and_then(|s| s.get(..4))
            .and_then(|s| s.parse().ok()),
        episodes,
    })
}

async fn scrape(url: &str, name: &str, season: u32) -> Result<Show> {
    let url = Url::parse(url).context("Invalid episode guide URL")?;
    if url.scheme() != "https" {
        bail!("Episode guide must use HTTPS");
    }
    let response = client()?
        .get(url)
        .send()
        .await
        .map_err(|_| anyhow::anyhow!("Could not fetch episode guide"))?;
    if !response.status().is_success() {
        bail!("Episode guide returned HTTP {}", response.status());
    }
    let html = response
        .text()
        .await
        .context("Could not read episode guide")?;
    Ok(Show {
        name: name.into(),
        year: None,
        episodes: parse_guide(&html, season),
        artwork: Artwork::default(),
    })
}

pub fn poster_url(value: &Value) -> Option<String> {
    let url = Url::parse(value.as_str()?).ok()?;
    (url.scheme() == "https" && url.username().is_empty() && url.password().is_none())
        .then(|| url.to_string())
}

pub fn tvmaze_artwork(show: &Value, seasons: &Value) -> Artwork {
    fn image(v: &Value) -> Option<String> {
        poster_url(&v["image"]["original"]).or_else(|| poster_url(&v["image"]["medium"]))
    }
    Artwork {
        poster: image(show),
        seasons: seasons
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| Some((u32::try_from(v["number"].as_u64()?).ok()?, image(v)?)))
            .collect(),
    }
}

pub fn parse_guide(html: &str, fallback_season: u32) -> Vec<Episode> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("h1,h2,h3,h4,h5,h6,td.summary").unwrap();
    let heading = Regex::new(r"(?i)season\s*(\d{1,3})").unwrap();
    let mut season = fallback_season;
    let mut counts = BTreeMap::<u32, u32>::new();
    let mut episodes = vec![];
    for element in document.select(&selector) {
        let text = element
            .text()
            .collect::<Vec<_>>()
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if element.value().name() != "td" {
            if let Some(c) = heading.captures(&text) {
                season = c[1].parse().unwrap_or(fallback_season);
            }
        } else if !text.is_empty() {
            let number = counts.entry(season).or_default();
            *number += 1;
            episodes.push(Episode {
                season,
                number: *number,
                name: text.trim_matches('"').into(),
            });
        }
    }
    if episodes.is_empty() {
        let rows = Selector::parse("tr").unwrap();
        let cells = Selector::parse("td").unwrap();
        for row in document.select(&rows) {
            let texts: Vec<String> = row
                .select(&cells)
                .map(|c| c.text().collect::<String>().trim().into())
                .collect();
            if texts.len() >= 2 && texts[0].parse::<u32>().is_ok() && !texts[1].is_empty() {
                episodes.push(Episode {
                    season: fallback_season,
                    number: episodes.len() as u32 + 1,
                    name: texts[1].clone(),
                });
            }
        }
    }
    episodes
}
