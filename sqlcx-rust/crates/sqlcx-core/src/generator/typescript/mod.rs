pub mod typebox;
pub mod bun_sql;

use std::collections::HashMap;
use std::path::Path;

use crate::error::Result;
use crate::config::TargetConfig;
use crate::generator::{GeneratedFile, LanguagePlugin};
use crate::ir::{QueryDef, SqlcxIR};

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

fn join_path(base: &str, filename: &str) -> String {
    if base.ends_with('/') {
        format!("{}{}", base, filename)
    } else {
        format!("{}/{}", base, filename)
    }
}

impl LanguagePlugin for TypeScriptPlugin {
    fn generate(&self, ir: &SqlcxIR, config: &TargetConfig) -> Result<Vec<GeneratedFile>> {
        let mut files = Vec::new();
        let overrides = HashMap::new();

        // 1. Generate schema.ts
        let typebox = TypeBoxGenerator;
        let schema_content = typebox.generate_schema_file(ir, &overrides);
        files.push(GeneratedFile {
            path: join_path(&config.out, "schema.ts"),
            content: schema_content,
        });

        // 2. Generate client.ts
        let bun_sql = BunSqlGenerator;
        let client_content = bun_sql.generate_client();
        files.push(GeneratedFile {
            path: join_path(&config.out, "client.ts"),
            content: client_content,
        });

        // 3. Generate query files — group queries by source_file
        let mut grouped: HashMap<String, Vec<&QueryDef>> = HashMap::new();
        for query in &ir.queries {
            grouped.entry(query.source_file.clone()).or_default().push(query);
        }

        for (source_file, queries) in &grouped {
            let basename = Path::new(source_file)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            let filename = format!("{}.queries.ts", basename);

            let owned_queries: Vec<QueryDef> = queries.iter().map(|q| (*q).clone()).collect();
            let content = bun_sql.generate_query_functions(&owned_queries);

            files.push(GeneratedFile {
                path: join_path(&config.out, &filename),
                content,
            });
        }

        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        };
        let files = plugin.generate(&ir, &config).unwrap();
        assert_eq!(files.len(), 3);
        assert!(files.iter().any(|f| f.path.ends_with("schema.ts")));
        assert!(files.iter().any(|f| f.path.ends_with("client.ts")));
        assert!(files.iter().any(|f| f.path.ends_with("users.queries.ts")));
    }
}
