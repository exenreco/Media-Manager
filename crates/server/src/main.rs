mod media;

use axum::{
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, patch, post},
    Json, Router,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tv_manager_core::{
    rename::{self, RenamePlan},
    PlanRequest,
};
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    root: PathBuf,
    token: Arc<String>,
    plans: Arc<Mutex<HashMap<Uuid, (Instant, RenamePlan)>>>,
    gate: Arc<tokio::sync::Semaphore>,
    media: Arc<media::Playback>,
}

type ApiError = (StatusCode, Json<serde_json::Value>);
fn error(status: StatusCode, message: impl ToString) -> ApiError {
    (
        status,
        Json(serde_json::json!({ "error": message.to_string() })),
    )
}
fn authenticate(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .unwrap_or("");
    if !constant_time_eq::constant_time_eq(token.as_bytes(), state.token.as_bytes()) {
        return Err(error(StatusCode::UNAUTHORIZED, "Invalid bearer token"));
    }
    Ok(())
}

async fn health(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    authenticate(&state, &headers)?;
    Ok(Json(
        serde_json::json!({ "service": "tv-manager", "version": env!("CARGO_PKG_VERSION") }),
    ))
}

async fn preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PlanRequest>,
) -> Result<Json<RenamePlan>, ApiError> {
    authenticate(&state, &headers)?;
    let _permit = state.gate.try_acquire().map_err(|_| {
        error(
            StatusCode::TOO_MANY_REQUESTS,
            "Another file operation is running",
        )
    })?;
    let plan = rename::preview(&state.root, &request)
        .await
        .map_err(|e| error(StatusCode::BAD_REQUEST, format!("{e:#}")))?;
    let mut plans = state
        .plans
        .lock()
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Plan store unavailable"))?;
    plans.retain(|_, (created, _)| created.elapsed() < Duration::from_secs(900));
    if plans.len() >= 32 {
        return Err(error(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many pending previews; apply a plan or wait 15 minutes",
        ));
    }
    plans.insert(plan.id, (Instant::now(), plan.clone()));
    Ok(Json(plan))
}

async fn apply(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<RenamePlan>, ApiError> {
    authenticate(&state, &headers)?;
    let permit = state.gate.clone().try_acquire_owned().map_err(|_| {
        error(
            StatusCode::TOO_MANY_REQUESTS,
            "Another file operation is running",
        )
    })?;
    let (created, mut plan) = state
        .plans
        .lock()
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Plan store unavailable"))?
        .remove(&id)
        .ok_or_else(|| {
            error(
                StatusCode::NOT_FOUND,
                "Preview expired or already applied; build a new preview",
            )
        })?;
    if created.elapsed() >= Duration::from_secs(900) {
        return Err(error(
            StatusCode::GONE,
            "Preview expired; build a new preview",
        ));
    }
    // Own the complete operation independently of the HTTP request. A phone can
    // disconnect or be suspended without cancelling poster work or releasing
    // the filesystem gate while the rename is still running.
    let media = state.media.clone();
    tokio::spawn(async move {
        let _permit = permit;
        let mut plan = tokio::task::spawn_blocking(move || {
            rename::apply(&mut plan).map_err(|e| error(StatusCode::CONFLICT, format!("{e:#}")))?;
            Ok(plan)
        })
        .await
        .map_err(|_| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "File operation interrupted; inspect folder before retrying",
            )
        })??;
        tv_manager_core::posters::download(&mut plan).await;
        media.remember(&plan);
        Ok(Json(plan))
    })
    .await
    .map_err(|_| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "File operation interrupted; inspect folder before retrying",
        )
    })?
}

