// psycopg (psycopg3) driver generator for Python

use std::collections::BTreeMap;
use std::path::Path;

use crate::error::Result;
use crate::generator::{DriverGenerator, GeneratedFile};
use crate::ir::{QueryCommand, QueryDef, SqlType, SqlTypeCategory, SqlcxIR};
use crate::utils::{pascal_case, snake_case};

pub struct PsycopgGenerator;

// ── Type mapping ──────────────────────────────────────────────────────────────

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
        SqlTypeCategory::Boolean => "bool".to_string(),
        SqlTypeCategory::Date => "datetime".to_string(),
        SqlTypeCategory::Json => "Any".to_string(),
        SqlTypeCategory::Binary => "bytes".to_string(),
        SqlTypeCategory::Unknown => "Any".to_string(),
    }
}

// ── Per-query generators ─────────────────────────────────────────────────────

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

/// Convert $1, $2, ... placeholders to %s for psycopg3
fn to_psycopg_params(sql: &str) -> String {
    let mut result = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' {
            if chars.peek().map_or(false, |ch| ch.is_ascii_digit()) {
                result.push_str("%s");
                while chars.peek().map_or(false, |ch| ch.is_ascii_digit()) {
                    chars.next();
                }
            } else {
                result.push(c);
            }
        } else {
            result.push(c);
        }
    }
    result
}

fn escape_sql(s: &str) -> String {
    let converted = to_psycopg_params(s);
    converted
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn generate_query_function(query: &QueryDef) -> String {
    let fn_name = snake_case(&query.name);
    let row_class = generate_row_class(query);
    let params_class = generate_params_class(query);
    let has_params = !query.params.is_empty();
    let params_type_name = format!("{}Params", pascal_case(&query.name));
    let sql_const = format!("{}_SQL = \"{}\"", fn_name.to_uppercase(), escape_sql(&query.sql));

    let params_sig = if has_params {
        format!(", params: {}", params_type_name)
    } else {
        String::new()
    };

    let params_arg = if has_params {
        let args: Vec<String> = query
            .params
            .iter()
            .map(|p| format!("params.{}", p.name))
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

// ── Public API ────────────────────────────────────────────────────────────────

impl PsycopgGenerator {
    pub fn generate_client(&self) -> String {
        r#"# Code generated by sqlcx. DO NOT EDIT.
from __future__ import annotations

from psycopg import Connection
"#
        .to_string()
    }

    pub fn generate_query_functions(&self, queries: &[QueryDef]) -> String {
        let header = "# Code generated by sqlcx. DO NOT EDIT.\nfrom __future__ import annotations\n\nfrom dataclasses import dataclass\nfrom typing import Any\nfrom datetime import datetime\nfrom psycopg import Connection";
        let functions: Vec<String> = queries.iter().map(generate_query_function).collect();
        if functions.is_empty() {
            return format!("{header}\n");
        }
        format!("{header}\n\n\n{}", functions.join("\n\n\n"))
    }
}

impl DriverGenerator for PsycopgGenerator {
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
        let gen = PsycopgGenerator;
        let content = gen.generate_client();
        assert!(content.contains("psycopg"));
        assert!(content.contains("DO NOT EDIT"));
        insta::assert_snapshot!("psycopg_client", content);
    }

    #[test]
    fn generates_query_functions() {
        let ir = parse_fixture_ir();
        let gen = PsycopgGenerator;
        let content = gen.generate_query_functions(&ir.queries);
        assert!(content.contains("def get_user"));
        assert!(content.contains("class GetUserRow"));
        assert!(content.contains("GET_USER_SQL"));
        insta::assert_snapshot!("psycopg_queries", content);
    }
}
