use crate::api::dto::{ScanRequest, ScanResponse};
use crate::service::database_service;
use axum::{extract::Json, http::StatusCode};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

#[utoipa::path(
    post,
    path = "/",
    request_body = ScanRequest,
    responses(
        (status = 200, description = "Database scan result", body = ScanResponse),
        (status = 400, description = "Invalid request or connection error", body = String),
        (status = 500, description = "Scanner error", body = String)
    )
)]
pub async fn scan_database(
    Json(payload): Json<ScanRequest>,
) -> Result<Json<ScanResponse>, (StatusCode, String)> {
    database_service::scan_database(&payload.db_url)
        .await
        .map(|db| Json(ScanResponse(db)))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}

pub fn scan_router() -> OpenApiRouter {
    OpenApiRouter::new().routes(routes!(scan_database))
}
