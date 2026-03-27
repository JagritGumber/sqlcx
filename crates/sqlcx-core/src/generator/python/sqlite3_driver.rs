// sqlite3 (Python stdlib) driver generator for Python

use std::collections::BTreeMap;
use std::path::Path;

use crate::error::Result;
use crate::generator::{DriverGenerator, GeneratedFile};
use crate::ir::{QueryCommand, QueryDef, SqlType, SqlTypeCategory, SqlcxIR};
use crate::utils::{pascal_case, snake_case};

pub struct Sqlite3Generator;

fn py_type(sql_type: &SqlType) -> String {
    if let Some(elem) = &sql_type.element_type {
        return format!("list[{}]", py_type(elem));
    }
    match sql_type.category {
        SqlTypeCategory::String | SqlTypeCategory::Uuid | SqlTypeCategory::Enum => {
            "str".to_string()
        }
        SqlTypeCategory::Number => {
            let upper = sql_type.raw.to_uppercase();
            if upper.contains("REAL")
                || upper.contains("FLOAT")
                || upper.contains("DOUBLE")
                || upper.contains("DECIMAL")
                || upper.contains("NUMERIC")
            {
                "float".to_string()
            } else {
                "int".to_string()
            }
        }
        SqlTypeCategory::Boolean => "int".to_string(), // SQLite uses 0/1
        SqlTypeCategory::Date => "str".to_string(),    // SQLite stores as text
        SqlTypeCategory::Json => "str".to_string(),    // SQLite stores as text
        SqlTypeCategory::Binary => "bytes".to_string(),
        SqlTypeCategory::Unknown => "Any".to_string(),
    }
}

/// Convert $1, $2, ... placeholders to ? for SQLite.
/// Returns the converted SQL and the param indices in occurrence order.
fn to_sqlite_params(sql: &str) -> (String, Vec<u32>) {
    let mut result = String::with_capacity(sql.len());
    let mut indices = Vec::new();
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' {
            if chars.peek().map_or(false, |ch| ch.is_ascii_digit()) {
                let mut num_str = String::new();
                while chars.peek().map_or(false, |ch| ch.is_ascii_digit()) {
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

fn generate_row_class(query: &QueryDef) -> String {
    if query.returns.is_empty() {
        return String::new();
    }
    let class_name = format!("{}Row", pascal_case(&query.name));
    let fields: Vec<String> = query
        .returns
        .iter()
        .map(|col| {
            let name = col.alias.as_deref().unwrap_or(&col.name);
            let ty = py_type(&col.sql_type);
            if col.nullable {
                format!("    {}: {} | None", name, ty)
            } else {
                format!("    {}: {}", name, ty)
            }
        })
        .collect();
    format!("@dataclass\nclass {}:\n{}", class_name, fields.join("\n"))
}

fn generate_params_class(query: &QueryDef) -> String {
    if query.params.is_empty() {
        return String::new();
    }
    let class_name = format!("{}Params", pascal_case(&query.name));
    let fields: Vec<String> = query
        .params
        .iter()
        .map(|p| format!("    {}: {}", p.name, py_type(&p.sql_type)))
        .collect();
    format!("@dataclass\nclass {}:\n{}", class_name, fields.join("\n"))
}

fn escape_sql_sqlite(s: &str) -> (String, Vec<u32>) {
    let (sqlite_sql, indices) = to_sqlite_params(s);
    let escaped = sqlite_sql
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    (escaped, indices)
}

fn generate_query_function(query: &QueryDef) -> String {
    let fn_name = snake_case(&query.name);
    let row_class = generate_row_class(query);
    let params_class = generate_params_class(query);
    let has_params = !query.params.is_empty();
    let params_type_name = format!("{}Params", pascal_case(&query.name));
    let (escaped_sql, param_indices) = escape_sql_sqlite(&query.sql);
    let sql_const = format!("{}_SQL = \"{}\"", fn_name.to_uppercase(), escaped_sql);

    let params_sig = if has_params {
        format!(", params: {}", params_type_name)
    } else {
        String::new()
    };

    // Build tuple in SQL occurrence order (handles $2 AND $1, $1 OR $1)
    let params_arg = if has_params {
        let args: Vec<String> = param_indices
            .iter()
            .map(|idx| {
                query.params
                    .iter()
                    .find(|p| p.index == *idx)
                    .map(|p| format!("params.{}", p.name))
                    .unwrap_or_else(|| "None".to_string())
            })
            .collect();
        format!("({}{})", args.join(", "), if args.len() == 1 { "," } else { "" })
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
                    fn_name.to_uppercase(), params_arg, type_name
                ),
            )
        }
        QueryCommand::Many => {
            let type_name = format!("{}Row", pascal_case(&query.name));
            (
                format!("list[{}]", type_name),
                format!(
                    "    cur = conn.execute({}_SQL, {})\n    return [{}(*row) for row in cur.fetchall()]",
                    fn_name.to_uppercase(), params_arg, type_name
                ),
            )
        }
        QueryCommand::Exec => (
            "None".to_string(),
            format!(
                "    conn.execute({}_SQL, {})",
                fn_name.to_uppercase(), params_arg
            ),
        ),
        QueryCommand::ExecResult => (
            "int".to_string(),
            format!(
                "    cur = conn.execute({}_SQL, {})\n    return cur.rowcount",
                fn_name.to_uppercase(), params_arg
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
            grouped.entry(query.source_file.clone()).or_default().push(query);
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
    use crate::parser::postgres::PostgresParser;
    use crate::parser::DatabaseParser;

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
    fn generates_client_file() {
        let gen = Sqlite3Generator;
        let content = gen.generate_client();
        assert!(content.contains("sqlite3"));
        assert!(content.contains("DO NOT EDIT"));
        insta::assert_snapshot!("sqlite3_client", content);
    }

    #[test]
    fn generates_query_functions() {
        let ir = parse_fixture_ir();
        let gen = Sqlite3Generator;
        let content = gen.generate_query_functions(&ir.queries);
        assert!(content.contains("def get_user"));
        assert!(content.contains("class GetUserRow"));
        assert!(!content.contains("async"));
        insta::assert_snapshot!("sqlite3_queries", content);
    }

    #[test]
    fn converts_dollar_params_to_question_marks() {
        let (sql, idx) = to_sqlite_params("SELECT * FROM users WHERE id = $1");
        assert_eq!(sql, "SELECT * FROM users WHERE id = ?");
        assert_eq!(idx, vec![1]);

        let (sql, idx) = to_sqlite_params("WHERE a = $1 OR b = $1");
        assert_eq!(sql, "WHERE a = ? OR b = ?");
        assert_eq!(idx, vec![1, 1]);

        let (sql, idx) = to_sqlite_params("WHERE a = $2 AND b = $1");
        assert_eq!(sql, "WHERE a = ? AND b = ?");
        assert_eq!(idx, vec![2, 1]);
    }
}
