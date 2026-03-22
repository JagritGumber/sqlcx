pub mod structs;
pub mod database_sql;

use crate::error::{Result, SqlcxError};
use crate::config::TargetConfig;
use crate::generator::{GeneratedFile, LanguagePlugin, SchemaGenerator, DriverGenerator};
use crate::ir::SqlcxIR;

pub struct GoPlugin {
    pub schema_name: String,
    pub driver_name: String,
}

impl GoPlugin {
    pub fn new(schema: &str, driver: &str) -> Result<Self> {
        Ok(Self {
            schema_name: schema.to_string(),
            driver_name: driver.to_string(),
        })
    }
}

fn resolve_schema(name: &str) -> Result<Box<dyn SchemaGenerator>> {
    match name {
        "structs" => Ok(Box::new(structs::GoStructGenerator)),
        _ => Err(SqlcxError::UnknownSchema(name.to_string())),
    }
}

fn resolve_driver(name: &str) -> Result<Box<dyn DriverGenerator>> {
    match name {
        "database-sql" => Ok(Box::new(database_sql::DatabaseSqlGenerator)),
        _ => Err(SqlcxError::UnknownDriver(name.to_string())),
    }
}

impl LanguagePlugin for GoPlugin {
    fn generate(&self, ir: &SqlcxIR, config: &TargetConfig) -> Result<Vec<GeneratedFile>> {
        let schema_gen = resolve_schema(&self.schema_name)?;
        let driver_gen = resolve_driver(&self.driver_name)?;
        let overrides = &config.overrides;

        let mut files = Vec::new();
        files.push(schema_gen.generate(ir, overrides)?);
        files.extend(driver_gen.generate(ir)?);
        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use crate::parser::postgres::PostgresParser;
    use crate::parser::DatabaseParser;
    use crate::generator::LanguagePlugin;

    fn parse_fixture_ir() -> SqlcxIR {
        let schema_sql = include_str!("../../../../../tests/fixtures/schema.sql");
        let queries_sql = include_str!("../../../../../tests/fixtures/queries/users.sql");
        let parser = PostgresParser::new();
        let (tables, enums) = parser.parse_schema(schema_sql).unwrap();
        let queries = parser
            .parse_queries(queries_sql, &tables, &enums, "queries/users.sql")
            .unwrap();
        SqlcxIR { tables, queries, enums }
    }

    #[test]
    fn generates_three_files() {
        let ir = parse_fixture_ir();
        let plugin = GoPlugin::new("structs", "database-sql").unwrap();
        let config = TargetConfig {
            language: "go".to_string(),
            out: "./db".to_string(),
            schema: "structs".to_string(),
            driver: "database-sql".to_string(),
            overrides: HashMap::new(),
        };
        let files = plugin.generate(&ir, &config).unwrap();
        assert_eq!(files.len(), 3);
        assert!(files.iter().any(|f| f.path == "models.go"));
        assert!(files.iter().any(|f| f.path == "client.go"));
        assert!(files.iter().any(|f| f.path == "users.queries.go"));
    }
}
