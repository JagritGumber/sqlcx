pub mod typebox;
pub mod bun_sql;

use crate::error::{Result, SqlcxError};
use crate::config::TargetConfig;
use crate::generator::{GeneratedFile, LanguagePlugin, SchemaGenerator, DriverGenerator};
use crate::ir::SqlcxIR;

use self::typebox::TypeBoxGenerator;
use self::bun_sql::BunSqlGenerator;

pub struct TypeScriptPlugin {
    pub schema_name: String,
    pub driver_name: String,
}

impl TypeScriptPlugin {
    pub fn new(schema: &str, driver: &str) -> Result<Self> {
        Ok(Self {
            schema_name: schema.to_string(),
            driver_name: driver.to_string(),
        })
    }
}

fn resolve_schema(name: &str) -> Result<Box<dyn SchemaGenerator>> {
    match name {
        "typebox" => Ok(Box::new(TypeBoxGenerator)),
        _ => Err(SqlcxError::UnknownSchema(name.to_string())),
    }
}

fn resolve_driver(name: &str) -> Result<Box<dyn DriverGenerator>> {
    match name {
        "bun-sql" => Ok(Box::new(BunSqlGenerator)),
        _ => Err(SqlcxError::UnknownDriver(name.to_string())),
    }
}

fn join_path(base: &str, filename: &str) -> String {
    if base.ends_with('/') {
        format!("{}{}", base, filename)
    } else {
        format!("{}/{}", base, filename)
    }
}

impl LanguagePlugin for TypeScriptPlugin {
    fn generate(&self, ir: &SqlcxIR, config: &TargetConfig) -> Result<Vec<GeneratedFile>> {
        let schema_gen = resolve_schema(&self.schema_name)?;
        let driver_gen = resolve_driver(&self.driver_name)?;
        let overrides = &config.overrides;

        let mut files = Vec::new();

        // Schema file
        let mut schema_file = schema_gen.generate(ir, overrides)?;
        schema_file.path = join_path(&config.out, &schema_file.path);
        files.push(schema_file);

        // Driver files (client.ts + *.queries.ts)
        let driver_files = driver_gen.generate(ir)?;
        for mut f in driver_files {
            f.path = join_path(&config.out, &f.path);
            files.push(f);
        }

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
        let plugin = TypeScriptPlugin::new("typebox", "bun-sql").unwrap();
        let config = TargetConfig {
            language: "typescript".to_string(),
            out: "./src/db".to_string(),
            schema: "typebox".to_string(),
            driver: "bun-sql".to_string(),
            overrides: HashMap::new(),
        };
        let files = plugin.generate(&ir, &config).unwrap();
        assert_eq!(files.len(), 3);
        assert!(files.iter().any(|f| f.path.ends_with("schema.ts")));
        assert!(files.iter().any(|f| f.path.ends_with("client.ts")));
        assert!(files.iter().any(|f| f.path.ends_with("users.queries.ts")));
    }
}
