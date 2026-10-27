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
    if request.movie
        && !matches!(
            request.source,
            Source::Omdb { .. } | Source::Tmdb { .. } | Source::Custom { .. }
        )
    {
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
        Source::Custom { base_url, key } => {
            if request.movie {
                movie_lookup(request, base_url, key, "Custom movie API").await?
            } else {
                tvmaze(request, base_url, key).await?
            }
        }
        Source::Tmdb { key } => {
            anyhow::ensure!(!key.trim().is_empty(), "Enter your TMDB API key");
            movie_lookup(request, "https://api.themoviedb.org/3", key, "TMDB").await?
        }
        Source::Omdb { key } => omdb(request, key).await?,
    };
    if let Some(season) = request.season {
        show.episodes.retain(|e| e.season == season);
        show.artwork.seasons.retain(|s, _| *s == season);
    }
    show.episodes.sort_by_key(|e| (e.season, e.number));
    if show.episodes.is_empty() && !request.movie {
        bail!("No numbered episodes found for this selection");
    }
    if let Some(url) = &request.options.poster_override {
        validate_artwork_choice(&show.artwork, url)?;
        show.artwork.poster = Some(url.clone());
    }
    for (season, url) in &request.options.season_overrides {
        validate_artwork_choice(&show.artwork, url)?;
        show.artwork.seasons.insert(*season, url.clone());
    }
    show.year = show.year.or(request.year);
    Ok(show)
}

fn tvmaze_search_url(request: &PlanRequest, base: &str, key: &str) -> Result<Url> {
    // Single search can arbitrarily choose between remakes with the same title.
    let endpoint = if request.year.is_some() {
        "search/shows"
    } else {
        "singlesearch/shows"
    };
    let mut url = Url::parse(&format!("{}/{endpoint}", base.trim_end_matches('/')))
        .context("Invalid provider URL")?;
    if url.scheme() != "https" {
        bail!("Metadata provider must use HTTPS");
    }
    url.query_pairs_mut().append_pair("q", request.name.trim());
    if !key.is_empty() {
        url.query_pairs_mut().append_pair("apikey", key);
    }
    Ok(url)
}

fn select_tvmaze_show(response: Value, year: Option<u32>) -> Result<Value> {
    let Some(year) = year else {
        return Ok(response);
    };
    response
        .as_array()
        .context("Invalid show search results")?
        .iter()
        .map(|item| &item["show"])
        .find(|show| {
            show["premiered"]
                .as_str()
                .and_then(|date| date.get(..4))
                .and_then(|year| year.parse::<u32>().ok())
                == Some(year)
        })
        .cloned()
        .with_context(|| {
            format!("No matching series found for year {year}; check the title and premiere year")
        })
}

fn omdb_search_url(request: &PlanRequest, key: &str) -> Result<Url> {
    let mut url = Url::parse_with_params(
        "https://www.omdbapi.com/",
        &[
            ("apikey", key),
            ("t", request.name.trim()),
            ("type", if request.movie { "movie" } else { "series" }),
        ],
    )?;
    if !request.options.selected_id.is_empty() {
        let id = &request.options.selected_id;
        if !id.starts_with("tt") || !id[2..].chars().all(|c| c.is_ascii_digit()) {
            bail!("Invalid IMDb ID");
        }
        url.query_pairs_mut().append_pair("i", id);
    }
    if let Some(year) = request.year {
        url.query_pairs_mut().append_pair("y", &year.to_string());
    }
    Ok(url)
}

