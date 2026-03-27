// better-sqlite3 driver generator for TypeScript (synchronous SQLite)

use std::collections::BTreeMap;
use std::path::Path;

use crate::error::Result;
use crate::generator::{DriverGenerator, GeneratedFile};
use crate::ir::{QueryCommand, QueryDef, SqlType, SqlTypeCategory, SqlcxIR};
use crate::utils::{camel_case, pascal_case};

pub struct BetterSqlite3Generator;

fn ts_type(sql_type: &SqlType) -> String {
    if let Some(elem) = &sql_type.element_type {
        return format!("{}[]", ts_type(elem));
    }
    match sql_type.category {
        SqlTypeCategory::String | SqlTypeCategory::Uuid | SqlTypeCategory::Enum => "string".to_string(),
        SqlTypeCategory::Number => "number".to_string(),
        SqlTypeCategory::Boolean => "number".to_string(), // SQLite uses 0/1
        SqlTypeCategory::Date => "string".to_string(),    // SQLite stores as text
        SqlTypeCategory::Json => "string".to_string(),    // SQLite stores as text
        SqlTypeCategory::Binary => "Buffer".to_string(),
        SqlTypeCategory::Unknown => "unknown".to_string(),
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

fn generate_row_type(query: &QueryDef) -> String {
    if query.returns.is_empty() {
        return String::new();
    }
    let type_name = format!("{}Row", pascal_case(&query.name));
    let fields: Vec<String> = query
        .returns
        .iter()
        .map(|col| {
            let field_name = col.alias.as_deref().unwrap_or(&col.name);
            let ts = ts_type(&col.sql_type);
            let nullable = if col.nullable { " | null" } else { "" };
            format!("  {field_name}: {ts}{nullable};")
        })
        .collect();
    format!("export interface {type_name} {{\n{}\n}}", fields.join("\n"))
}

fn generate_params_type(query: &QueryDef) -> String {
    if query.params.is_empty() {
        return String::new();
    }
    let type_name = format!("{}Params", pascal_case(&query.name));
    let fields: Vec<String> = query
        .params
        .iter()
        .map(|p| format!("  {}: {};", p.name, ts_type(&p.sql_type)))
        .collect();
    format!("export interface {type_name} {{\n{}\n}}", fields.join("\n"))
}

fn json_stringify_sqlite(s: &str) -> (String, Vec<u32>) {
    let (sqlite_sql, indices) = to_sqlite_params(s);
    let escaped = sqlite_sql
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    (format!("\"{escaped}\""), indices)
}

fn generate_query_function(query: &QueryDef) -> String {
    let fn_name = camel_case(&query.name);
    let row_type = generate_row_type(query);
    let params_interface = generate_params_type(query);
    let has_params = !query.params.is_empty();
    let params_type_name = format!("{}Params", pascal_case(&query.name));
    let (sql_str, param_indices) = json_stringify_sqlite(&query.sql);
    let sql_const = format!("export const {fn_name}Sql = {sql_str};");

    let params_sig = if has_params {
        format!(", params: {params_type_name}")
    } else {
        String::new()
    };

    // Build args in SQL occurrence order (handles $2 AND $1, $1 OR $1)
    let spread_args = if has_params {
        let args: Vec<String> = param_indices
            .iter()
            .map(|idx| {
                query.params
                    .iter()
                    .find(|p| p.index == *idx)
                    .map(|p| format!("params.{}", p.name))
                    .unwrap_or_else(|| "undefined".to_string())
            })
            .collect();
        args.join(", ")
    } else {
        String::new()
    };

    // better-sqlite3 is synchronous — no async/await/Promise
    let (return_type, body) = match query.command {
        QueryCommand::One => {
            let type_name = format!("{}Row", pascal_case(&query.name));
            (
                format!("{type_name} | undefined"),
                format!("  return db.prepare({fn_name}Sql).get({spread_args}) as {type_name} | undefined;"),
            )
        }
        QueryCommand::Many => {
            let type_name = format!("{}Row", pascal_case(&query.name));
            (
                format!("{type_name}[]"),
                format!("  return db.prepare({fn_name}Sql).all({spread_args}) as {type_name}[];"),
            )
        }
        QueryCommand::Exec => (
            "void".to_string(),
            format!("  db.prepare({fn_name}Sql).run({spread_args});"),
        ),
        QueryCommand::ExecResult => (
            "{ changes: number }".to_string(),
            format!("  const result = db.prepare({fn_name}Sql).run({spread_args});\n  return {{ changes: result.changes }};"),
        ),
    };

    let mut parts: Vec<String> = Vec::new();
    if !row_type.is_empty() {
        parts.push(row_type);
    }
    if !params_interface.is_empty() {
        parts.push(params_interface);
    }
    parts.push(sql_const);
    parts.push(format!(
        "export function {fn_name}(db: Database{params_sig}): {return_type} {{\n{body}\n}}"
    ));

    parts.join("\n\n")
}

impl BetterSqlite3Generator {
    pub fn generate_client(&self) -> String {
        r#"import Database from "better-sqlite3";

export type { Database };
"#
        .to_string()
    }

    pub fn generate_query_functions(&self, queries: &[QueryDef]) -> String {
        let header = "// Code generated by sqlcx. DO NOT EDIT.\n\nimport type { Database } from \"./client\";";
        let functions: Vec<String> = queries.iter().map(generate_query_function).collect();
        if functions.is_empty() {
            return format!("{header}\n");
        }
        format!("{header}\n\n{}", functions.join("\n\n"))
    }
}

impl DriverGenerator for BetterSqlite3Generator {
    fn generate(&self, ir: &SqlcxIR) -> Result<Vec<GeneratedFile>> {
        let mut files = Vec::new();

        files.push(GeneratedFile {
            path: "client.ts".to_string(),
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
                path: format!("{}.queries.ts", basename),
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
        let gen = BetterSqlite3Generator;
        let content = gen.generate_client();
        assert!(content.contains("better-sqlite3"));
        insta::assert_snapshot!("better_sqlite3_client", content);
    }

    #[test]
    fn generates_query_functions() {
        let ir = parse_fixture_ir();
        let gen = BetterSqlite3Generator;
        let content = gen.generate_query_functions(&ir.queries);
        // Synchronous — no async
        assert!(content.contains("export function getUser"));
        assert!(!content.contains("async"));
        insta::assert_snapshot!("better_sqlite3_queries", content);
    }
}
