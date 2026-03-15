use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub admin: AdminConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub port: u16,
    pub log_level: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 5000,
            log_level: "info".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgres://postgres:postgres@localhost/postgres".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct AdminConfig {
    pub username: String,
    pub password: String,
}

impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            username: "admin".to_string(),
            password: "admin".to_string(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let config_path = std::env::var("PGDASH_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("config.yaml"));

        let mut config = if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            serde_yaml::from_str(&content)?
        } else if std::env::var("PGDASH_CONFIG").is_ok() {
            return Err(format!("Config file not found: {:?}", config_path).into());
        } else {
            Config::default()
        };

        if let Ok(port) = std::env::var("PGDASH_SERVER_PORT")
            && let Ok(port) = port.parse()
        {
            config.server.port = port;
        }
        if let Ok(log_level) = std::env::var("PGDASH_SERVER_LOG_LEVEL") {
            config.server.log_level = log_level;
        }
        if let Ok(url) = std::env::var("PGDASH_DATABASE_URL") {
            config.database.url = url;
        }
        if let Ok(username) = std::env::var("PGDASH_ADMIN_USERNAME") {
            config.admin.username = username;
        }
        if let Ok(password) = std::env::var("PGDASH_ADMIN_PASSWORD") {
            config.admin.password = password;
        }

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::Config;
    use std::env;
    use std::fs;
    use std::path::PathBuf;

    fn temp_config_file(content: &str) -> PathBuf {
        let path = env::temp_dir().join("pgdash_test_config.yaml");
        fs::write(&path, content).unwrap();
        path
    }

    fn set_var(key: &str, value: &str) {
        unsafe { env::set_var(key, value) }
    }

    fn remove_var(key: &str) {
        unsafe { env::remove_var(key) }
    }

    #[test]
    fn test_default_config() {
        remove_var("PGDASH_CONFIG");
        remove_var("PGDASH_SERVER_PORT");
        remove_var("PGDASH_SERVER_LOG_LEVEL");
        remove_var("PGDASH_DATABASE_URL");
        remove_var("PGDASH_ADMIN_USERNAME");
        remove_var("PGDASH_ADMIN_PASSWORD");

        let config = Config::default();

        assert_eq!(config.server.port, 5000);
        assert_eq!(config.server.log_level, "info");
        assert_eq!(
            config.database.url,
            "postgres://postgres:postgres@localhost/postgres"
        );
        assert_eq!(config.admin.username, "admin");
        assert_eq!(config.admin.password, "admin");
    }

    #[test]
    fn test_load_config_from_file() {
        remove_var("PGDASH_CONFIG");
        remove_var("PGDASH_SERVER_PORT");
        remove_var("PGDASH_SERVER_LOG_LEVEL");
        remove_var("PGDASH_DATABASE_URL");
        remove_var("PGDASH_ADMIN_USERNAME");
        remove_var("PGDASH_ADMIN_PASSWORD");

        let config_path = temp_config_file(
            r#"
server:
  port: 8080
  log_level: debug
database:
  url: postgres://user:pass@localhost/mydb
admin:
  username: testuser
  password: testpass
"#,
        );

        set_var("PGDASH_CONFIG", config_path.to_str().unwrap());

        let config = Config::load().unwrap();

        assert_eq!(config.server.port, 8080);
        assert_eq!(config.server.log_level, "debug");
        assert_eq!(config.database.url, "postgres://user:pass@localhost/mydb");
        assert_eq!(config.admin.username, "testuser");
        assert_eq!(config.admin.password, "testpass");

        remove_var("PGDASH_CONFIG");
    }

    #[test]
    fn test_env_var_overrides_file() {
        remove_var("PGDASH_CONFIG");
        remove_var("PGDASH_SERVER_PORT");
        remove_var("PGDASH_SERVER_LOG_LEVEL");
        remove_var("PGDASH_DATABASE_URL");
        remove_var("PGDASH_ADMIN_USERNAME");
        remove_var("PGDASH_ADMIN_PASSWORD");

        let config_path = temp_config_file(
            r#"
server:
  port: 8080
  log_level: debug
database:
  url: postgres://user:pass@localhost/mydb
admin:
  username: fileuser
  password: filepass
"#,
        );

        set_var("PGDASH_CONFIG", config_path.to_str().unwrap());
        set_var("PGDASH_SERVER_PORT", "9000");
        set_var("PGDASH_ADMIN_USERNAME", "envuser");

        let config = Config::load().unwrap();

        assert_eq!(config.server.port, 9000);
        assert_eq!(config.server.log_level, "debug");
        assert_eq!(config.database.url, "postgres://user:pass@localhost/mydb");
        assert_eq!(config.admin.username, "envuser");
        assert_eq!(config.admin.password, "filepass");

        remove_var("PGDASH_CONFIG");
    }

    #[test]
    fn test_env_var_port_override() {
        remove_var("PGDASH_CONFIG");
        remove_var("PGDASH_SERVER_PORT");
        remove_var("PGDASH_SERVER_LOG_LEVEL");
        remove_var("PGDASH_DATABASE_URL");
        remove_var("PGDASH_ADMIN_USERNAME");
        remove_var("PGDASH_ADMIN_PASSWORD");

        let config_path = temp_config_file(
            r#"
server:
  port: 5000
  log_level: info
database:
  url: postgres://localhost/postgres
admin:
  username: admin
  password: admin
"#,
        );

        set_var("PGDASH_CONFIG", config_path.to_str().unwrap());
        set_var("PGDASH_SERVER_PORT", "3000");

        let config = Config::load().unwrap();

        assert_eq!(config.server.port, 3000);

        remove_var("PGDASH_CONFIG");
    }

    #[test]
    fn test_env_var_log_level_override() {
        remove_var("PGDASH_CONFIG");
        remove_var("PGDASH_SERVER_PORT");
        remove_var("PGDASH_SERVER_LOG_LEVEL");
        remove_var("PGDASH_DATABASE_URL");
        remove_var("PGDASH_ADMIN_USERNAME");
        remove_var("PGDASH_ADMIN_PASSWORD");

        let config_path = temp_config_file(
            r#"
server:
  port: 5000
  log_level: info
database:
  url: postgres://localhost/postgres
admin:
  username: admin
  password: admin
"#,
        );

        set_var("PGDASH_CONFIG", config_path.to_str().unwrap());
        set_var("PGDASH_SERVER_LOG_LEVEL", "trace");

        let config = Config::load().unwrap();

        assert_eq!(config.server.log_level, "trace");

        remove_var("PGDASH_CONFIG");
    }

    #[test]
    fn test_env_var_database_url_override() {
        remove_var("PGDASH_CONFIG");
        remove_var("PGDASH_SERVER_PORT");
        remove_var("PGDASH_SERVER_LOG_LEVEL");
        remove_var("PGDASH_DATABASE_URL");
        remove_var("PGDASH_ADMIN_USERNAME");
        remove_var("PGDASH_ADMIN_PASSWORD");

        let config_path = temp_config_file(
            r#"
server:
  port: 5000
  log_level: info
database:
  url: postgres://localhost/postgres
admin:
  username: admin
  password: admin
"#,
        );

        set_var("PGDASH_CONFIG", config_path.to_str().unwrap());
        set_var(
            "PGDASH_DATABASE_URL",
            "postgres://custom:custom@customhost/customdb",
        );

        let config = Config::load().unwrap();

        assert_eq!(
            config.database.url,
            "postgres://custom:custom@customhost/customdb"
        );

        remove_var("PGDASH_CONFIG");
    }

    #[test]
    fn test_env_var_admin_credentials_override() {
        remove_var("PGDASH_CONFIG");
        remove_var("PGDASH_SERVER_PORT");
        remove_var("PGDASH_SERVER_LOG_LEVEL");
        remove_var("PGDASH_DATABASE_URL");
        remove_var("PGDASH_ADMIN_USERNAME");
        remove_var("PGDASH_ADMIN_PASSWORD");

        let config_path = temp_config_file(
            r#"
server:
  port: 5000
  log_level: info
database:
  url: postgres://localhost/postgres
admin:
  username: admin
  password: admin
"#,
        );

        set_var("PGDASH_CONFIG", config_path.to_str().unwrap());
        set_var("PGDASH_ADMIN_USERNAME", "newadmin");
        set_var("PGDASH_ADMIN_PASSWORD", "newpassword");

        let config = Config::load().unwrap();

        assert_eq!(config.admin.username, "newadmin");
        assert_eq!(config.admin.password, "newpassword");

        remove_var("PGDASH_CONFIG");
    }

    #[test]
    fn test_config_missing_file_returns_error() {
        remove_var("PGDASH_CONFIG");
        remove_var("PGDASH_SERVER_PORT");
        remove_var("PGDASH_SERVER_LOG_LEVEL");
        remove_var("PGDASH_DATABASE_URL");
        remove_var("PGDASH_ADMIN_USERNAME");
        remove_var("PGDASH_ADMIN_PASSWORD");

        set_var("PGDASH_CONFIG", "/nonexistent/path/config.yaml");

        let result = Config::load();

        assert!(result.is_err());

        remove_var("PGDASH_CONFIG");
    }

    #[test]
    fn test_partial_config_file() {
        remove_var("PGDASH_CONFIG");
        remove_var("PGDASH_SERVER_PORT");
        remove_var("PGDASH_SERVER_LOG_LEVEL");
        remove_var("PGDASH_DATABASE_URL");
        remove_var("PGDASH_ADMIN_USERNAME");
        remove_var("PGDASH_ADMIN_PASSWORD");

        let config_path = temp_config_file(
            r#"
server:
  port: 7000
  log_level: info
"#,
        );

        set_var("PGDASH_CONFIG", config_path.to_str().unwrap());

        let config = Config::load().unwrap();

        assert_eq!(config.server.port, 7000);
        assert_eq!(config.server.log_level, "info");
        assert_eq!(
            config.database.url,
            "postgres://postgres:postgres@localhost/postgres"
        );
        assert_eq!(config.admin.username, "admin");

        remove_var("PGDASH_CONFIG");
    }
}
