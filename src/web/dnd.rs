use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Html,
    Json,
};
use chrono::Local;
use serde::{Deserialize, Serialize};

use crate::data::dnd::{is_read_only, DndRepository, MonthKey, RankedDate};

#[derive(Clone)]
pub struct DndWebState {
    pub dnd: Arc<DndRepository>,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub month_token: String,
    pub name: String,
    pub code: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub edit_token: String,
    pub selected_dates: Vec<String>,
    pub month_name: String,
    pub readonly: bool,
}

#[derive(Deserialize)]
pub struct AvailabilityPatchRequest {
    pub edit_token: String,
    pub set_available: Vec<String>,
    pub set_unavailable: Vec<String>,
}

#[derive(Serialize)]
pub struct AvailabilityPatchResponse {
    pub ok: bool,
    pub selected_count: usize,
}

#[derive(Deserialize)]
pub struct SummaryQuery {
    pub month_token: String,
}

#[derive(Serialize)]
pub struct SummaryResponse {
    pub participants: i64,
    pub dates: Vec<RankedDate>,
}

fn month_is_read_only(month_name: &str) -> bool {
    match MonthKey::parse(month_name) {
        Ok(month_key) => is_read_only(month_key, Local::now().date_naive()),
        Err(_) => true,
    }
}

fn internal_error(err: sqlx::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

pub fn router(state: DndWebState) -> axum::Router {
    axum::Router::new()
        .route("/dnd/{month_token}", axum::routing::get(dnd_page))
        .route("/api/dnd/login", axum::routing::post(login))
        .route("/api/dnd/availability", axum::routing::patch(patch_availability))
        .route("/api/dnd/summary", axum::routing::get(summary))
        .with_state(state)
}

pub async fn dnd_page(Path(month_token): Path<String>) -> Html<String> {
    let page = include_str!("assets/dnd.html").replace("{{MONTH_TOKEN}}", &month_token);
    Html(page)
}

pub async fn login(
    State(state): State<DndWebState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, String)> {
    let month = state
        .dnd
        .month_by_access_token(&req.month_token)
        .await
        .map_err(internal_error)?
        .ok_or((StatusCode::NOT_FOUND, "Unknown month token".to_string()))?;

    let participant = state
        .dnd
        .participant_by_month_and_name(month.id, &req.name)
        .await
        .map_err(internal_error)?
        .ok_or((StatusCode::UNAUTHORIZED, "Unknown name for this month".to_string()))?;

    let valid = state
        .dnd
        .validate_login_code(participant.id, &req.code)
        .await
        .map_err(internal_error)?;
    if !valid {
        return Err((StatusCode::UNAUTHORIZED, "Invalid or expired login code".to_string()));
    }

    let selected_dates = state
        .dnd
        .selected_dates(participant.id)
        .await
        .map_err(internal_error)?;

    let readonly = month_is_read_only(&month.month);
    Ok(Json(LoginResponse {
        edit_token: participant.edit_token,
        selected_dates,
        month_name: month.month,
        readonly,
    }))
}

pub async fn patch_availability(
    State(state): State<DndWebState>,
    Json(req): Json<AvailabilityPatchRequest>,
) -> Result<Json<AvailabilityPatchResponse>, (StatusCode, String)> {
    let participant = state
        .dnd
        .participant_by_edit_token(&req.edit_token)
        .await
        .map_err(internal_error)?
        .ok_or((StatusCode::UNAUTHORIZED, "Invalid edit token".to_string()))?;

    let month = state
        .dnd
        .month_by_id(participant.month_id)
        .await
        .map_err(internal_error)?
        .ok_or((StatusCode::NOT_FOUND, "Month not found".to_string()))?;

    if month_is_read_only(&month.month) {
        return Err((
            StatusCode::CONFLICT,
            "This month is read-only and can no longer be edited".to_string(),
        ));
    }

    for date in &req.set_available {
        state
            .dnd
            .set_day_available(participant.id, date)
            .await
            .map_err(internal_error)?;
    }
    for date in &req.set_unavailable {
        state
            .dnd
            .set_day_unavailable(participant.id, date)
            .await
            .map_err(internal_error)?;
    }

    let selected_count = state
        .dnd
        .selected_dates(participant.id)
        .await
        .map_err(internal_error)?
        .len();

    Ok(Json(AvailabilityPatchResponse {
        ok: true,
        selected_count,
    }))
}

pub async fn summary(
    State(state): State<DndWebState>,
    Query(query): Query<SummaryQuery>,
) -> Result<Json<SummaryResponse>, (StatusCode, String)> {
    let month = state
        .dnd
        .month_by_access_token(&query.month_token)
        .await
        .map_err(internal_error)?
        .ok_or((StatusCode::NOT_FOUND, "Unknown month token".to_string()))?;

    let participants = state
        .dnd
        .participant_count_for_month_id(month.id)
        .await
        .map_err(internal_error)?;
    let dates = state
        .dnd
        .summary_for_month_id(month.id)
        .await
        .map_err(internal_error)?;

    Ok(Json(SummaryResponse {
        participants,
        dates,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, Duration};

    async fn test_state() -> DndWebState {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let repo = DndRepository::new(pool);
        repo.init_for_tests().await.unwrap();
        DndWebState { dnd: Arc::new(repo) }
    }

    #[tokio::test]
    async fn login_returns_edit_token_for_valid_code() {
        let state = test_state().await;
        let month = state
            .dnd
            .ensure_month("123", MonthKey::March)
            .await
            .unwrap();
        let participant = state
            .dnd
            .ensure_participant(month.id, "sean")
            .await
            .unwrap();
        let code = state
            .dnd
            .issue_login_code(participant.id, Duration::minutes(15))
            .await
            .unwrap();

        let response = login(
            State(state),
            Json(LoginRequest {
                month_token: month.access_token,
                name: "sean".to_string(),
                code,
            }),
        )
        .await
        .unwrap();

        assert_eq!(response.edit_token, participant.edit_token);
    }

    #[tokio::test]
    async fn availability_patch_rejects_read_only_month() {
        if Local::now().month() == 1 {
            return;
        }

        let state = test_state().await;
        let month = state
            .dnd
            .ensure_month("123", MonthKey::January)
            .await
            .unwrap();
        let participant = state
            .dnd
            .ensure_participant(month.id, "sean")
            .await
            .unwrap();

        let result = patch_availability(
            State(state),
            Json(AvailabilityPatchRequest {
                edit_token: participant.edit_token,
                set_available: vec!["2026-01-05".to_string()],
                set_unavailable: vec![],
            }),
        )
        .await;

        assert!(result.is_err());
        let (status, _) = result.err().unwrap();
        assert_eq!(status, StatusCode::CONFLICT);
    }
}
