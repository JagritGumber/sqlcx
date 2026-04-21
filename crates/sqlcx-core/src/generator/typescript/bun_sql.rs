// bun-sql driver. Emits queries.ts only — no client wrapper. Functions take
// `sql: SQL` directly (Bun's native SQL instance) and call `sql.unsafe(...)`.

use crate::error::Result;
use crate::generator::typescript::common::{
    BodyCtx, TsDriverShape, TsTypeMap, generate_driver_files,
};
use crate::generator::{DriverGenerator, GeneratedFile};
use crate::ir::{QueryCommand, SqlcxIR};

pub struct BunSqlGenerator;

impl TsTypeMap for BunSqlGenerator {}

impl TsDriverShape for BunSqlGenerator {
    fn imports(&self) -> String {
        "import type { SQL } from \"bun\";".to_string()
    }
    fn connection_type(&self) -> &'static str {
        "SQL"
    }
    fn connection_param(&self) -> &'static str {
        "sql"
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
                    "  const rows = (await sql.unsafe({sc}, {va})) as {rt}[];\n  return rows[0] ?? null;"
                ),
            ),
            QueryCommand::Many => (
                format!("Promise<{rt}[]>"),
                format!("  return (await sql.unsafe({sc}, {va})) as {rt}[];"),
            ),
            QueryCommand::Exec => (
                "Promise<void>".to_string(),
                format!("  await sql.unsafe({sc}, {va});"),
            ),
            QueryCommand::ExecResult => (
                "Promise<{ rowsAffected: number }>".to_string(),
                format!(
                    "  const result = await sql.unsafe({sc}, {va});\n  return {{ rowsAffected: result.count }};"
                ),
            ),
        }
    }
}

impl DriverGenerator for BunSqlGenerator {
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
    fn generates_bun_sql_query_functions() {
        let ir = parse_fixture_ir();
        let content = generate_queries_file(&BunSqlGenerator, &ir.queries);
        assert!(content.contains("import type { SQL } from \"bun\""));
        assert!(content.contains("sql.unsafe"));
        assert!(content.contains("export async function getUser"));
        insta::assert_snapshot!("bun_sql_queries", content);
    }
}
