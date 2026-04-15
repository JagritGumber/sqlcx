use crate::error::{Result, SqlcxError};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Deserialize, Clone, Debug)]
pub struct SqlcxConfig {
    pub sql: String,
    pub parser: String,
    pub targets: Vec<TargetConfig>,
    #[serde(default)]
    pub overrides: HashMap<String, String>,
    #[serde(default)]
    pub migrate: Option<MigrateConfig>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct TargetConfig {
    pub language: String,
    pub out: String,
    pub schema: String,
    pub driver: String,
    #[serde(default)]
    pub overrides: HashMap<String, String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct MigrateConfig {
    #[serde(default = "default_migrate_dir")]
    pub dir: String,
    #[serde(default)]
    pub database_url: Option<String>,
    #[serde(default = "default_auto_regenerate")]
    pub auto_regenerate: bool,
}

fn default_migrate_dir() -> String {
    "./sql/migrations".to_string()
}

fn default_auto_regenerate() -> bool {
    true
}

/// Load config from a directory by auto-detecting sqlcx.toml or sqlcx.json.
/// Tries sqlcx.toml first, then sqlcx.json.
pub fn load_config(dir: &Path) -> Result<SqlcxConfig> {
    let toml_path = dir.join("sqlcx.toml");
    if toml_path.exists() {
        let content = std::fs::read_to_string(&toml_path)?;
        return toml::from_str(&content).map_err(SqlcxError::from);
    }

    let json_path = dir.join("sqlcx.json");
    if json_path.exists() {
        let content = std::fs::read_to_string(&json_path)?;
        return serde_json::from_str(&content).map_err(SqlcxError::from);
    }

    Err(SqlcxError::ConfigNotFound(
        "no sqlcx.toml or sqlcx.json found".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_toml_config() {
        let toml_str = r#"
sql = "./sql"
parser = "postgres"
[[targets]]
language = "typescript"
out = "./src/db"
schema = "typebox"
driver = "bun-sql"
[overrides]
uuid = "string"
"#;
        let config: SqlcxConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.sql, "./sql");
        assert_eq!(config.parser, "postgres");
        assert_eq!(config.targets.len(), 1);
        assert_eq!(config.targets[0].language, "typescript");
        assert_eq!(config.overrides.get("uuid"), Some(&"string".to_string()));
    }

    #[test]
    fn deserialize_json_config() {
        let json_str = r#"{"sql":"./sql","parser":"postgres","targets":[{"language":"typescript","out":"./src/db","schema":"typebox","driver":"bun-sql"}]}"#;
        let config: SqlcxConfig = serde_json::from_str(json_str).unwrap();
        assert_eq!(config.sql, "./sql");
        assert_eq!(config.targets.len(), 1);
        assert!(config.overrides.is_empty());
    }

    #[test]
    fn load_config_auto_detect_toml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("sqlcx.toml"),
            r#"
sql = "./sql"
parser = "postgres"
[[targets]]
language = "typescript"
out = "./src/db"
schema = "typebox"
driver = "bun-sql"
"#,
        )
        .unwrap();
        let config = load_config(dir.path()).unwrap();
        assert_eq!(config.parser, "postgres");
    }

    #[test]
    fn load_config_auto_detect_json() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("sqlcx.json"),
            r#"{"sql":"./sql","parser":"postgres","targets":[{"language":"typescript","out":"./src/db","schema":"typebox","driver":"bun-sql"}]}"#,
        )
        .unwrap();
        let config = load_config(dir.path()).unwrap();
        assert_eq!(config.parser, "postgres");
    }

    #[test]
    fn load_config_not_found() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_config(dir.path()).is_err());
    }
}