async fn tvmaze(request: &PlanRequest, base: &str, key: &str) -> Result<Show> {
    let client = client()?;
    let base = base.trim_end_matches('/');
    if Url::parse(base)?.scheme() != "https" {
        bail!("Metadata provider must use HTTPS");
    }
    let result = if request.options.selected_id.is_empty() {
        let response = json(&client, tvmaze_search_url(request, base, key)?).await?;
        select_tvmaze_show(response, request.year)?
    } else {
        let id = request
            .options
            .selected_id
            .parse::<u64>()
            .context("Invalid TVMaze show ID")?;
        let mut url = Url::parse(&format!("{base}/shows/{id}"))?;
        if !key.is_empty() {
            url.query_pairs_mut().append_pair("apikey", key);
        }
        let result = json(&client, url).await?;
        if request.year.is_some()
            && result["premiered"]
                .as_str()
                .and_then(|s| s.get(..4))
                .and_then(|s| s.parse::<u32>().ok())
                != request.year
        {
            bail!("The selected show does not match the entered year; search again");
        }
        result
    };
    let id = result["id"].as_u64().context("Show not found")?;
    let mut url = Url::parse(&format!("{base}/shows/{id}/episodes"))?;
    if !key.is_empty() {
        url.query_pairs_mut().append_pair("apikey", key);
    }
    let episodes = json(&client, url).await?;
    let episodes: Vec<Episode> = episodes
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
    let mut artwork = tvmaze_artwork(&result, &seasons);
    let mut images_url = Url::parse(&format!("{base}/shows/{id}/images"))?;
    if !key.is_empty() {
        images_url.query_pairs_mut().append_pair("apikey", key);
    }
    if let Ok(images) = json(&client, images_url).await {
        for (index, item) in images
            .as_array()
            .into_iter()
            .flatten()
            .filter(|item| item["type"] == "poster")
            .enumerate()
        {
            if let Some(url) = poster_url(&item["resolutions"]["original"]["url"]) {
                artwork.alternatives.push(crate::ArtworkChoice {
                    label: format!("TVMaze poster {}", index + 1),
                    url,
                    season: None,
                    provider: "TVMaze".into(),
                });
            }
        }
    }
    supplement_tmdb(
        &client,
        &mut artwork,
        result["externals"]["imdb"].as_str(),
        &request.options.tmdb_key,
        false,
        &episodes,
    )
    .await;
    Ok(Show {
        artwork,
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
    let url = omdb_search_url(request, key)?;
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
    for season in seasons.into_iter().filter(|_| !request.movie) {
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
    let mut artwork = Artwork {
        poster: poster_url(&result["Poster"]),
        ..Default::default()
    };
    supplement_tmdb(
        &client,
        &mut artwork,
        Some(id),
        &request.options.tmdb_key,
        request.movie,
        &episodes,
    )
    .await;
    Ok(Show {
        artwork,
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
        alternatives: vec![],
        warnings: vec![],
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

fn validate_artwork_choice(artwork: &Artwork, url: &str) -> Result<()> {
    if artwork.poster.as_deref() == Some(url)
        || artwork.seasons.values().any(|value| value == url)
        || artwork.alternatives.iter().any(|item| item.url == url)
    {
        return Ok(());
    }
    bail!("Selected artwork is no longer available from this provider; search again")
}

pub async fn search(request: &PlanRequest) -> Result<Vec<crate::ShowMatch>> {
    if request.name.trim().is_empty() {
        bail!("Enter a title before searching");
    }
    let client = client()?;
    match &request.source {
        Source::Tmdb { key } => {
            anyhow::ensure!(!key.trim().is_empty(), "Enter your TMDB API key");
            movie_search(request, "https://api.themoviedb.org/3", key, "TMDB").await
        }
        Source::Custom { base_url, key } if request.movie => {
            movie_search(request, base_url, key, "Custom movie API").await
        }
        Source::TvMaze | Source::Custom { .. } => {
            if request.movie {
                bail!("Choose TMDB, OMDb, or a custom movie API to search movies");
            }
            let (base, key) = match &request.source {
                Source::Custom { base_url, key } => (base_url.as_str(), key.as_str()),
                _ => ("https://api.tvmaze.com", ""),
            };
            let mut query = request.clone();
            query.year = Some(0);
            let response = json(&client, tvmaze_search_url(&query, base, key)?).await?;
            Ok(response
                .as_array()
                .context("Invalid search response")?
                .iter()
                .filter_map(|item| {
                    let show = &item["show"];
                    let year = show["premiered"]
                        .as_str()
                        .and_then(|s| s.get(..4))
                        .and_then(|s| s.parse().ok());
                    if request.year.is_some() && request.year != year {
                        return None;
                    }
                    Some(crate::ShowMatch {
                        id: show["id"].as_u64()?.to_string(),
                        name: show["name"].as_str()?.into(),
                        year,
                        poster: poster_url(&show["image"]["medium"]),
                        provider: if matches!(request.source, Source::Custom { .. }) {
                            "Custom TVMaze"
                        } else {
                            "TVMaze"
                        }
                        .into(),
                        kind: show["type"].as_str().unwrap_or("Series").into(),
                    })
                })
                .take(30)
                .collect())
        }
        Source::Omdb { key } => {
            if key.trim().is_empty() {
                bail!("Enter your OMDb API key");
            }
            let mut url = Url::parse_with_params(
                "https://www.omdbapi.com/",
                &[
                    ("apikey", key.as_str()),
                    ("s", request.name.trim()),
                    ("type", if request.movie { "movie" } else { "series" }),
                ],
            )?;
            if let Some(year) = request.year {
                url.query_pairs_mut().append_pair("y", &year.to_string());
            }
            let response = json(&client, url).await?;
            Ok(response["Search"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|show| {
                    Some(crate::ShowMatch {
                        id: show["imdbID"].as_str()?.into(),
                        name: show["Title"].as_str()?.into(),
                        year: show["Year"]
                            .as_str()
                            .and_then(|s| s.get(..4))
                            .and_then(|s| s.parse().ok()),
                        poster: poster_url(&show["Poster"]),
                        provider: "OMDb".into(),
                        kind: show["Type"].as_str().unwrap_or("Series").into(),
                    })
                })
                .collect())
        }
        _ => bail!("Choose TVMaze, OMDb, or a custom API to search for a match"),
    }
}

fn movie_url(base: &str, key: &str, path: &str) -> Result<Url> {
    let mut url = Url::parse(base).context("Invalid movie provider base URL")?;
    anyhow::ensure!(
        url.scheme() == "https"
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "Movie provider must use an HTTPS base URL without credentials, query, or fragment"
    );
    url.set_path(&format!("{}/{}", url.path().trim_end_matches('/'), path));
    if !key.trim().is_empty() {
        url.query_pairs_mut().append_pair("api_key", key.trim());
    }
    Ok(url)
}
fn movie_search_url(request: &PlanRequest, base: &str, key: &str) -> Result<Url> {
    let mut url = movie_url(base, key, "search/movie")?;
    url.query_pairs_mut()
        .append_pair("query", request.name.trim())
        .append_pair("include_adult", "false");
    if let Some(year) = request.year {
        url.query_pairs_mut().append_pair("year", &year.to_string());
    }
    Ok(url)
}
fn movie_year(item: &Value) -> Option<u32> {
    item["release_date"].as_str()?.get(..4)?.parse().ok()
}
fn movie_poster(value: &Value) -> Option<String> {
    if let Some(url) = poster_url(value) {
        return Some(url);
    }
    let path = value.as_str()?;
    (path.starts_with('/')
        && !path.starts_with("//")
        && !path.contains("..")
        && !path.contains(['?', '#', '\\']))
    .then(|| format!("https://image.tmdb.org/t/p/original{path}"))
}
fn parse_movie_matches(
    response: &Value,
    year: Option<u32>,
    provider: &str,
) -> Result<Vec<crate::ShowMatch>> {
    let results = response["results"]
        .as_array()
        .context("Movie provider must return a results array")?;
    Ok(results
        .iter()
        .filter(|item| year.is_none() || movie_year(item) == year)
        .filter_map(|item| {
            Some(crate::ShowMatch {
                id: item["id"].as_u64()?.to_string(),
                name: item["title"].as_str()?.into(),
                year: movie_year(item),
                poster: movie_poster(&item["poster_path"]),
                provider: provider.into(),
                kind: "Movie".into(),
            })
        })
        .take(30)
        .collect())
}
async fn movie_search(
    request: &PlanRequest,
    base: &str,
    key: &str,
    provider: &str,
) -> Result<Vec<crate::ShowMatch>> {
    anyhow::ensure!(request.movie, "TMDB movie search requires Movies mode");
    parse_movie_matches(
        &json(&client()?, movie_search_url(request, base, key)?).await?,
        request.year,
        provider,
    )
}
fn parse_movie_details(details: &Value, request: &PlanRequest, provider: &str) -> Result<Show> {
    let name = details["title"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .context("Movie provider returned no title")?;
    let year = movie_year(details);
    anyhow::ensure!(
        request.year.is_none() || year == request.year,
        "Movie release year does not match the requested year"
    );
    let mut artwork = Artwork {
        poster: movie_poster(&details["poster_path"]),
        ..Default::default()
    };
    for (index, poster) in details["images"]["posters"]
        .as_array()
        .into_iter()
        .flatten()
        .take(40)
        .enumerate()
    {
        if let Some(url) = movie_poster(&poster["file_path"]) {
            artwork.alternatives.push(crate::ArtworkChoice {
                label: format!("{provider} poster {}", index + 1),
                url,
                season: None,
                provider: provider.into(),
            });
        }
    }
    Ok(Show {
        name: name.into(),
        year,
        episodes: Vec::new(),
        artwork,
    })
}
async fn movie_lookup(
    request: &PlanRequest,
    base: &str,
    key: &str,
    provider: &str,
) -> Result<Show> {
    anyhow::ensure!(request.movie, "TMDB movie provider requires Movies mode");
    let id = if request.options.selected_id.is_empty() {
        movie_search(request, base, key, provider)
            .await?
            .first()
            .context("No matching movie found; check the title and year")?
            .id
            .clone()
    } else {
        request.options.selected_id.clone()
    };
    let id = id.parse::<u64>().context("Invalid movie provider ID")?;
    let mut url = movie_url(base, key, &format!("movie/{id}"))?;
    url.query_pairs_mut()
        .append_pair("append_to_response", "images");
    let details = json(&client()?, url).await?;
    if let Some(returned) = details["id"].as_u64() {
        anyhow::ensure!(
            returned == id,
            "Movie provider returned a different movie ID"
        );
    }
    parse_movie_details(&details, request, provider)
}

async fn supplement_tmdb(
    client: &Client,
    artwork: &mut Artwork,
    imdb: Option<&str>,
    key: &str,
    movie: bool,
    episodes: &[Episode],
) {
    if key.trim().is_empty() {
        return;
    }
    if let Err(error) = tmdb_artwork(client, artwork, imdb, key, movie, episodes).await {
        artwork
            .warnings
            .push(format!("TMDB artwork unavailable: {error}"));
    }
}
async fn tmdb_artwork(
    client: &Client,
    artwork: &mut Artwork,
    imdb: Option<&str>,
    key: &str,
    movie: bool,
    episodes: &[Episode],
) -> Result<()> {
    let imdb = imdb.context("This provider has no IMDb ID to verify a TMDB match")?;
    if !imdb.starts_with("tt") || !imdb[2..].chars().all(|c| c.is_ascii_digit()) {
        bail!("Invalid provider IMDb ID");
    }
    let url = Url::parse_with_params(
        &format!("https://api.themoviedb.org/3/find/{imdb}"),
        &[("external_source", "imdb_id"), ("api_key", key)],
    )?;
    let matches = json(client, url).await?;
    let item = matches[if movie { "movie_results" } else { "tv_results" }]
        .as_array()
        .and_then(|items| items.first())
        .context("No verified TMDB match")?;
    let id = item["id"].as_u64().context("Invalid TMDB ID")?;
    let kind = if movie { "movie" } else { "tv" };
    let details = json(
        client,
        Url::parse_with_params(
            &format!("https://api.themoviedb.org/3/{kind}/{id}"),
            &[("api_key", key), ("append_to_response", "images")],
        )?,
    )
    .await?;
    let image_url = |value: &Value| {
        value
            .as_str()
            .filter(|s| s.starts_with('/') && !s.contains("..") && !s.contains('?'))
            .map(|s| format!("https://image.tmdb.org/t/p/original{s}"))
    };
    for (index, image) in details["images"]["posters"]
        .as_array()
        .into_iter()
        .flatten()
        .take(40)
        .enumerate()
    {
        if let Some(url) = image_url(&image["file_path"]) {
            artwork.alternatives.push(crate::ArtworkChoice {
                label: format!("TMDB poster {}", index + 1),
                url,
                season: None,
                provider: "TMDB".into(),
            });
        }
    }
    if artwork.poster.is_none() {
        artwork.poster = image_url(&details["poster_path"]);
    }
    for season in details["seasons"].as_array().into_iter().flatten() {
        let Some(number) = season["season_number"]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
        else {
            continue;
        };
        if let Some(url) = image_url(&season["poster_path"]) {
            artwork.alternatives.push(crate::ArtworkChoice {
                label: format!("TMDB season {number:02}"),
                url: url.clone(),
                season: Some(number),
                provider: "TMDB".into(),
            });
            // Different providers may use calendar years or different anime orders.
            // Only fill matching numbered seasons with the same episode count.
            let count = episodes.iter().filter(|e| e.season == number).count() as u64;
            if count > 0 && season["episode_count"].as_u64() == Some(count) {
                artwork.seasons.entry(number).or_insert(url);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod year_search_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn omitted_year_keeps_title_only_searches() {
        let request = PlanRequest {
            name: "The Office".into(),
            ..Default::default()
        };
        let url = tvmaze_search_url(&request, "https://api.tvmaze.com", "").unwrap();
        assert_eq!(url.path(), "/singlesearch/shows");
        assert!(!omdb_search_url(&request, "test")
            .unwrap()
            .query_pairs()
            .any(|(key, _)| key == "y"));
        let show = json!({"id": 1});
        assert_eq!(select_tvmaze_show(show.clone(), None).unwrap(), show);
    }

    #[test]
    fn one_piece_year_selects_anime_without_losing_available_season_artwork() {
        let results = json!([
            {"show": {"id": 46065, "name": "One Piece", "premiered": "2023-08-31"}},
            {"show": {"id": 1505, "name": "One Piece", "premiered": "1999-10-20",
                "image": {"original": "https://example.com/anime.jpg"}}}
        ]);
        let show = select_tvmaze_show(results, Some(1999)).unwrap();
        assert_eq!(show["id"], 1505);
        let artwork = tvmaze_artwork(
            &show,
            &json!([
                {"number": 1999, "image": null},
                {"number": 2024, "image": {"original": "https://example.com/season-2024.jpg"}}
            ]),
        );
        assert_eq!(
            artwork.poster.as_deref(),
            Some("https://example.com/anime.jpg")
        );
        assert!(artwork.seasons.contains_key(&2024));
        assert!(!artwork.seasons.contains_key(&1999));
    }

    #[test]
    fn supplied_year_selects_the_correct_remake_and_rejects_other_years() {
        let request = PlanRequest {
            name: "The Office".into(),
            year: Some(2005),
            ..Default::default()
        };
        let url = tvmaze_search_url(&request, "https://example.com/api/", "test").unwrap();
        assert_eq!(url.path(), "/api/search/shows");
        assert!(url
            .query_pairs()
            .any(|(key, value)| key == "q" && value == "The Office"));
        assert!(url
            .query_pairs()
            .any(|(key, value)| key == "apikey" && value == "test"));
        let results = json!([
            {"show": {"id": 1, "premiered": "2001-07-09"}},
            {"show": {"id": 2, "premiered": null}},
            {"show": {"id": 3, "premiered": "2005-03-24"}}
        ]);
        assert_eq!(
            select_tvmaze_show(results.clone(), request.year).unwrap()["id"],
            3
        );
        assert!(select_tvmaze_show(results, Some(2020)).is_err());
        assert!(omdb_search_url(&request, "test")
            .unwrap()
            .query_pairs()
            .any(|(key, value)| key == "y" && value == "2005"));
    }
}

#[cfg(test)]
mod movie_provider_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn movie_queries_include_title_and_optional_year_for_tmdb_and_custom_bases() {
        let mut request = PlanRequest {
            movie: true,
            name: "Dune & more".into(),
            year: Some(2021),
            ..Default::default()
        };
        let url = movie_search_url(&request, "https://provider.example/api/", "test-key").unwrap();
        assert_eq!(url.path(), "/api/search/movie");
        assert!(url
            .query_pairs()
            .any(|(k, v)| k == "query" && v == "Dune & more"));
        assert!(url.query_pairs().any(|(k, v)| k == "year" && v == "2021"));
        request.year = None;
        assert!(
            !movie_search_url(&request, "https://api.themoviedb.org/3", "")
                .unwrap()
                .query_pairs()
                .any(|(k, _)| k == "year")
        );
        assert!(movie_search_url(&request, "http://provider.example/api", "").is_err());
        assert!(
            movie_search_url(&request, "https://user:secret@provider.example/api", "").is_err()
        );
    }
    #[test]
    fn movie_matches_filter_remakes_and_preserve_poster_urls() {
        let response = json!({"results":[{"id":1,"title":"Dune","release_date":"1984-12-14","poster_path":"/old.jpg"},{"id":2,"title":"Dune","release_date":"2021-10-22","poster_path":"https://custom.example/new.png"}]});
        let matches = parse_movie_matches(&response, Some(2021), "Custom movie API").unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, "2");
        assert_eq!(
            matches[0].poster.as_deref(),
            Some("https://custom.example/new.png")
        );
        assert_eq!(
            parse_movie_matches(&response, None, "TMDB").unwrap().len(),
            2
        );
    }
    #[test]
    fn details_provide_movie_title_year_and_alternative_posters_without_episodes() {
        let request = PlanRequest {
            movie: true,
            year: Some(2021),
            ..Default::default()
        };
        let details = json!({"id":2,"title":"Dune","release_date":"2021-10-22","poster_path":"/main.jpg","images":{"posters":[{"file_path":"/alternate.jpg"},{"file_path":"https://custom.example/alternate.png"}]}});
        let show = parse_movie_details(&details, &request, "TMDB").unwrap();
        assert_eq!(show.name, "Dune");
        assert_eq!(show.year, Some(2021));
        assert!(show.episodes.is_empty());
        assert_eq!(show.artwork.alternatives.len(), 2);
        assert_eq!(
            show.artwork.poster.as_deref(),
            Some("https://image.tmdb.org/t/p/original/main.jpg")
        );
        assert!(parse_movie_details(
            &details,
            &PlanRequest {
                year: Some(1984),
                ..request
            },
            "TMDB"
        )
        .is_err());
        assert!(movie_poster(&json!("http://unsafe.example/image.jpg")).is_none());
        assert!(movie_poster(&json!("/../image.jpg")).is_none());
    }
}
