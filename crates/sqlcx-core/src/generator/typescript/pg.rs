// pg (node-postgres) driver. Emits queries.ts only — no client wrapper.
// Functions take `pool: Pool` directly and call `pool.query<Row>(...)`.

use crate::error::Result;
use crate::generator::typescript::common::{
    BodyCtx, TsDriverShape, TsTypeMap, generate_driver_files,
};
use crate::generator::{DriverGenerator, GeneratedFile};
use crate::ir::{QueryCommand, SqlcxIR};

pub struct PgGenerator;

impl TsTypeMap for PgGenerator {}

impl TsDriverShape for PgGenerator {
    fn imports(&self) -> String {
        "import type { Pool } from \"pg\";".to_string()
    }
    fn connection_type(&self) -> &'static str {
        "Pool"
    }
    fn connection_param(&self) -> &'static str {
        "pool"
    }
    fn is_async(&self) -> bool {
        true
    }
    fn render_body(&self, ctx: &BodyCtx<'_>) -> (String, String) {
        let (sc, rt, va) = (ctx.sql_const, ctx.row_type, ctx.values_arg);
        match ctx.command {
            QueryCommand::One => (
                format!("Promise<{rt} | null>"),
                format!(
                    "  const result = await pool.query<{rt}>({sc}, {va});\n  return result.rows[0] ?? null;"
                ),
            ),
            QueryCommand::Many => (
                format!("Promise<{rt}[]>"),
                format!(
                    "  const result = await pool.query<{rt}>({sc}, {va});\n  return result.rows;"
                ),
            ),
            QueryCommand::Exec => (
                "Promise<void>".to_string(),
                format!("  await pool.query({sc}, {va});"),
            ),
            QueryCommand::ExecResult => (
                "Promise<{ rowsAffected: number }>".to_string(),
                format!(
                    "  const result = await pool.query({sc}, {va});\n  return {{ rowsAffected: result.rowCount ?? 0 }};"
                ),
            ),
        }
    }
}

impl DriverGenerator for PgGenerator {
    fn generate(&self, ir: &SqlcxIR) -> Result<Vec<GeneratedFile>> {
        generate_driver_files(self, ir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::typescript::common::generate_queries_file;
    use crate::parser::DatabaseParser;
    use crate::parser::postgres::PostgresParser;

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
    fn generates_pg_query_functions() {
        let ir = parse_fixture_ir();
        let content = generate_queries_file(&PgGenerator, &ir.queries);
        assert!(content.contains("import type { Pool } from \"pg\""));
        assert!(content.contains("pool.query"));
        assert!(content.contains("export async function getUser"));
        insta::assert_snapshot!("pg_queries", content);
    }
}
