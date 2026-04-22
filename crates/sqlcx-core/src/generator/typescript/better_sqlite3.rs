// better-sqlite3 driver (synchronous). Emits queries.ts only. Functions take
// `db: Database.Database` directly (instance type via default-export's
// namespace side) and call `db.prepare(sql).get/all/run(...spread)`.

use crate::error::Result;
use crate::generator::typescript::common::{
    BodyCtx, TsDriverShape, TsTypeMap, generate_driver_files,
};
use crate::generator::{DriverGenerator, GeneratedFile};
use crate::ir::{QueryCommand, SqlcxIR};

pub struct BetterSqlite3Generator;

// SQLite has no native boolean/date/json types; better-sqlite3 surfaces them
// as number/text and returns Buffer for blobs.
impl TsTypeMap for BetterSqlite3Generator {
    fn boolean_ty(&self) -> &'static str {
        "number"
    }
    fn date_ty(&self) -> &'static str {
        "string"
    }
    fn json_ty(&self) -> &'static str {
        "string"
    }
    fn binary_ty(&self) -> &'static str {
        "Buffer"
    }
}

fn to_sqlite_params(sql: &str) -> (String, Vec<u32>) {
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

impl TsDriverShape for BetterSqlite3Generator {
    fn imports(&self) -> String {
        "import type Database from \"better-sqlite3\";".to_string()
    }
    fn connection_type(&self) -> &'static str {
        "Database.Database"
    }
    fn connection_param(&self) -> &'static str {
        "db"
    }
    fn is_async(&self) -> bool {
        false
    }
    fn rewrite_placeholders(&self, sql: &str) -> (String, Vec<u32>) {
        to_sqlite_params(sql)
    }
    fn render_body(&self, ctx: &BodyCtx<'_>) -> (String, String) {
        let (sc, rt, vs) = (ctx.sql_const, ctx.row_type, ctx.values_spread);
        match ctx.command {
            QueryCommand::One => (
                format!("{rt} | undefined"),
                format!("  return db.prepare({sc}).get({vs}) as {rt} | undefined;"),
            ),
            QueryCommand::Many => (
                format!("{rt}[]"),
                format!("  return db.prepare({sc}).all({vs}) as {rt}[];"),
            ),
            QueryCommand::Exec => ("void".to_string(), format!("  db.prepare({sc}).run({vs});")),
            QueryCommand::ExecResult => (
                "{ changes: number }".to_string(),
                format!(
                    "  const result = db.prepare({sc}).run({vs});\n  return {{ changes: result.changes }};"
                ),
            ),
        }
    }
}

impl DriverGenerator for BetterSqlite3Generator {
    fn generate(&self, ir: &SqlcxIR) -> Result<Vec<GeneratedFile>> {
        generate_driver_files(self, ir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::typescript::common::generate_queries_file;
    use crate::parser::DatabaseParser;
    use crate::parser::sqlite::SqliteParser;

    fn parse_fixture_ir() -> SqlcxIR {
        let schema_sql = include_str!("../../../../../tests/fixtures/sqlite_schema.sql");
        let queries_sql = include_str!("../../../../../tests/fixtures/sqlite_queries/users.sql");
        let parser = SqliteParser::new();
        let (tables, enums) = parser.parse_schema(schema_sql).unwrap();
        let queries = parser
            .parse_queries(queries_sql, &tables, &enums, "sqlite_queries/users.sql")
            .unwrap();
        SqlcxIR {
            tables,
            queries,
            enums,
        }
    }

    #[test]
    fn generates_better_sqlite3_query_functions() {
        let ir = parse_fixture_ir();
        let content = generate_queries_file(&BetterSqlite3Generator, &ir.queries);
        assert!(content.contains("import type Database from \"better-sqlite3\""));
        assert!(content.contains("Database.Database"));
        assert!(content.contains("db.prepare"));
        insta::assert_snapshot!("better_sqlite3_queries", content);
    }

    #[test]
    fn rewrites_dollar_n_to_question_mark() {
        let (sql, idx) = to_sqlite_params("WHERE a = $1 AND b = $2");
        assert_eq!(sql, "WHERE a = ? AND b = ?");
        assert_eq!(idx, vec![1, 2]);
    }
}
