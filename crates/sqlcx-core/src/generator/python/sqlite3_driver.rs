// sqlite3 (Python stdlib) driver. Emits queries.py only. ? positional
// placeholders, tuple params arg, sync functions against sqlite3.Connection.
// SQLite lacks native Boolean/Date/Json — the type map surfaces them as
// int/str/str and binary as bytes.

use crate::error::Result;
use crate::generator::python::common::{
    PyBodyCtx, PyDriverShape, PyTypeMap, generate_driver_files,
};
use crate::generator::{DriverGenerator, GeneratedFile};
use crate::ir::{QueryCommand, QueryDef, SqlcxIR};

pub struct Sqlite3Generator;

impl PyTypeMap for Sqlite3Generator {
    fn boolean_ty(&self) -> &'static str {
        "int"
    }
    fn date_ty(&self) -> &'static str {
        "str"
    }
    fn json_ty(&self) -> &'static str {
        "str"
    }
}

fn rewrite_qmark(sql: &str) -> (String, Vec<u32>) {
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

impl PyDriverShape for Sqlite3Generator {
    fn driver_import(&self) -> &'static str {
        "from sqlite3 import Connection"
    }
    fn connection_type(&self) -> &'static str {
        "Connection"
    }
    fn is_async(&self) -> bool {
        false
    }
    fn rewrite_sql(&self, query: &QueryDef) -> String {
        rewrite_qmark(&query.sql).0
    }
    fn build_params_arg(&self, query: &QueryDef) -> String {
        if query.params.is_empty() {
            return "()".to_string();
        }
        let indices = rewrite_qmark(&query.sql).1;
        let args: Vec<String> = indices
            .iter()
            .map(|idx| {
                query
                    .params
                    .iter()
                    .find(|p| p.index == *idx)
                    .map(|p| format!("params.{}", p.name))
                    .unwrap_or_else(|| "None".to_string())
            })
            .collect();
        let trailing = if args.len() == 1 { "," } else { "" };
        format!("({}{trailing})", args.join(", "))
    }
    fn render_body(&self, ctx: &PyBodyCtx<'_>) -> (String, String) {
        let (sc, rt, pa) = (ctx.sql_const, ctx.row_type, ctx.params_arg);
        match ctx.command {
            QueryCommand::One => (
                format!("{rt} | None"),
                format!(
                    "    cur = conn.execute({sc}, {pa})\n    row = cur.fetchone()\n    if row is None:\n        return None\n    return {rt}(*row)"
                ),
            ),
            QueryCommand::Many => (
                format!("list[{rt}]"),
                format!(
                    "    cur = conn.execute({sc}, {pa})\n    return [{rt}(*row) for row in cur.fetchall()]"
                ),
            ),
            QueryCommand::Exec => ("None".to_string(), format!("    conn.execute({sc}, {pa})")),
            QueryCommand::ExecResult => (
                "int".to_string(),
                format!("    cur = conn.execute({sc}, {pa})\n    return cur.rowcount"),
            ),
        }
    }
}

impl DriverGenerator for Sqlite3Generator {
    fn generate(&self, ir: &SqlcxIR) -> Result<Vec<GeneratedFile>> {
        generate_driver_files(self, ir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::python::common::generate_queries_file;
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
    fn generates_sqlite3_query_functions() {
        let ir = parse_fixture_ir();
        let content = generate_queries_file(&Sqlite3Generator, &ir.queries);
        assert!(content.contains("from sqlite3 import Connection"));
        assert!(content.contains("def get_user"));
        assert!(!content.contains("$1"));
        insta::assert_snapshot!("sqlite3_queries", content);
    }
}
