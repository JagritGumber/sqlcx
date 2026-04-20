pub mod common;
pub mod serde_structs;
pub mod sqlx_driver;
pub mod tokio_postgres;

use crate::config::TargetConfig;
use crate::error::{Result, SqlcxError};
use crate::generator::{DriverGenerator, GeneratedFile, LanguagePlugin, SchemaGenerator};
use crate::ir::SqlcxIR;

use self::serde_structs::SerdeStructGenerator;
use self::sqlx_driver::SqlxGenerator;

pub struct RustPlugin {
    pub schema_name: String,
    pub driver_name: String,
}

impl RustPlugin {
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
        "serde" => Ok(Box::new(SerdeStructGenerator)),
        _ => Err(SqlcxError::UnknownSchema(name.to_string())),
    }
}

fn resolve_driver(name: &str) -> Result<Box<dyn DriverGenerator>> {
    match name {
        "sqlx" => Ok(Box::new(SqlxGenerator)),
        "tokio-postgres" => Ok(Box::new(tokio_postgres::TokioPostgresGenerator)),
        _ => Err(SqlcxError::UnknownDriver(name.to_string())),
    }
}

impl LanguagePlugin for RustPlugin {
    fn generate(&self, ir: &SqlcxIR, config: &TargetConfig) -> Result<Vec<GeneratedFile>> {
        let schema_gen = resolve_schema(&self.schema_name)?;
        let driver_gen = resolve_driver(&self.driver_name)?;
        let overrides = &config.overrides;

        let mut files = Vec::new();

        // Schema file (models.rs)
        files.push(schema_gen.generate(ir, overrides)?);

        // Driver files (client.rs + *_queries.rs)
        files.extend(driver_gen.generate(ir)?);

        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TargetConfig;
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
    fn generates_three_files() {
        let ir = parse_fixture_ir();
        let plugin = RustPlugin::new("serde", "sqlx").unwrap();
        let config = TargetConfig {
            language: "rust".to_string(),
            out: "./src/db".to_string(),
            schema: "serde".to_string(),
            driver: "sqlx".to_string(),
            overrides: HashMap::new(),
        };
        let files = plugin.generate(&ir, &config).unwrap();
        assert_eq!(files.len(), 3);
        assert!(files.iter().any(|f| f.path == "models.rs"));
        assert!(files.iter().any(|f| f.path == "client.rs"));
        assert!(files.iter().any(|f| f.path == "users_queries.rs"));
    }
}
