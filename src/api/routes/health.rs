use utoipa_axum::{router::OpenApiRouter, routes};

#[utoipa::path(
    get,
    path = "/",
    responses(
        (status = 200, description = "Health check", body = String)
    )
)]
pub async fn health_check() -> &'static str {
    "OK"
}

pub fn health_router() -> OpenApiRouter {
    OpenApiRouter::new().routes(routes!(health_check))
}
