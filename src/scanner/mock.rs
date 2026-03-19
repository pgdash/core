#[cfg(test)]
pub mod mock_client {
    use crate::scanner::traits::{DatabaseClient, DatabaseRow};
    use std::future::Future;

    pub struct MockRow {
        data: serde_json::Value,
    }

    impl MockRow {
        pub fn new(data: serde_json::Value) -> Self {
            Self { data }
        }
    }

    impl DatabaseRow for MockRow {
        fn get_string(&self, name: &str) -> String {
            self.data.get(name).unwrap().as_str().unwrap().to_string()
        }
        fn get_opt_string(&self, name: &str) -> Option<String> {
            self.data
                .get(name)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        }
        fn get_str(&self, name: &str) -> &str {
            self.data.get(name).unwrap().as_str().unwrap()
        }
        fn get_opt_str(&self, name: &str) -> Option<&str> {
            self.data.get(name).and_then(|v| v.as_str())
        }
        fn get_u32(&self, name: &str) -> u32 {
            self.data.get(name).unwrap().as_u64().unwrap() as u32
        }
        fn get_opt_u32(&self, name: &str) -> Option<u32> {
            self.data
                .get(name)
                .and_then(|v| v.as_u64())
                .map(|u| u as u32)
        }
        fn get_i32(&self, name: &str) -> i32 {
            self.data.get(name).unwrap().as_i64().unwrap() as i32
        }
        fn get_opt_i32(&self, name: &str) -> Option<i32> {
            self.data
                .get(name)
                .and_then(|v| v.as_i64())
                .map(|i| i as i32)
        }
        fn get_i64(&self, name: &str) -> i64 {
            self.data.get(name).unwrap().as_i64().unwrap()
        }
        fn get_opt_i64(&self, name: &str) -> Option<i64> {
            self.data.get(name).and_then(|v| v.as_i64())
        }
        fn get_bool(&self, name: &str) -> bool {
            self.data.get(name).unwrap().as_bool().unwrap()
        }
        fn get_opt_bool(&self, name: &str) -> Option<bool> {
            self.data.get(name).and_then(|v| v.as_bool())
        }
        fn get_vec_string(&self, name: &str) -> Vec<String> {
            self.data
                .get(name)
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect()
        }
        fn try_get_string(&self, name: &str) -> Result<String, String> {
            self.data
                .get(name)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| "Not found".to_string())
        }
        fn try_get_str(&self, name: &str) -> Result<&str, String> {
            self.data
                .get(name)
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Not found".to_string())
        }
        fn try_get_u32(&self, name: &str) -> Result<u32, String> {
            self.data
                .get(name)
                .and_then(|v| v.as_u64())
                .map(|u| u as u32)
                .ok_or_else(|| "Not found".to_string())
        }
    }

    pub struct MockClient {
        pub responses: std::collections::HashMap<String, serde_json::Value>,
    }

    impl MockClient {
        pub fn new() -> Self {
            Self {
                responses: std::collections::HashMap::new(),
            }
        }
        pub fn add_response(&mut self, query_substring: &str, response: serde_json::Value) {
            self.responses.insert(query_substring.to_string(), response);
        }
    }

    impl DatabaseClient for MockClient {
        type Row = MockRow;

        fn query<'a>(
            &'a self,
            statement: &'a str,
            _params: &'a [&'a (dyn tokio_postgres::types::ToSql + Sync)],
        ) -> impl Future<Output = Result<Vec<Self::Row>, String>> + Send {
            let mut result = Vec::new();
            for (k, v) in &self.responses {
                // Return matching rows based on substring key
                if statement.contains(k) {
                    if let Some(arr) = v.as_array() {
                        for item in arr {
                            result.push(MockRow::new(item.clone()));
                        }
                    }
                    return std::future::ready(Ok(result));
                }
            }
            // Return empty by default if not mocked, instead of erroring, or mock requires exact keys.
            // Let's return empty if not mocked for test robustness.
            std::future::ready(Ok(Vec::new()))
        }
    }
}
