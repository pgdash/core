use crate::schema::Database;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ScanRequest {
    #[schema(example = "postgres://user:pass@localhost:5432/mydb")]
    pub db_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = ScanResponse)]
pub struct ScanResponse(pub Database);
