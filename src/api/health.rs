use axum::{Json, extract::State, http::StatusCode};

use crate::{
    api::dto::{HealthResponseDto, ReadinessResponseDto},
    application::AppState,
};

pub async fn health(State(_state): State<AppState>) -> Json<HealthResponseDto> {
    Json(HealthResponseDto {
        status: "ok".to_owned(),
    })
}

pub async fn readiness(State(state): State<AppState>) -> (StatusCode, Json<ReadinessResponseDto>) {
    if state.lifecycle.is_ready() {
        (
            StatusCode::OK,
            Json(ReadinessResponseDto {
                status: "ready".to_owned(),
            }),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ReadinessResponseDto {
                status: "not_ready".to_owned(),
            }),
        )
    }
}
