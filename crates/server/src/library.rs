use super::*;
use serde::Deserialize;
use tv_manager_core::{health, history, metadata};
#[derive(Deserialize)]
pub struct Folder {
    #[serde(default)]
    pub folder: String,
}
fn folder(state: &AppState, value: &str) -> Result<PathBuf, ApiError> {
    rename::scoped_folder(&state.root, value).map_err(|e| error(StatusCode::BAD_REQUEST, e))
}
pub async fn search(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PlanRequest>,
) -> Result<Json<Vec<tv_manager_core::ShowMatch>>, ApiError> {
    authenticate(&state, &headers)?;
    metadata::search(&request)
        .await
        .map(Json)
        .map_err(|e| error(StatusCode::BAD_REQUEST, e))
}
pub async fn history_list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(query): Json<Folder>,
) -> Result<Json<Vec<history::HistoryRecord>>, ApiError> {
    authenticate(&state, &headers)?;
    let root = folder(&state, &query.folder)?;
    let _permit = state
        .gate
        .clone()
        .try_acquire_owned()
        .map_err(|_| error(StatusCode::CONFLICT, "Another file operation is running"))?;
    tokio::task::spawn_blocking(move || history::list(&root))
        .await
        .map_err(|_| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "History read interrupted",
            )
        })?
        .map(Json)
        .map_err(|e| error(StatusCode::BAD_REQUEST, e))
}
pub async fn undo(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(query): Json<Folder>,
) -> Result<Json<history::HistoryRecord>, ApiError> {
    authenticate(&state, &headers)?;
    let root = folder(&state, &query.folder)?;
    let permit = state
        .gate
        .clone()
        .try_acquire_owned()
        .map_err(|_| error(StatusCode::CONFLICT, "Another file operation is running"))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        history::undo(&root, id)
    })
    .await
    .map_err(|_| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Undo interrupted; refresh history before retrying",
        )
    })?
    .map(Json)
    .map_err(|e| error(StatusCode::CONFLICT, e))
}
pub async fn health_scan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<health::HealthReport>, ApiError> {
    authenticate(&state, &headers)?;
    let plan = state
        .plans
        .lock()
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Plan store unavailable"))?
        .get(&id)
        .filter(|(created, _)| created.elapsed() < Duration::from_secs(900))
        .map(|(_, p)| p.clone())
        .ok_or_else(|| {
            error(
                StatusCode::NOT_FOUND,
                "Build a fresh preview before scanning",
            )
        })?;
    let permit = state
        .gate
        .clone()
        .try_acquire_owned()
        .map_err(|_| error(StatusCode::CONFLICT, "Another file operation is running"))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        health::scan(&plan)
    })
    .await
    .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Scan interrupted"))?
    .map(Json)
    .map_err(|e| error(StatusCode::BAD_REQUEST, e))
}
pub async fn create_pairing(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    authenticate(&state, &headers)?;
    let mut codes = state
        .pairings
        .lock()
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Pairing unavailable"))?;
    codes.retain(|_, created| created.elapsed() < Duration::from_secs(300));
    if codes.len() >= 8 {
        return Err(error(
            StatusCode::TOO_MANY_REQUESTS,
            "Wait for an existing pairing code to expire",
        ));
    }
    let code = Uuid::new_v4();
    codes.insert(code, Instant::now());
    Ok(Json(serde_json::json!({"code":code,"expires_in":300})))
}
#[derive(Deserialize)]
pub struct Redeem {
    code: Uuid,
}
pub async fn redeem(
    State(state): State<AppState>,
    Json(request): Json<Redeem>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let created = state
        .pairings
        .lock()
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Pairing unavailable"))?
        .remove(&request.code);
    if !created.is_some_and(|created| created.elapsed() < Duration::from_secs(300)) {
        return Err(error(
            StatusCode::UNAUTHORIZED,
            "Pairing code expired or already used",
        ));
    }
    Ok(Json(serde_json::json!({"token":state.token.as_str()})))
}

fn artwork_plan(state: &AppState, id: Uuid) -> Result<RenamePlan, ApiError> {
    state
        .plans
        .lock()
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Plan store unavailable"))?
        .get(&id)
        .filter(|(created, _)| created.elapsed() < Duration::from_secs(900))
        .map(|(_, plan)| plan.clone())
        .ok_or_else(|| {
            error(
                StatusCode::NOT_FOUND,
                "Preview expired; update Workspace details",
            )
        })
}
pub async fn artwork(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<tv_manager_core::artwork::Poster>>, ApiError> {
    authenticate(&state, &headers)?;
    let plan = artwork_plan(&state, id)?;
    tokio::task::spawn_blocking(move || tv_manager_core::artwork::inventory(&plan))
        .await
        .map_err(|_| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Artwork read interrupted",
            )
        })?
        .map(Json)
        .map_err(|e| error(StatusCode::BAD_REQUEST, e))
}
pub async fn save_artwork(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(save): Json<tv_manager_core::artwork::Save>,
) -> Result<Json<Vec<tv_manager_core::artwork::Poster>>, ApiError> {
    authenticate(&state, &headers)?;
    let plan = artwork_plan(&state, id)?;
    let permit = state
        .gate
        .clone()
        .try_acquire_owned()
        .map_err(|_| error(StatusCode::CONFLICT, "Another file operation is running"))?;
    tv_manager_core::artwork::validate_save(&plan, &save)
        .map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
    let bytes = tv_manager_core::artwork::fetch(&save.url)
        .await
        .map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        tv_manager_core::artwork::write(&plan, &save, &bytes)?;
        tv_manager_core::artwork::inventory(&plan)
    })
    .await
    .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Poster save interrupted"))?
    .map(Json)
    .map_err(|e| error(StatusCode::CONFLICT, e))
}

pub async fn download_artwork(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<tv_manager_core::artwork::Batch>, ApiError> {
    authenticate(&state, &headers)?;
    let plan = artwork_plan(&state, id)?;
    let permit = state
        .gate
        .clone()
        .try_acquire_owned()
        .map_err(|_| error(StatusCode::CONFLICT, "Another file operation is running"))?;
    // Keep the operation alive if a mobile client disconnects mid-download.
    tokio::spawn(async move {
        let _permit = permit;
        tv_manager_core::artwork::download_missing(&plan).await
    })
    .await
    .map_err(|_| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Poster download interrupted",
        )
    })?
    .map(Json)
    .map_err(|e| error(StatusCode::BAD_REQUEST, e))
}
