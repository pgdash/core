use axum::{Router, extract::Json, routing::post};
use pgdash_lib::scanner::PostgresScanner;
use serde::Deserialize;
use std::net::SocketAddr;
use tokio_postgres::NoTls;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Deserialize)]
struct ScanRequest {
    db_url: String,
    db_name: String,
}

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
        .route("/scan", post(scan_database));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health_check() -> &'static str {
    "OK"
}

async fn scan_database(
    Json(payload): Json<ScanRequest>,
) -> Result<Json<pgdash_lib::schema::Database>, (axum::http::StatusCode, String)> {
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

    match scanner.scan(&payload.db_name).await {
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
