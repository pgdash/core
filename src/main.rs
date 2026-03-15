use pgdash_lib::api::{ScanRequest, ScanResponse, routes};
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_swagger_ui::SwaggerUi;

use pgdash_lib::app_config::Config;

#[derive(OpenApi)]
#[openapi(
    components(schemas(ScanRequest, ScanResponse,)),
    tags(
        (name = "health", description = "health check endpoints"),
        (name = "scan", description = "scan database endpoints")
    ),
    info(title = "My API", version = env!("CARGO_PKG_VERSION"))
)]
struct ApiDoc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = match Config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!(
                "Warning: Failed to load config, using defaults. Error: {}",
                e
            );
            Config::default()
        }
    };

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| config.server.log_level.clone()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Admin username: {}", config.admin.username);

    let cors_layer = CorsLayer::new()
        .allow_origin(Any)
        .allow_headers(Any)
        .allow_methods(Any);

    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .nest("/health", routes::health::health_router())
        .nest("/scan", routes::scan::scan_router())
        .layer(ServiceBuilder::new().layer(cors_layer))
        .split_for_parts();

    let app = axum::Router::new()
        .merge(router)
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", api));

    let addr = SocketAddr::from(([0, 0, 0, 0], config.server.port));
    info!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
