// mysql-connector-python driver. Emits queries.py only. %s positional
// placeholders (with literal % escaped to %%), tuple params arg, sync
// functions against MySQLConnection using the explicit cursor pattern.

use crate::error::Result;
use crate::generator::python::common::{
    PyBodyCtx, PyDriverShape, PyTypeMap, generate_driver_files,
};
use crate::generator::{DriverGenerator, GeneratedFile};
use crate::ir::{QueryCommand, QueryDef, SqlcxIR};

pub struct MysqlConnectorGenerator;

impl PyTypeMap for MysqlConnectorGenerator {}

/// Rewrite placeholders to mysql-connector's `%s` positional form, escape
/// literal `%` to `%%`, and return the param indices in occurrence order.
/// Accepts Postgres-style `$N` (stored by the PG parser) and native `?`
/// (stored by the MySQL/SQLite parsers) — for `?` the occurrence index is
/// the 1-based count, matching the MySQL parser's `extract_param_indices`.
fn rewrite_mysql(sql: &str) -> (String, Vec<u32>) {
    let mut result = String::with_capacity(sql.len());
    let mut indices = Vec::new();
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            result.push_str("%%");
        } else if c == '$' && chars.peek().is_some_and(|ch| ch.is_ascii_digit()) {
            let mut num_str = String::new();
            while chars.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                num_str.push(chars.next().unwrap());
            }
            result.push_str("%s");
            indices.push(num_str.parse::<u32>().unwrap_or(0));
        } else if c == '?' {
            result.push_str("%s");
            indices.push(indices.len() as u32 + 1);
        } else {
            result.push(c);
        }
    }
    (result, indices)
}

impl PyDriverShape for MysqlConnectorGenerator {
    fn driver_import(&self) -> &'static str {
        "from mysql.connector.connection import MySQLConnection"
    }
    fn connection_type(&self) -> &'static str {
        "MySQLConnection"
    }
    fn is_async(&self) -> bool {
        false
    }
    fn rewrite_sql(&self, query: &QueryDef) -> String {
        rewrite_mysql(&query.sql).0
    }
    fn build_params_arg(&self, query: &QueryDef) -> String {
        if query.params.is_empty() {
            return "()".to_string();
        }
        let indices = rewrite_mysql(&query.sql).1;
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
                    "    cur = conn.cursor()\n    try:\n        cur.execute({sc}, {pa})\n        row = cur.fetchone()\n        if row is None:\n            return None\n        return {rt}(*row)\n    finally:\n        cur.close()"
                ),
            ),
            QueryCommand::Many => (
                format!("list[{rt}]"),
                format!(
                    "    cur = conn.cursor()\n    try:\n        cur.execute({sc}, {pa})\n        return [{rt}(*row) for row in cur.fetchall()]\n    finally:\n        cur.close()"
                ),
            ),
            QueryCommand::Exec => (
                "None".to_string(),
                format!(
                    "    cur = conn.cursor()\n    try:\n        cur.execute({sc}, {pa})\n    finally:\n        cur.close()"
                ),
            ),
            QueryCommand::ExecResult => (
                "int".to_string(),
                format!(
                    "    cur = conn.cursor()\n    try:\n        cur.execute({sc}, {pa})\n        return cur.rowcount\n    finally:\n        cur.close()"
                ),
            ),
        }
    }
}

impl DriverGenerator for MysqlConnectorGenerator {
    fn generate(&self, ir: &SqlcxIR) -> Result<Vec<GeneratedFile>> {
        generate_driver_files(self, ir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::python::common::generate_queries_file;
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
    fn generates_mysql_connector_query_functions() {
        let ir = parse_fixture_ir();
        let content = generate_queries_file(&MysqlConnectorGenerator, &ir.queries);
        assert!(content.contains("from mysql.connector.connection import MySQLConnection"));
        assert!(content.contains("conn.cursor()"));
        assert!(!content.contains("$1"));
        insta::assert_snapshot!("mysql_connector_queries", content);
    }

    #[test]
    fn escapes_literal_percent() {
        let (sql, _) = rewrite_mysql("WHERE name LIKE '%foo%' AND id = $1");
        assert_eq!(sql, "WHERE name LIKE '%%foo%%' AND id = %s");
    }

    #[test]
    fn native_qmark_input_tracks_occurrence_indices() {
        // MySQL/SQLite parsers store SQL with native `?` placeholders.
        // rewrite_mysql must still emit `%s` and 1-based occurrence indices
        // so build_params_arg doesn't produce an empty tuple.
        let (sql, idx) = rewrite_mysql("WHERE a = ? AND b = ?");
        assert_eq!(sql, "WHERE a = %s AND b = %s");
        assert_eq!(idx, vec![1, 2]);
    }
}
