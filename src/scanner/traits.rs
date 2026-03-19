use std::future::Future;

pub trait DatabaseRow: Send + Sync {
    fn get_string(&self, name: &str) -> String;
    fn get_opt_string(&self, name: &str) -> Option<String>;
    fn get_str(&self, name: &str) -> &str;
    fn get_opt_str(&self, name: &str) -> Option<&str>;
    fn get_u32(&self, name: &str) -> u32;
    fn get_opt_u32(&self, name: &str) -> Option<u32>;
    fn get_i32(&self, name: &str) -> i32;
    fn get_opt_i32(&self, name: &str) -> Option<i32>;
    fn get_i64(&self, name: &str) -> i64;
    fn get_opt_i64(&self, name: &str) -> Option<i64>;
    fn get_bool(&self, name: &str) -> bool;
    fn get_opt_bool(&self, name: &str) -> Option<bool>;
    fn get_vec_string(&self, name: &str) -> Vec<String>;

    fn try_get_string(&self, name: &str) -> Result<String, String>;
    fn try_get_str(&self, name: &str) -> Result<&str, String>;
    fn try_get_u32(&self, name: &str) -> Result<u32, String>;
}

pub trait DatabaseClient: Send + Sync {
    type Row: DatabaseRow;

    fn query<'a>(
        &'a self,
        statement: &'a str,
        params: &'a [&'a (dyn tokio_postgres::types::ToSql + Sync)],
    ) -> impl Future<Output = Result<Vec<Self::Row>, String>> + Send;
}

impl DatabaseRow for tokio_postgres::Row {
    fn get_string(&self, name: &str) -> String {
        self.get(name)
    }
    fn get_opt_string(&self, name: &str) -> Option<String> {
        self.get(name)
    }
    fn get_str(&self, name: &str) -> &str {
        self.get(name)
    }
    fn get_opt_str(&self, name: &str) -> Option<&str> {
        self.get(name)
    }
    fn get_u32(&self, name: &str) -> u32 {
        self.get(name)
    }
    fn get_opt_u32(&self, name: &str) -> Option<u32> {
        self.get(name)
    }
    fn get_i32(&self, name: &str) -> i32 {
        self.get(name)
    }
    fn get_opt_i32(&self, name: &str) -> Option<i32> {
        self.get(name)
    }
    fn get_i64(&self, name: &str) -> i64 {
        self.get(name)
    }
    fn get_opt_i64(&self, name: &str) -> Option<i64> {
        self.get(name)
    }
    fn get_bool(&self, name: &str) -> bool {
        self.get(name)
    }
    fn get_opt_bool(&self, name: &str) -> Option<bool> {
        self.get(name)
    }
    fn get_vec_string(&self, name: &str) -> Vec<String> {
        self.get(name)
    }

    fn try_get_string(&self, name: &str) -> Result<String, String> {
        self.try_get(name).map_err(|e| e.to_string())
    }
    fn try_get_str(&self, name: &str) -> Result<&str, String> {
        self.try_get(name).map_err(|e| e.to_string())
    }
    fn try_get_u32(&self, name: &str) -> Result<u32, String> {
        self.try_get(name).map_err(|e| e.to_string())
    }
}

impl DatabaseClient for tokio_postgres::Client {
    type Row = tokio_postgres::Row;

    async fn query<'a>(
        &'a self,
        statement: &'a str,
        params: &'a [&'a (dyn tokio_postgres::types::ToSql + Sync)],
    ) -> Result<Vec<Self::Row>, String> {
        self.query(statement, params)
            .await
            .map_err(|e| e.to_string())
    }
}