async fn edit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, index)): Path<(Uuid, usize)>,
    Json(edit): Json<rename::FilenameEdit>,
) -> Result<Json<RenamePlan>, ApiError> {
    authenticate(&state, &headers)?;
    let _permit = state.gate.try_acquire().map_err(|_| {
        error(
            StatusCode::TOO_MANY_REQUESTS,
            "Another file operation is running",
        )
    })?;
    let mut plans = state
        .plans
        .lock()
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Plan store unavailable"))?;
    let (created, plan) = plans.get_mut(&id).ok_or_else(|| {
        error(
            StatusCode::NOT_FOUND,
            "Build a fresh preview before editing",
        )
    })?;
    if created.elapsed() >= Duration::from_secs(900) {
        return Err(error(
            StatusCode::GONE,
            "Preview expired; build a fresh preview",
        ));
    }
    rename::edit_target(plan, index, &edit.filename)
        .map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
    Ok(Json(plan.clone()))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let root = std::env::var("TV_MANAGER_ROOT")
        .map(PathBuf::from)
        .map_err(|_| anyhow::anyhow!("Set TV_MANAGER_ROOT to the folder to expose"))?
        .canonicalize()?;
    anyhow::ensure!(root.is_dir(), "TV_MANAGER_ROOT must be a directory");
    let token = std::env::var("TV_MANAGER_TOKEN").unwrap_or_default();
    anyhow::ensure!(
        token.len() >= 32,
        "TV_MANAGER_TOKEN must contain at least 32 characters"
    );
    let bind = std::env::var("TV_MANAGER_BIND").unwrap_or_else(|_| "127.0.0.1:8787".into());
    let state = AppState {
        root,
        token: Arc::new(token),
        plans: Arc::new(Mutex::new(HashMap::new())),
        gate: Arc::new(tokio::sync::Semaphore::new(1)),
        media: Arc::new(media::Playback::default()),
    };
    let app = router(state);
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    println!("Media Manager listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}

fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/plans", post(preview))
        .route("/v1/plans/{id}/apply", post(apply))
        .route("/v1/plans/{id}/tasks/{index}", patch(edit))
        .route("/v1/plans/{id}/tasks/{index}/play", post(media::ticket))
        .route("/v1/media/{ticket}", get(media::stream))
        .layer(DefaultBodyLimit::max(256 * 1024))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use tower::ServiceExt;
    use tv_manager_core::Source;

    const TOKEN: &str = "test-token-with-at-least-32-characters";

    #[tokio::test]
    #[ignore = "Uses the live TVMaze API and image CDN; run explicitly with network access"]
    async fn live_tvmaze_poster_preview_and_download() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("Season 01")).unwrap();
        std::fs::write(root.path().join("Season 01/1.mkv"), b"test media").unwrap();
        let app = app(root.path().into());
        let payload = PlanRequest {
            name: "Breaking Bad".into(),
            season: Some(1),
            download_posters: true,
            ..Default::default()
        };
        let response = app
            .clone()
            .oneshot(request(
                "POST",
                "/v1/plans",
                serde_json::to_string(&payload).unwrap(),
            ))
            .await
            .unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 1_000_000).await.unwrap();
        assert_eq!(
            status,
            StatusCode::OK,
            "{}",
            String::from_utf8_lossy(&bytes)
        );
        let plan: RenamePlan = serde_json::from_slice(&bytes).unwrap();
        assert!(plan.show.artwork.poster.is_some());
        assert!(!plan.posters.is_empty());
        assert!(
            !root.path().join("poster.jpg").exists(),
            "Preview must not save images"
        );
        let response = app
            .oneshot(request(
                "POST",
                &format!("/v1/plans/{}/apply", plan.id),
                String::new(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let plan: RenamePlan =
            serde_json::from_slice(&to_bytes(response.into_body(), 1_000_000).await.unwrap())
                .unwrap();
        assert!(
            plan.posters
                .iter()
                .all(|p| p.status == rename::TaskStatus::Applied),
            "{:#?}",
            plan.posters
        );
        assert!(
            std::fs::metadata(root.path().join("poster.jpg"))
                .unwrap()
                .len()
                > 100
        );
    }
    fn app(root: PathBuf) -> Router {
        router(AppState {
            root,
            token: Arc::new(TOKEN.into()),
            plans: Arc::new(Mutex::new(HashMap::new())),
            gate: Arc::new(tokio::sync::Semaphore::new(1)),
            media: Arc::new(media::Playback::default()),
        })
    }
    fn request(method: &str, uri: &str, body: String) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {TOKEN}"))
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap()
    }

    #[tokio::test]
    async fn control_endpoints_require_authentication() {
        let root = tempfile::tempdir().unwrap();
        let app = app(root.path().into());
        for (method, path, body) in [
            ("GET", "/v1/health", String::new()),
            (
                "PATCH",
                "/v1/plans/00000000-0000-0000-0000-000000000000/tasks/0",
                "{\"filename\":\"Pilot.mp4\"}".into(),
            ),
            (
                "POST",
                "/v1/plans/00000000-0000-0000-0000-000000000000/tasks/0/play",
                String::new(),
            ),
            (
                "POST",
                "/v1/plans",
                serde_json::to_string(&PlanRequest::default()).unwrap(),
            ),
            (
                "POST",
                "/v1/plans/00000000-0000-0000-0000-000000000000/apply",
                String::new(),
            ),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .header("content-type", "application/json")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
    }

    #[tokio::test]
    async fn remote_edits_and_range_playback_work_before_and_after_apply() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("1.mp4"), b"0123456789").unwrap();
        let app = app(root.path().into());
        let payload = PlanRequest {
            name: "Example".into(),
            source: Source::Manual {
                titles: "Pilot".into(),
            },
            ..Default::default()
        };
        let response = app
            .clone()
            .oneshot(request(
                "POST",
                "/v1/plans",
                serde_json::to_string(&payload).unwrap(),
            ))
            .await
            .unwrap();
        let plan: RenamePlan =
            serde_json::from_slice(&to_bytes(response.into_body(), 1_000_000).await.unwrap())
                .unwrap();
        let edit_path = format!("/v1/plans/{}/tasks/0", plan.id);
        let response = app
            .clone()
            .oneshot(request(
                "PATCH",
                &edit_path,
                "{\"filename\":\"../escape.mp4\"}".into(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let response = app
            .clone()
            .oneshot(request(
                "PATCH",
                &edit_path,
                "{\"filename\":\"Edited.mp4\"}".into(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(root.path().join("1.mp4").exists());
        assert!(!root.path().join("Edited.mp4").exists());
        let response = app
            .clone()
            .oneshot(request("POST", &format!("{edit_path}/play"), String::new()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let value: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 1000).await.unwrap()).unwrap();
        let stream_path = format!("/v1/media/{}", value["ticket"].as_str().unwrap());
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&stream_path)
                    .header("range", "bytes=2-5")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(response.headers()["content-range"], "bytes 2-5/10");
        assert_eq!(
            &to_bytes(response.into_body(), 100).await.unwrap()[..],
            b"2345"
        );
        let response = app
            .clone()
            .oneshot(request(
                "POST",
                &format!("/v1/plans/{}/apply", plan.id),
                String::new(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(root.path().join("Edited.mp4").exists());
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&stream_path)
                    .header("range", "bytes=0-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            &to_bytes(response.into_body(), 100).await.unwrap()[..],
            b"01"
        );
        let response = app
            .oneshot(
                Request::builder()
                    .uri(&stream_path)
                    .header("range", "bytes=999-")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    }

    #[tokio::test]
    async fn preview_rejects_root_escape_before_metadata_lookup() {
        let root = tempfile::tempdir().unwrap();
        let payload = PlanRequest {
            folder: "..".into(),
            name: "Example".into(),
            ..Default::default()
        };
        let response = app(root.path().into())
            .oneshot(request(
                "POST",
                "/v1/plans",
                serde_json::to_string(&payload).unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn remote_manual_preview_applies_once_and_preserves_file_content() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("1.mkv"), b"original content").unwrap();
        let app = app(root.path().into());
        let payload = PlanRequest {
            name: "Example".into(),
            source: Source::Manual {
                titles: "Pilot".into(),
            },
            ..Default::default()
        };
        let response = app
            .clone()
            .oneshot(request(
                "POST",
                "/v1/plans",
                serde_json::to_string(&payload).unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let plan: RenamePlan =
            serde_json::from_slice(&to_bytes(response.into_body(), 1_000_000).await.unwrap())
                .unwrap();
        assert!(root.path().join("1.mkv").exists());
        let path = format!("/v1/plans/{}/apply", plan.id);
        let response = app
            .clone()
            .oneshot(request("POST", &path, String::new()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(!root.path().join("1.mkv").exists());
        assert_eq!(
            std::fs::read(root.path().join("S01E01 - Pilot.mkv")).unwrap(),
            b"original content"
        );
        let response = app
            .oneshot(request("POST", &path, String::new()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
