use crate::scanner::PostgresScanner;
use crate::schema::Database;
use tokio_postgres::NoTls;
use tracing::info;

pub async fn scan_database(db_url: &str) -> Result<Database, String> {
    let parsed = url::Url::parse(db_url).map_err(|e| format!("Invalid db_url: {}", e))?;

    let db_name = parsed.path().trim_start_matches('/').to_string();

    if db_name.is_empty() {
        return Err("db_url must include a database name in the path".to_string());
    }

    let (client, connection) = tokio_postgres::connect(db_url, NoTls)
        .await
        .map_err(|e| format!("Connection error: {}", e))?;

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            tracing::error!("Connection error: {}", e);
        }
    });

    let scanner = PostgresScanner::new(&client);

    info!("Scanning database: {}", db_name);
    scanner.scan(&db_name).await
}
