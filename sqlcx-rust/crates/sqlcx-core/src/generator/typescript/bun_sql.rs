// Bun.sql driver generator

use std::collections::HashMap;
use std::path::Path;

use crate::error::Result;
use crate::generator::{DriverGenerator, GeneratedFile};
use crate::ir::{QueryCommand, QueryDef, SqlType, SqlTypeCategory, SqlcxIR};
use crate::utils::{camel_case, pascal_case};

pub struct BunSqlGenerator;

// ── Type mapping ──────────────────────────────────────────────────────────────

fn ts_type(sql_type: &SqlType) -> String {
    if let Some(elem) = &sql_type.element_type {
        return format!("{}[]", ts_type(elem));
    }
    match sql_type.category {
        SqlTypeCategory::String | SqlTypeCategory::Uuid | SqlTypeCategory::Enum => {
            "string".to_string()
        }
        SqlTypeCategory::Number => "number".to_string(),
        SqlTypeCategory::Boolean => "boolean".to_string(),
        SqlTypeCategory::Date => "Date".to_string(),
        SqlTypeCategory::Json => "unknown".to_string(),
        SqlTypeCategory::Binary => "Uint8Array".to_string(),
        SqlTypeCategory::Unknown => "unknown".to_string(),
    }
}

// ── Per-query type generators ─────────────────────────────────────────────────

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

/// Escape a SQL string for embedding as a TS string literal (JSON.stringify equivalent).
fn json_stringify(s: &str) -> String {
    let escaped = s
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

// ── Public API ────────────────────────────────────────────────────────────────

impl BunSqlGenerator {
    /// Generate the client.ts file content (DatabaseClient interface + BunSqlClient adapter).
    pub fn generate_client(&self) -> String {
        r#"export interface DatabaseClient {
  query<T>(sql: string, params: unknown[]): Promise<T[]>;
  queryOne<T>(sql: string, params: unknown[]): Promise<T | null>;
  execute(sql: string, params: unknown[]): Promise<{ rowsAffected: number }>;
}

interface BunSqlDriver {
  unsafe(query: string, values?: unknown[]): Promise<any[] & { count: number }>;
}

export class BunSqlClient implements DatabaseClient {
  private sql: BunSqlDriver;

  constructor(sql: BunSqlDriver) {
    this.sql = sql;
  }

  async query<T>(text: string, values?: unknown[]): Promise<T[]> {
    const result = await this.sql.unsafe(text, values);
    return [...result] as T[];
  }

  async queryOne<T>(text: string, values?: unknown[]): Promise<T | null> {
    const rows = await this.query<T>(text, values);
    return rows[0] ?? null;
  }

  async execute(text: string, values?: unknown[]): Promise<{ rowsAffected: number }> {
    const result = await this.sql.unsafe(text, values);
    return { rowsAffected: result.count };
  }
}"#
        .to_string()
    }

    /// Generate all typed query functions for a set of queries (one query file).
    pub fn generate_query_functions(&self, queries: &[QueryDef]) -> String {
        let header = r#"import type { DatabaseClient } from "./client";"#;
        let functions: Vec<String> = queries.iter().map(generate_query_function).collect();
        if functions.is_empty() {
            return format!("{header}\n");
        }
        format!("{header}\n\n{}", functions.join("\n\n"))
    }
}

impl DriverGenerator for BunSqlGenerator {
    fn generate(&self, ir: &SqlcxIR) -> Result<Vec<GeneratedFile>> {
        let mut files = Vec::new();

        // client.ts
        files.push(GeneratedFile {
            path: "client.ts".to_string(),
            content: self.generate_client(),
        });

        // Group queries by source_file → one .queries.ts per file
        let mut grouped: HashMap<String, Vec<&QueryDef>> = HashMap::new();
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

// ── Tests ─────────────────────────────────────────────────────────────────────

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
        SqlcxIR {
            tables,
            queries,
            enums,
        }
    }

    #[test]
    fn generates_client_file() {
        let gen = BunSqlGenerator;
        let content = gen.generate_client();
        assert!(content.contains("export interface DatabaseClient"));
        assert!(content.contains("export class BunSqlClient implements DatabaseClient"));
        insta::assert_snapshot!("bun_sql_client", content);
    }

    #[test]
    fn generates_query_functions() {
        let ir = parse_fixture_ir();
        let gen = BunSqlGenerator;
        let content = gen.generate_query_functions(&ir.queries);
        assert!(content.contains("export async function getUser"));
        assert!(content.contains("export interface GetUserRow"));
        assert!(content.contains("getUserSql"));
        insta::assert_snapshot!("bun_sql_queries", content);
    }
}
