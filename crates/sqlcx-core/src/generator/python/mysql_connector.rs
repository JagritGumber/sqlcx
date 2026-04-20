// mysql-connector-python driver generator.
//
// Type mapping matches the default (Postgres) Python mapping — MySQL's
// types flow through to Python identically (bool, datetime, bytes, etc.).
// Placeholder style is `%s` (mysql-connector's positional form).

use std::collections::BTreeMap;
use std::path::Path;

use crate::error::Result;
use crate::generator::python::common::{
    DefaultPyTypeMap, escape_sql, generate_params_class, generate_row_class,
};
use crate::generator::{DriverGenerator, GeneratedFile};
use crate::ir::{QueryCommand, QueryDef, SqlcxIR};
use crate::utils::{pascal_case, snake_case};

pub struct MysqlConnectorGenerator;

/// Convert $1, $2, ... placeholders to `%s` for mysql-connector-python.
/// Returns rewritten SQL and the param indices in SQL occurrence order.
fn to_mysql_params(sql: &str) -> (String, Vec<u32>) {
    let mut result = String::with_capacity(sql.len());
    let mut indices = Vec::new();
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' {
            if chars.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                let mut num_str = String::new();
                while chars.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                    num_str.push(chars.next().unwrap());
                }
                result.push_str("%s");
                indices.push(num_str.parse::<u32>().unwrap_or(0));
            } else {
                result.push(c);
            }
        } else {
            result.push(c);
        }
    }
    (result, indices)
}

fn generate_query_function(query: &QueryDef) -> String {
    let fn_name = snake_case(&query.name);
    let row_class = generate_row_class(&DefaultPyTypeMap, query);
    let params_class = generate_params_class(&DefaultPyTypeMap, query);
    let has_params = !query.params.is_empty();
    let params_type_name = format!("{}Params", pascal_case(&query.name));
    let (rewritten_sql, param_indices) = to_mysql_params(&query.sql);
    let sql_const = format!(
        "{}_SQL = \"{}\"",
        fn_name.to_uppercase(),
        escape_sql(&rewritten_sql)
    );

    let params_sig = if has_params {
        format!(", params: {}", params_type_name)
    } else {
        String::new()
    };

    // Build tuple in SQL occurrence order (handles $2 AND $1, $1 OR $1).
    // Single-element tuple needs trailing comma.
    let params_arg = if has_params {
        let args: Vec<String> = param_indices
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
        format!("({}{})", args.join(", "), trailing)
    } else {
        "()".to_string()
    };

    // mysql-connector-python uses a cursor from conn.cursor(), not conn.execute directly.
    // Pattern: `with conn.cursor() as cur: cur.execute(SQL, params); row = cur.fetchone()`
    let (return_type, body) = match query.command {
        QueryCommand::One => {
            let type_name = format!("{}Row", pascal_case(&query.name));
            (
                format!("{} | None", type_name),
                format!(
                    "    cur = conn.cursor()\n    try:\n        cur.execute({}_SQL, {})\n        row = cur.fetchone()\n        if row is None:\n            return None\n        return {}(*row)\n    finally:\n        cur.close()",
                    fn_name.to_uppercase(),
                    params_arg,
                    type_name
                ),
            )
        }
        QueryCommand::Many => {
            let type_name = format!("{}Row", pascal_case(&query.name));
            (
                format!("list[{}]", type_name),
                format!(
                    "    cur = conn.cursor()\n    try:\n        cur.execute({}_SQL, {})\n        return [{}(*row) for row in cur.fetchall()]\n    finally:\n        cur.close()",
                    fn_name.to_uppercase(),
                    params_arg,
                    type_name
                ),
            )
        }
        QueryCommand::Exec => (
            "None".to_string(),
            format!(
                "    cur = conn.cursor()\n    try:\n        cur.execute({}_SQL, {})\n    finally:\n        cur.close()",
                fn_name.to_uppercase(),
                params_arg
            ),
        ),
        QueryCommand::ExecResult => (
            "int".to_string(),
            format!(
                "    cur = conn.cursor()\n    try:\n        cur.execute({}_SQL, {})\n        return cur.rowcount\n    finally:\n        cur.close()",
                fn_name.to_uppercase(),
                params_arg
            ),
        ),
    };

    let mut parts: Vec<String> = Vec::new();
    if !row_class.is_empty() {
        parts.push(row_class);
    }
    if !params_class.is_empty() {
        parts.push(params_class);
    }
    parts.push(sql_const);
    parts.push(format!(
        "def {}(conn: MySQLConnection{}) -> {}:\n{}",
        fn_name, params_sig, return_type, body
    ));

    parts.join("\n\n")
}

