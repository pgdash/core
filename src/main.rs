use axum::{Router, extract::Json, routing::post};
use pgdash_lib::scanner::PostgresScanner;
use serde::Deserialize;
use std::net::SocketAddr;
use tokio_postgres::NoTls;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(Deserialize, utoipa::ToSchema)]
struct ScanRequest {
    db_url: String,
}

#[derive(OpenApi)]
#[openapi(
    paths(scan_database, health_check),
    components(schemas(
        ScanRequest,
        pgdash_lib::schema::Database,
        pgdash_lib::schema::Schema,
        pgdash_lib::schema::Table,
        pgdash_lib::schema::Column,
        pgdash_lib::schema::Index,
        pgdash_lib::schema::Constraint,
        pgdash_lib::schema::Trigger,
        pgdash_lib::schema::View,
        pgdash_lib::schema::EnumType,
        pgdash_lib::schema::Sequence,
        pgdash_lib::schema::Function,
        pgdash_lib::schema::PostgresDataType,
        pgdash_lib::schema::ReferentialAction,
        pgdash_lib::schema::ConstraintType,
    ))
)]
struct ApiDoc;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = Router::new()
        .route("/health", axum::routing::get(health_check))
        .route("/scan", post(scan_database))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()));

    let addr = SocketAddr::from(([0, 0, 0, 0], 5000));
    info!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Health check", body = String)
    )
)]
async fn health_check() -> &'static str {
    "OK"
}

#[utoipa::path(
    post,
    path = "/scan",
    request_body = ScanRequest,
    responses(
        (status = 200, description = "Database scan result", body = pgdash_lib::schema::Database),
        (status = 400, description = "Invalid request or connection error", body = String),
        (status = 500, description = "Scanner error", body = String)
    )
)]
async fn scan_database(
    Json(payload): Json<ScanRequest>,
) -> Result<Json<pgdash_lib::schema::Database>, (axum::http::StatusCode, String)> {
    let parsed = url::Url::parse(&payload.db_url).map_err(|e| {
        error!("Invalid db_url: {}", e);
        (
            axum::http::StatusCode::BAD_REQUEST,
            format!("Invalid db_url: {}", e),
        )
    })?;

    let db_name = parsed.path().trim_start_matches('/').to_string();

    if db_name.is_empty() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "db_url must include a database name in the path".to_string(),
        ));
    }

    let (client, connection) = tokio_postgres::connect(&payload.db_url, NoTls)
        .await
        .map_err(|e| {
            error!("Failed to connect: {}", e);
            (
                axum::http::StatusCode::BAD_REQUEST,
                format!("Connection error: {}", e),
            )
        })?;

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            error!("Connection error: {}", e);
        }
    });

    let scanner = PostgresScanner::new(&client);

    info!("Scanning database: {}", db_name);
    match scanner.scan(&db_name).await {
        Ok(database) => Ok(Json(database)),
        Err(e) => {
            error!("Scanner error: {}", e);
            Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to scan database: {}", e),
            ))
        }
    }
}
