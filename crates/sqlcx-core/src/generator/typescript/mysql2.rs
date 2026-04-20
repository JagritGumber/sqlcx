// mysql2 driver generator for TypeScript

use std::collections::BTreeMap;
use std::path::Path;

use crate::error::Result;
use crate::generator::typescript::common::{
    TsTypeMap, generate_params_type, generate_row_type, json_stringify,
};
use crate::generator::{DriverGenerator, GeneratedFile};
use crate::ir::{QueryCommand, QueryDef, SqlcxIR};
use crate::utils::{camel_case, pascal_case};

pub struct Mysql2Generator;

// mysql2 maps Binary to Buffer (Node's native binary type) instead of Uint8Array.
struct Mysql2TypeMap;
impl TsTypeMap for Mysql2TypeMap {
    fn binary_ty(&self) -> &'static str {
        "Buffer"
    }
}

/// Convert $1, $2, ... placeholders to ? for MySQL.
/// Returns the converted SQL and the param indices in occurrence order
/// (handles reused params like $1 OR $1 and out-of-order like $2 AND $1).
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
    let fn_name = camel_case(&query.name);
    let row_type = generate_row_type(&Mysql2TypeMap, query);
    let params_interface = generate_params_type(&Mysql2TypeMap, query);
    let has_params = !query.params.is_empty();
    let params_type_name = format!("{}Params", pascal_case(&query.name));
    let (mysql_sql, param_indices) = to_mysql_params(&query.sql);
    let sql_const = format!(
        "export const {fn_name}Sql = {};",
        json_stringify(&mysql_sql)
    );

    let params_sig = if has_params {
        format!(", params: {params_type_name}")
    } else {
        String::new()
    };

    // Build values array in SQL occurrence order (handles $2 AND $1, $1 OR $1)
    let values_arg = if has_params {
        let args: Vec<String> = param_indices
            .iter()
            .map(|idx| {
                query
                    .params
                    .iter()
                    .find(|p| p.index == *idx)
                    .map(|p| format!("params.{}", p.name))
                    .unwrap_or_else(|| "undefined".to_string())
            })
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
        let gen_ = Mysql2Generator;
        let content = gen_.generate_client();
        assert!(content.contains("mysql2/promise"));
        assert!(content.contains("export class Mysql2Client"));
        insta::assert_snapshot!("mysql2_client", content);
    }

    #[test]
    fn generates_query_functions() {
        let ir = parse_fixture_ir();
        let gen_ = Mysql2Generator;
        let content = gen_.generate_query_functions(&ir.queries);
        assert!(content.contains("export async function getUser"));
        insta::assert_snapshot!("mysql2_queries", content);
    }

    #[test]
    fn converts_dollar_params_to_question_marks() {
        let (sql, idx) = to_mysql_params("SELECT * FROM users WHERE id = $1");
        assert_eq!(sql, "SELECT * FROM users WHERE id = ?");
        assert_eq!(idx, vec![1]);

        let (sql, idx) = to_mysql_params("INSERT INTO users (a, b) VALUES ($1, $2)");
        assert_eq!(sql, "INSERT INTO users (a, b) VALUES (?, ?)");
        assert_eq!(idx, vec![1, 2]);

        let (sql, idx) = to_mysql_params("SELECT * FROM users");
        assert_eq!(sql, "SELECT * FROM users");
        assert!(idx.is_empty());

        // Reused params
        let (sql, idx) = to_mysql_params("WHERE a = $1 OR b = $1");
        assert_eq!(sql, "WHERE a = ? OR b = ?");
        assert_eq!(idx, vec![1, 1]);

        // Out-of-order params
        let (sql, idx) = to_mysql_params("WHERE a = $2 AND b = $1");
        assert_eq!(sql, "WHERE a = ? AND b = ?");
        assert_eq!(idx, vec![2, 1]);
    }
}
