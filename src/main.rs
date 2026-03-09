use axum::{Router, extract::State, response::Json, routing::get};
use deadpool_postgres::{Config, ManagerConfig, Pool, RecyclingMethod, Runtime};
use pgdash_lib::scanner::PostgresScanner;
use std::net::SocketAddr;
use tokio_postgres::NoTls;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

struct AppState {
    pool: Pool,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let db_url = "postgres://postgres:postgres@localhost/dvdrental?sslmode=disable";
    let mut cfg = Config::new();
    cfg.url = Some(db_url.to_string());
    cfg.manager = Some(ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    });

    let pool = cfg
        .create_pool(Some(Runtime::Tokio1), NoTls)
        .expect("Failed to create pool");

    let state = std::sync::Arc::new(AppState { pool });

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/scan", get(scan_database))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health_check() -> &'static str {
    "OK"
}

async fn scan_database(
    State(state): State<std::sync::Arc<AppState>>,
) -> Result<Json<pgdash_lib::schema::Database>, (axum::http::StatusCode, String)> {
    let conn = state.pool.get().await.map_err(|e| {
        error!("Failed to get connection from pool: {}", e);
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Database connection error: {e}"),
        )
    })?;

    let db_name = "dvdrental".to_string();
    let scanner = PostgresScanner::new(&conn);

    match scanner.scan(&db_name).await {
        Ok(database) => Ok(Json(database)),
        Err(e) => {
            error!("Scanner error: {}", e);
            Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to scan database: {e}"),
            ))
        }
    }
}
