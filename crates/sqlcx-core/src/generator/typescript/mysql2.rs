// mysql2 driver. Emits queries.ts only — no client wrapper. Functions take
// `pool: Pool` directly and call `pool.execute<...>(sql, values)`. Rewrites
// Postgres-style `$N` placeholders to `?` in occurrence order.

use crate::error::Result;
use crate::generator::typescript::common::{
    BodyCtx, TsDriverShape, TsTypeMap, generate_driver_files,
};
use crate::generator::{DriverGenerator, GeneratedFile};
use crate::ir::{QueryCommand, SqlcxIR};

pub struct Mysql2Generator;

impl TsTypeMap for Mysql2Generator {
    // mysql2 returns Node Buffer for binary columns, not Uint8Array.
    fn binary_ty(&self) -> &'static str {
        "Buffer"
    }
}

fn to_mysql_params(sql: &str) -> (String, Vec<u32>) {
    let mut result = String::with_capacity(sql.len());
    let mut indices = Vec::new();
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' && chars.peek().is_some_and(|ch| ch.is_ascii_digit()) {
            let mut num_str = String::new();
            while chars.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                num_str.push(chars.next().unwrap());
            }
            result.push('?');
            indices.push(num_str.parse::<u32>().unwrap_or(0));
        } else {
            result.push(c);
        }
    }
    (result, indices)
}

impl TsDriverShape for Mysql2Generator {
    fn imports(&self) -> String {
        "import type { Pool, RowDataPacket, ResultSetHeader } from \"mysql2/promise\";".to_string()
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
    fn rewrite_placeholders(&self, sql: &str) -> (String, Vec<u32>) {
        to_mysql_params(sql)
    }
    fn render_body(&self, ctx: &BodyCtx<'_>) -> (String, String) {
        let (sc, rt, va) = (ctx.sql_const, ctx.row_type, ctx.values_arg);
        match ctx.command {
            QueryCommand::One => (
                format!("Promise<{rt} | null>"),
                format!(
                    "  const [rows] = await pool.execute<({rt} & RowDataPacket)[]>({sc}, {va});\n  return rows[0] ?? null;"
                ),
            ),
            QueryCommand::Many => (
                format!("Promise<{rt}[]>"),
                format!(
                    "  const [rows] = await pool.execute<({rt} & RowDataPacket)[]>({sc}, {va});\n  return rows;"
                ),
            ),
            QueryCommand::Exec => (
                "Promise<void>".to_string(),
                format!("  await pool.execute<ResultSetHeader>({sc}, {va});"),
            ),
            QueryCommand::ExecResult => (
                "Promise<{ rowsAffected: number }>".to_string(),
                format!(
                    "  const [result] = await pool.execute<ResultSetHeader>({sc}, {va});\n  return {{ rowsAffected: result.affectedRows }};"
                ),
            ),
        }
    }
}

impl DriverGenerator for Mysql2Generator {
    fn generate(&self, ir: &SqlcxIR) -> Result<Vec<GeneratedFile>> {
        generate_driver_files(self, ir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::typescript::common::generate_queries_file;
    use crate::parser::DatabaseParser;
    use crate::parser::mysql::MySqlParser;

    fn parse_fixture_ir() -> SqlcxIR {
        let schema_sql = include_str!("../../../../../tests/fixtures/mysql_schema.sql");
        let queries_sql = include_str!("../../../../../tests/fixtures/mysql_queries/users.sql");
        let parser = MySqlParser::new();
        let (tables, enums) = parser.parse_schema(schema_sql).unwrap();
        let queries = parser
            .parse_queries(queries_sql, &tables, &enums, "mysql_queries/users.sql")
            .unwrap();
        SqlcxIR {
            tables,
            queries,
            enums,
        }
    }

    #[test]
    fn generates_mysql2_query_functions() {
        let ir = parse_fixture_ir();
        let content = generate_queries_file(&Mysql2Generator, &ir.queries);
        assert!(content.contains("import type { Pool, RowDataPacket, ResultSetHeader }"));
        assert!(content.contains("pool.execute"));
        assert!(!content.contains("$1"));
        insta::assert_snapshot!("mysql2_queries", content);
    }

    #[test]
    fn rewrites_dollar_n_to_question_mark() {
        let (sql, idx) = to_mysql_params("WHERE a = $1 AND b = $2 OR a = $1");
        assert_eq!(sql, "WHERE a = ? AND b = ? OR a = ?");
        assert_eq!(idx, vec![1, 2, 1]);
    }
}
