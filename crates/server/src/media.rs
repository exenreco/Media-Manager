use super::{authenticate, error, ApiError, AppState};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;
use tv_manager_core::rename::{playback_path, RenamePlan};
use uuid::Uuid;

const LIFETIME: Duration = Duration::from_secs(900);

#[derive(Default)]
pub struct Playback {
    completed: Mutex<HashMap<Uuid, (Instant, RenamePlan)>>,
    tickets: Mutex<HashMap<Uuid, (Instant, Uuid, usize)>>,
}

impl Playback {
    pub fn remember(&self, plan: &RenamePlan) {
        if let Ok(mut plans) = self.completed.lock() {
            plans.retain(|_, (created, _)| created.elapsed() < LIFETIME);
            if plans.len() >= 32 {
                if let Some(oldest) = plans.iter().min_by_key(|(_, (t, _))| *t).map(|(id, _)| *id) {
                    plans.remove(&oldest);
                }
            }
            plans.insert(plan.id, (Instant::now(), plan.clone()));
        }
    }
}

fn find_plan(state: &AppState, id: Uuid) -> Result<RenamePlan, ApiError> {
    if let Some((created, plan)) = state
        .plans
        .lock()
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Plan store unavailable"))?
        .get(&id)
    {
        if created.elapsed() < LIFETIME {
            return Ok(plan.clone());
        }
    }
    if let Some((created, plan)) = state
        .media
        .completed
        .lock()
        .map_err(|_| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Playback store unavailable",
            )
        })?
        .get(&id)
    {
        if created.elapsed() < LIFETIME {
            return Ok(plan.clone());
        }
    }
    Err(error(
        StatusCode::NOT_FOUND,
        "Preview expired; refresh the library to play this file",
    ))
}

pub async fn ticket(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, index)): Path<(Uuid, usize)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authenticate(&state, &headers)?;
    playback_path(&find_plan(&state, id)?, index).map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
    let mut tickets = state.media.tickets.lock().map_err(|_| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Playback store unavailable",
        )
    })?;
    tickets.retain(|_, (created, _, _)| created.elapsed() < LIFETIME);
    if tickets.len() >= 64 {
        return Err(error(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many playback sessions; wait for an older link to expire",
        ));
    }
    let ticket = Uuid::new_v4();
    tickets.insert(ticket, (Instant::now(), id, index));
    Ok(Json(serde_json::json!({ "ticket": ticket })))
}

fn byte_range(value: &str, size: u64) -> Option<(u64, u64)> {
    if size == 0 {
        return None;
    }
    let value = value.strip_prefix("bytes=")?;
    if value.contains(',') {
        return None;
    }
    let (first, last) = value.split_once('-')?;
    if first.is_empty() {
        let length = last.parse::<u64>().ok()?.min(size);
        return (length > 0).then_some((size - length, size - 1));
    }
    let start = first.parse::<u64>().ok()?;
    let end = if last.is_empty() {
        size - 1
    } else {
        last.parse::<u64>().ok()?.min(size - 1)
    };
    (start <= end && start < size).then_some((start, end))
}

pub async fn stream(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(ticket): Path<Uuid>,
) -> Result<Response, ApiError> {
    let (created, id, index) = state
        .media
        .tickets
        .lock()
        .map_err(|_| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Playback store unavailable",
            )
        })?
        .get(&ticket)
        .copied()
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "Playback link not found"))?;
    if created.elapsed() >= LIFETIME {
        return Err(error(
            StatusCode::GONE,
            "Playback link expired; press Play again",
        ));
    }
    let path = playback_path(&find_plan(&state, id)?, index)
        .map_err(|e| error(StatusCode::NOT_FOUND, e))?;
    let content_type = match path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "ts" | "m2ts" => "video/mp2t",
        "mkv" => "video/x-matroska",
        _ => "video/x-msvideo",
    };
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|_| error(StatusCode::NOT_FOUND, "Media file is unavailable"))?;
    let size = file
        .metadata()
        .await
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Could not read media"))?
        .len();
    let range = headers.get(header::RANGE);
    let (start, end) = if let Some(range) = range {
        let parsed = range.to_str().ok().and_then(|s| byte_range(s, size));
        let Some(parsed) = parsed else {
            return Ok((
                StatusCode::RANGE_NOT_SATISFIABLE,
                [(header::CONTENT_RANGE, format!("bytes */{size}"))],
            )
                .into_response());
        };
        parsed
    } else {
        (0, size.saturating_sub(1))
    };
    let length = if size == 0 { 0 } else { end - start + 1 };
    file.seek(std::io::SeekFrom::Start(start))
        .await
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Could not seek media"))?;
    let mut response = Response::builder()
        .status(if range.is_some() {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        })
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, length)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::REFERRER_POLICY, "no-referrer");
    if range.is_some() {
        response = response.header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{size}"));
    }
    response
        .body(Body::from_stream(ReaderStream::new(file.take(length))))
        .map_err(|_| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Could not start media stream",
            )
        })
}

#[cfg(test)]
mod tests {
    use super::byte_range;
    #[test]
    fn validates_video_seek_ranges() {
        assert_eq!(byte_range("bytes=2-4", 10), Some((2, 4)));
        assert_eq!(byte_range("bytes=8-", 10), Some((8, 9)));
        assert_eq!(byte_range("bytes=-3", 10), Some((7, 9)));
        assert_eq!(byte_range("bytes=0-100", 10), Some((0, 9)));
        for range in [
            "bytes=10-",
            "bytes=5-2",
            "bytes=-0",
            "bytes=0-1,4-5",
            "invalid",
        ] {
            assert_eq!(byte_range(range, 10), None);
        }
    }
}
