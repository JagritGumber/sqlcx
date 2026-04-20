pub mod asyncpg;
pub mod common;
pub mod mysql_connector;
pub mod psycopg;
pub mod pydantic;
pub mod sqlite3_driver;

use crate::config::TargetConfig;
use crate::error::{Result, SqlcxError};
use crate::generator::{DriverGenerator, GeneratedFile, LanguagePlugin, SchemaGenerator};
use crate::ir::SqlcxIR;

use self::pydantic::PydanticGenerator;

pub struct PythonPlugin {
    pub schema_name: String,
    pub driver_name: String,
}

impl PythonPlugin {
    pub fn new(schema: &str, driver: &str) -> Result<Self> {
        resolve_schema(schema)?;
        resolve_driver(driver)?;
        Ok(Self {
            schema_name: schema.to_string(),
            driver_name: driver.to_string(),
        })
    }
}

fn resolve_schema(name: &str) -> Result<Box<dyn SchemaGenerator>> {
    match name {
        "pydantic" => Ok(Box::new(PydanticGenerator)),
        _ => Err(SqlcxError::UnknownSchema(name.to_string())),
    }
}

fn resolve_driver(name: &str) -> Result<Option<Box<dyn DriverGenerator>>> {
    match name {
        "none" => Ok(None),
        "psycopg" => Ok(Some(Box::new(psycopg::PsycopgGenerator))),
        "asyncpg" => Ok(Some(Box::new(asyncpg::AsyncpgGenerator))),
        "sqlite3" => Ok(Some(Box::new(sqlite3_driver::Sqlite3Generator))),
        "mysql-connector" => Ok(Some(Box::new(mysql_connector::MysqlConnectorGenerator))),
        _ => Err(SqlcxError::UnknownDriver(name.to_string())),
    }
}

impl LanguagePlugin for PythonPlugin {
    fn generate(&self, ir: &SqlcxIR, config: &TargetConfig) -> Result<Vec<GeneratedFile>> {
        let schema_gen = resolve_schema(&self.schema_name)?;
        let overrides = &config.overrides;

        let mut files = vec![schema_gen.generate(ir, overrides)?];

        if let Some(driver_gen) = resolve_driver(&self.driver_name)? {
            files.extend(driver_gen.generate(ir)?);
        }

        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::LanguagePlugin;
    use crate::parser::DatabaseParser;
    use crate::parser::postgres::PostgresParser;
    use std::collections::HashMap;

    fn parse_fixture_ir() -> SqlcxIR {
        let schema_sql = include_str!("../../../../../tests/fixtures/schema.sql");
        let queries_sql = include_str!("../../../../../tests/fixtures/queries/users.sql");
        let parser = PostgresParser::new();
        let (tables, enums) = parser.parse_schema(schema_sql).unwrap();
        let queries = parser
            .parse_queries(queries_sql, &tables, &enums, "queries/users.sql")
            .unwrap();
        SqlcxIR {
            tables,
            queries,
            enums,
        }
    }

    #[test]
    fn generates_one_file_with_no_driver() {
        let ir = parse_fixture_ir();
        let plugin = PythonPlugin::new("pydantic", "none").unwrap();
        let config = TargetConfig {
            language: "python".to_string(),
            out: "./src/db".to_string(),
            schema: "pydantic".to_string(),
            driver: "none".to_string(),
            overrides: HashMap::new(),
        };
        let files = plugin.generate(&ir, &config).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files.iter().any(|f| f.path == "models.py"));
    }

    #[test]
    fn generates_three_files_with_psycopg() {
        let ir = parse_fixture_ir();
        let plugin = PythonPlugin::new("pydantic", "psycopg").unwrap();
        let config = TargetConfig {
            language: "python".to_string(),
            out: "./src/db".to_string(),
            schema: "pydantic".to_string(),
            driver: "psycopg".to_string(),
            overrides: HashMap::new(),
        };
        let files = plugin.generate(&ir, &config).unwrap();
        assert_eq!(files.len(), 3);
        assert!(files.iter().any(|f| f.path == "models.py"));
        assert!(files.iter().any(|f| f.path == "client.py"));
        assert!(files.iter().any(|f| f.path.ends_with("_queries.py")));
    }

    #[test]
    fn generates_three_files_with_asyncpg() {
        let ir = parse_fixture_ir();
        let plugin = PythonPlugin::new("pydantic", "asyncpg").unwrap();
        let config = TargetConfig {
            language: "python".to_string(),
            out: "./src/db".to_string(),
            schema: "pydantic".to_string(),
            driver: "asyncpg".to_string(),
            overrides: HashMap::new(),
        };
        let files = plugin.generate(&ir, &config).unwrap();
        assert_eq!(files.len(), 3);
        assert!(files.iter().any(|f| f.path == "models.py"));
        assert!(files.iter().any(|f| f.path == "client.py"));
    }
}
