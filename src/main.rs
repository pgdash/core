use pgdash_lib::api::{ScanRequest, ScanResponse, routes};
use std::net::SocketAddr;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_swagger_ui::SwaggerUi;

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
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .nest("/health", routes::health::health_router())
        .nest("/scan", routes::scan::scan_router())
        .split_for_parts();

    let app = axum::Router::new()
        // .route("/health", axum::routing::get(health_check))
        // .route("/scan", axum::routing::post(scan_database))
        .merge(router)
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", api));

    let addr = SocketAddr::from(([0, 0, 0, 0], 5000));
    info!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
