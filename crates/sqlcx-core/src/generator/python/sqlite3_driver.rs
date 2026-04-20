// sqlite3 (Python stdlib) driver generator.
//
// SQLite doesn't have native Boolean/Date/Json types — values come back
// as int, str, str respectively. Sqlite3PyTypeMap overrides those
// methods; everything else inherits from the default Python mapping.

use std::collections::BTreeMap;
use std::path::Path;

use crate::error::Result;
use crate::generator::python::common::{
    PyTypeMap, escape_sql, generate_params_class, generate_row_class,
};
use crate::generator::{DriverGenerator, GeneratedFile};
use crate::ir::{QueryCommand, QueryDef, SqlcxIR};
use crate::utils::{pascal_case, snake_case};

pub struct Sqlite3Generator;

struct Sqlite3PyTypeMap;
impl PyTypeMap for Sqlite3PyTypeMap {
    fn boolean_ty(&self) -> &'static str {
        // SQLite stores booleans as 0/1 integers.
        "int"
    }
    fn date_ty(&self) -> &'static str {
        // SQLite stores dates as ISO 8601 text.
        "str"
    }
    fn json_ty(&self) -> &'static str {
        // SQLite stores JSON as text; callers json.loads() to decode.
        "str"
    }
}

/// Convert $1, $2, ... placeholders to `?` for SQLite.
/// Returns the rewritten SQL and the param indices in SQL occurrence order
/// (handles reused params like `$1 OR $1` and out-of-order like `$2 AND $1`).
fn to_sqlite_params(sql: &str) -> (String, Vec<u32>) {
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
                result.push('?');
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
    let row_class = generate_row_class(&Sqlite3PyTypeMap, query);
    let params_class = generate_params_class(&Sqlite3PyTypeMap, query);
    let has_params = !query.params.is_empty();
    let params_type_name = format!("{}Params", pascal_case(&query.name));
    let (rewritten_sql, param_indices) = to_sqlite_params(&query.sql);
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
    // Single-element tuple needs a trailing comma: `(x,)`.
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

    let (return_type, body) = match query.command {
        QueryCommand::One => {
            let type_name = format!("{}Row", pascal_case(&query.name));
            (
                format!("{} | None", type_name),
                format!(
                    "    cur = conn.execute({}_SQL, {})\n    row = cur.fetchone()\n    if row is None:\n        return None\n    return {}(*row)",
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
                    "    cur = conn.execute({}_SQL, {})\n    return [{}(*row) for row in cur.fetchall()]",
                    fn_name.to_uppercase(),
                    params_arg,
                    type_name
                ),
            )
        }
        QueryCommand::Exec => (
            "None".to_string(),
            format!(
                "    conn.execute({}_SQL, {})",
                fn_name.to_uppercase(),
                params_arg
            ),
        ),
        QueryCommand::ExecResult => (
            "int".to_string(),
            format!(
                "    cur = conn.execute({}_SQL, {})\n    return cur.rowcount",
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
        "def {}(conn: Connection{}) -> {}:\n{}",
        fn_name, params_sig, return_type, body
    ));

    parts.join("\n\n")
}

impl Sqlite3Generator {
    pub fn generate_client(&self) -> String {
        r#"# Code generated by sqlcx. DO NOT EDIT.
from __future__ import annotations

from sqlite3 import Connection
"#
        .to_string()
    }

    pub fn generate_query_functions(&self, queries: &[QueryDef]) -> String {
        let header = "# Code generated by sqlcx. DO NOT EDIT.\nfrom __future__ import annotations\n\nfrom dataclasses import dataclass\nfrom typing import Any\nfrom sqlite3 import Connection";
        let functions: Vec<String> = queries.iter().map(generate_query_function).collect();
        if functions.is_empty() {
            return format!("{header}\n");
        }
        format!("{header}\n\n\n{}", functions.join("\n\n\n"))
    }
}

impl DriverGenerator for Sqlite3Generator {
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
        let gen_ = Sqlite3Generator;
        let content = gen_.generate_client();
        assert!(content.contains("sqlite3"));
        assert!(content.contains("DO NOT EDIT"));
        insta::assert_snapshot!("sqlite3_client", content);
    }

    #[test]
    fn generates_query_functions() {
        let ir = parse_fixture_ir();
        let gen_ = Sqlite3Generator;
        let content = gen_.generate_query_functions(&ir.queries);
        // SQLite uses `?` placeholders, not `%(name)s` or `$1`.
        assert!(content.contains("def get_user"));
        assert!(content.contains("class GetUserRow"));
        assert!(content.contains("GET_USER_SQL"));
        assert!(content.contains("conn.execute"));
        insta::assert_snapshot!("sqlite3_queries", content);
    }

    #[test]
    fn converts_dollar_params_to_question_marks() {
        let (sql, idx) = to_sqlite_params("SELECT * FROM users WHERE id = $1");
        assert_eq!(sql, "SELECT * FROM users WHERE id = ?");
        assert_eq!(idx, vec![1]);

        let (sql, idx) = to_sqlite_params("INSERT INTO users (a, b) VALUES ($1, $2)");
        assert_eq!(sql, "INSERT INTO users (a, b) VALUES (?, ?)");
        assert_eq!(idx, vec![1, 2]);

        // Reused
        let (sql, idx) = to_sqlite_params("WHERE a = $1 OR b = $1");
        assert_eq!(sql, "WHERE a = ? OR b = ?");
        assert_eq!(idx, vec![1, 1]);

        // Out-of-order
        let (sql, idx) = to_sqlite_params("WHERE a = $2 AND b = $1");
        assert_eq!(sql, "WHERE a = ? AND b = ?");
        assert_eq!(idx, vec![2, 1]);
    }
}
