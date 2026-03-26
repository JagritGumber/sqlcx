// mysql2 driver generator for TypeScript

use std::collections::BTreeMap;
use std::path::Path;

use crate::error::Result;
use crate::generator::{DriverGenerator, GeneratedFile};
use crate::ir::{QueryCommand, QueryDef, SqlType, SqlTypeCategory, SqlcxIR};
use crate::utils::{camel_case, pascal_case};

pub struct Mysql2Generator;

fn ts_type(sql_type: &SqlType) -> String {
    if let Some(elem) = &sql_type.element_type {
        return format!("{}[]", ts_type(elem));
    }
    match sql_type.category {
        SqlTypeCategory::String | SqlTypeCategory::Uuid | SqlTypeCategory::Enum => "string".to_string(),
        SqlTypeCategory::Number => "number".to_string(),
        SqlTypeCategory::Boolean => "boolean".to_string(),
        SqlTypeCategory::Date => "Date".to_string(),
        SqlTypeCategory::Json => "unknown".to_string(),
        SqlTypeCategory::Binary => "Buffer".to_string(),
        SqlTypeCategory::Unknown => "unknown".to_string(),
    }
}

/// Convert $1, $2, ... placeholders to ? for MySQL
fn to_mysql_params(sql: &str) -> String {
    let mut result = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' {
            if chars.peek().map_or(false, |ch| ch.is_ascii_digit()) {
                result.push('?');
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

fn json_stringify(s: &str) -> String {
    let mysql_sql = to_mysql_params(s);
    let escaped = mysql_sql
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    format!("\"{escaped}\"")
}

fn generate_query_function(query: &QueryDef) -> String {
    let fn_name = camel_case(&query.name);
    let row_type = generate_row_type(query);
    let params_interface = generate_params_type(query);
    let has_params = !query.params.is_empty();
    let params_type_name = format!("{}Params", pascal_case(&query.name));
    let sql_const = format!("export const {fn_name}Sql = {};", json_stringify(&query.sql));

    let params_sig = if has_params {
        format!(", params: {params_type_name}")
    } else {
        String::new()
    };

    let values_arg = if has_params {
        let args: Vec<String> = query
            .params
            .iter()
            .map(|p| format!("params.{}", p.name))
            .collect();
        format!("[{}]", args.join(", "))
    } else {
        "[]".to_string()
    };

    let (return_type, body) = match query.command {
        QueryCommand::One => {
            let type_name = format!("{}Row", pascal_case(&query.name));
            (
                format!("Promise<{type_name} | null>"),
                format!("  return client.queryOne<{type_name}>({fn_name}Sql, {values_arg});"),
            )
        }
        QueryCommand::Many => {
            let type_name = format!("{}Row", pascal_case(&query.name));
            (
                format!("Promise<{type_name}[]>"),
                format!("  return client.query<{type_name}>({fn_name}Sql, {values_arg});"),
            )
        }
        QueryCommand::Exec => (
            "Promise<void>".to_string(),
            format!("  await client.execute({fn_name}Sql, {values_arg});"),
        ),
        QueryCommand::ExecResult => (
            "Promise<{ rowsAffected: number }>".to_string(),
            format!("  return client.execute({fn_name}Sql, {values_arg});"),
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
        "export async function {fn_name}(client: DatabaseClient{params_sig}): {return_type} {{\n{body}\n}}"
    ));

    parts.join("\n\n")
}

impl Mysql2Generator {
    pub fn generate_client(&self) -> String {
        r#"import mysql from "mysql2/promise";

export interface DatabaseClient {
  query<T>(sql: string, params: unknown[]): Promise<T[]>;
  queryOne<T>(sql: string, params: unknown[]): Promise<T | null>;
  execute(sql: string, params: unknown[]): Promise<{ rowsAffected: number }>;
}

export class Mysql2Client implements DatabaseClient {
  private pool: mysql.Pool;

  constructor(pool: mysql.Pool) {
    this.pool = pool;
  }

  async query<T>(text: string, values?: unknown[]): Promise<T[]> {
    const [rows] = await this.pool.execute(text, values);
    return rows as T[];
  }

  async queryOne<T>(text: string, values?: unknown[]): Promise<T | null> {
    const rows = await this.query<T>(text, values);
    return rows[0] ?? null;
  }

  async execute(text: string, values?: unknown[]): Promise<{ rowsAffected: number }> {
    const [result] = await this.pool.execute(text, values);
    return { rowsAffected: (result as mysql.ResultSetHeader).affectedRows };
  }
}"#
        .to_string()
    }

    pub fn generate_query_functions(&self, queries: &[QueryDef]) -> String {
        let header = "// Code generated by sqlcx. DO NOT EDIT.\n\nimport type { DatabaseClient } from \"./client\";";
        let functions: Vec<String> = queries.iter().map(generate_query_function).collect();
        if functions.is_empty() {
            return format!("{header}\n");
        }
        format!("{header}\n\n{}", functions.join("\n\n"))
    }
}

impl DriverGenerator for Mysql2Generator {
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
        let gen = Mysql2Generator;
        let content = gen.generate_client();
        assert!(content.contains("mysql2/promise"));
        assert!(content.contains("export class Mysql2Client"));
        insta::assert_snapshot!("mysql2_client", content);
    }

    #[test]
    fn generates_query_functions() {
        let ir = parse_fixture_ir();
        let gen = Mysql2Generator;
        let content = gen.generate_query_functions(&ir.queries);
        assert!(content.contains("export async function getUser"));
        insta::assert_snapshot!("mysql2_queries", content);
    }

    #[test]
    fn converts_dollar_params_to_question_marks() {
        assert_eq!(to_mysql_params("SELECT * FROM users WHERE id = $1"), "SELECT * FROM users WHERE id = ?");
        assert_eq!(to_mysql_params("INSERT INTO users (a, b) VALUES ($1, $2)"), "INSERT INTO users (a, b) VALUES (?, ?)");
        assert_eq!(to_mysql_params("SELECT * FROM users"), "SELECT * FROM users");
    }
}
