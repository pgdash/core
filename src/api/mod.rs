pub mod dto;
pub mod routes;

pub use dto::{ScanRequest, ScanResponse};
pub use routes::{health_check, scan_database};