impl MysqlConnectorGenerator {
    pub fn generate_client(&self) -> String {
        r#"# Code generated by sqlcx. DO NOT EDIT.
from __future__ import annotations

from mysql.connector.connection import MySQLConnection
"#
        .to_string()
    }

    pub fn generate_query_functions(&self, queries: &[QueryDef]) -> String {
        let header = "# Code generated by sqlcx. DO NOT EDIT.\nfrom __future__ import annotations\n\nfrom dataclasses import dataclass\nfrom typing import Any\nfrom datetime import datetime\nfrom mysql.connector.connection import MySQLConnection";
        let functions: Vec<String> = queries.iter().map(generate_query_function).collect();
        if functions.is_empty() {
            return format!("{header}\n");
        }
        format!("{header}\n\n\n{}", functions.join("\n\n\n"))
    }
}

impl DriverGenerator for MysqlConnectorGenerator {
    fn generate(&self, ir: &SqlcxIR) -> Result<Vec<GeneratedFile>> {
        let mut files = Vec::new();

        files.push(GeneratedFile {
            path: "client.py".to_string(),
            content: self.generate_client(),
        });

        let mut grouped: BTreeMap<String, Vec<&QueryDef>> = BTreeMap::new();
        for query in &ir.queries {
            grouped
                .entry(query.source_file.clone())
                .or_default()
                .push(query);
        }
        for (source_file, queries) in &grouped {
            let basename = Path::new(source_file)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            let owned: Vec<QueryDef> = queries.iter().map(|q| (*q).clone()).collect();
            files.push(GeneratedFile {
                path: format!("{}_queries.py", basename),
                content: self.generate_query_functions(&owned),
            });
        }

        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::*;
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
    fn generates_client_file() {
        let gen_ = MysqlConnectorGenerator;
        let content = gen_.generate_client();
        assert!(content.contains("mysql.connector"));
        assert!(content.contains("MySQLConnection"));
        insta::assert_snapshot!("mysql_connector_client", content);
    }

    #[test]
    fn generates_query_functions() {
        let ir = parse_fixture_ir();
        let gen_ = MysqlConnectorGenerator;
        let content = gen_.generate_query_functions(&ir.queries);
        assert!(content.contains("def get_user"));
        assert!(content.contains("class GetUserRow"));
        assert!(content.contains("GET_USER_SQL"));
        // Uses cursor pattern with try/finally.
        assert!(content.contains("cur = conn.cursor()"));
        assert!(content.contains("cur.close()"));
        insta::assert_snapshot!("mysql_connector_queries", content);
    }

    #[test]
    fn converts_dollar_params_to_percent_s() {
        let (sql, idx) = to_mysql_params("SELECT * FROM users WHERE id = $1");
        assert_eq!(sql, "SELECT * FROM users WHERE id = %s");
        assert_eq!(idx, vec![1]);

        let (sql, idx) = to_mysql_params("INSERT INTO users (a, b) VALUES ($1, $2)");
        assert_eq!(sql, "INSERT INTO users (a, b) VALUES (%s, %s)");
        assert_eq!(idx, vec![1, 2]);

        // Reused
        let (sql, idx) = to_mysql_params("WHERE a = $1 OR b = $1");
        assert_eq!(sql, "WHERE a = %s OR b = %s");
        assert_eq!(idx, vec![1, 1]);

        // Out-of-order
        let (sql, idx) = to_mysql_params("WHERE a = $2 AND b = $1");
        assert_eq!(sql, "WHERE a = %s AND b = %s");
        assert_eq!(idx, vec![2, 1]);
    }
}
