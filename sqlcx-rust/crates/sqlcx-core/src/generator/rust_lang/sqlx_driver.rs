use std::path::Path;

use crate::error::Result;
use crate::generator::{DriverGenerator, GeneratedFile};
use crate::ir::{ColumnDef, QueryCommand, QueryDef, SqlType, SqlTypeCategory, SqlcxIR};
use crate::utils::pascal_case;

pub struct SqlxGenerator;

// ── Type mapping ──────────────────────────────────────────────────────────────

/// Map a SQL type to its Rust type for query row structs.
fn rust_type(sql_type: &SqlType) -> String {
    if let Some(elem) = &sql_type.element_type {
        return format!("Vec<{}>", rust_type(elem));
    }

    match sql_type.category {
        SqlTypeCategory::String | SqlTypeCategory::Uuid | SqlTypeCategory::Enum => {
            "String".to_string()
        }
        SqlTypeCategory::Number => number_type(&sql_type.raw),
        SqlTypeCategory::Boolean => "bool".to_string(),
        SqlTypeCategory::Date => date_type(&sql_type.raw),
        SqlTypeCategory::Json => "serde_json::Value".to_string(),
        SqlTypeCategory::Binary => "Vec<u8>".to_string(),
        SqlTypeCategory::Unknown => "serde_json::Value".to_string(),
    }
}

fn number_type(raw: &str) -> String {
    let upper = raw.to_uppercase();
    if upper.contains("BIGINT") || upper.contains("BIGSERIAL") {
        "i64".to_string()
    } else if upper.contains("REAL")
        || upper.contains("FLOAT")
        || upper.contains("DOUBLE")
        || upper.contains("DECIMAL")
        || upper.contains("NUMERIC")
    {
        "f64".to_string()
    } else {
        "i32".to_string()
    }
}

fn date_type(raw: &str) -> String {
    let upper = raw.to_uppercase();
    if upper.contains("TIMESTAMP") {
        "chrono::NaiveDateTime".to_string()
    } else if upper.contains("TIME") {
        "chrono::NaiveTime".to_string()
    } else {
        "chrono::NaiveDate".to_string()
    }
}

/// Build a field type for a return column: nullable → `Option<T>`.
fn row_field_type(col: &ColumnDef) -> String {
    let base = rust_type(&col.sql_type);
    if col.nullable {
        format!("Option<{}>", base)
    } else {
        base
    }
}

/// Map a SQL type to its Rust function parameter type (using references).
fn param_type(sql_type: &SqlType) -> String {
    let base = rust_type(sql_type);
    match base.as_str() {
        "String" => "&str".to_string(),
        "Vec<u8>" => "&[u8]".to_string(),
        _ => base,
    }
}

/// Convert a snake_case function name to snake_case (no-op, but ensures
/// consistency with the naming convention).
fn snake_case(name: &str) -> String {
    // Query names from the parser are already in snake_case form like
    // "get_user". camel_case would produce "getUser"; we want "get_user".
    // We just lowercase the first char if it's pascal and insert underscores.
    let mut out = String::with_capacity(name.len() + 4);
    let chars: Vec<char> = name.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() && i > 0 && chars[i - 1].is_lowercase() {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

// ── Per-query generators ──────────────────────────────────────────────────────

fn generate_row_struct(query: &QueryDef) -> String {
    if query.returns.is_empty() {
        return String::new();
    }
    let type_name = format!("{}Row", pascal_case(&query.name));
    let fields: Vec<String> = query
        .returns
        .iter()
        .map(|col| {
            let field_name = col.alias.as_deref().unwrap_or(&col.name);
            format!("    pub {}: {},", field_name, row_field_type(col))
        })
        .collect();

    format!(
        "#[derive(Debug, Clone, sqlx::FromRow)]\npub struct {} {{\n{}\n}}",
        type_name,
        fields.join("\n")
    )
}

fn generate_result_struct(query: &QueryDef) -> String {
    let type_name = format!("{}Result", pascal_case(&query.name));
    format!(
        "pub struct {} {{\n    pub rows_affected: u64,\n}}",
        type_name
    )
}

fn generate_query_function(query: &QueryDef) -> String {
    let fn_name = snake_case(&query.name);
    let sql_const_name = format!(
        "{}_SQL",
        fn_name.to_uppercase()
    );

    let mut parts: Vec<String> = Vec::new();

    // SQL constant
    parts.push(format!(
        "pub const {}: &str = {:?};",
        sql_const_name,
        query.sql
    ));

    // Row struct (for :one and :many)
    let row_struct = generate_row_struct(query);
    if !row_struct.is_empty() {
        parts.push(row_struct);
    }

    // Result struct (for :execresult)
    if query.command == QueryCommand::ExecResult {
        parts.push(generate_result_struct(query));
    }

    // Function params
    let mut params_sig = String::from("pool: &sqlx::PgPool");
    for p in &query.params {
        let ptype = param_type(&p.sql_type);
        // For reference types, the param is already a reference
        if ptype.starts_with('&') {
            params_sig.push_str(&format!(", {}: {}", p.name, ptype));
        } else {
            params_sig.push_str(&format!(", {}: {}", p.name, ptype));
        }
    }

    // Bind calls
    let binds: String = query
        .params
        .iter()
        .map(|p| format!("\n        .bind({})", p.name))
        .collect();

    // Function body based on command
    let (return_type, body) = match query.command {
        QueryCommand::One => {
            let type_name = format!("{}Row", pascal_case(&query.name));
            (
                format!("Result<Option<{}>, sqlx::Error>", type_name),
                format!(
                    "    sqlx::query_as::<_, {}>({}){}
        .fetch_optional(pool)
        .await",
                    type_name, sql_const_name, binds
                ),
            )
        }
        QueryCommand::Many => {
            let type_name = format!("{}Row", pascal_case(&query.name));
            (
                format!("Result<Vec<{}>, sqlx::Error>", type_name),
                format!(
                    "    sqlx::query_as::<_, {}>({}){}
        .fetch_all(pool)
        .await",
                    type_name, sql_const_name, binds
                ),
            )
        }
        QueryCommand::Exec => (
            "Result<(), sqlx::Error>".to_string(),
            format!(
                "    sqlx::query({}){}
        .fetch_optional(pool)
        .await
        .map(|_| ())",
                sql_const_name, binds
            ),
        ),
        QueryCommand::ExecResult => {
            let result_type = format!("{}Result", pascal_case(&query.name));
            (
                format!("Result<{}, sqlx::Error>", result_type),
                format!(
                    "    let result = sqlx::query({}){}
        .execute(pool)
        .await?;
    Ok({} {{ rows_affected: result.rows_affected() }})",
                    sql_const_name, binds, result_type
                ),
            )
        }
    };

    parts.push(format!(
        "pub async fn {}({}) -> {} {{\n{}\n}}",
        fn_name, params_sig, return_type, body
    ));

    parts.join("\n\n")
}

// ── Public API ────────────────────────────────────────────────────────────────

impl SqlxGenerator {
    /// Generate the client.rs file content.
    pub fn generate_client(&self) -> String {
        "// Code generated by sqlcx. DO NOT EDIT.\n\n\
         // This module uses sqlx for database access.\n\
         // Pass a &sqlx::PgPool, &sqlx::MySqlPool, or &sqlx::SqlitePool\n\
         // to the query functions below."
            .to_string()
    }

    /// Generate all typed query functions for a set of queries.
    pub fn generate_query_functions(&self, queries: &[QueryDef]) -> String {
        let header = "// Code generated by sqlcx. DO NOT EDIT.\n\nuse sqlx;";
        let functions: Vec<String> = queries.iter().map(generate_query_function).collect();
        if functions.is_empty() {
            return format!("{header}\n");
        }
        format!("{header}\n\n{}", functions.join("\n\n"))
    }
}

impl DriverGenerator for SqlxGenerator {
    fn generate(&self, ir: &SqlcxIR) -> Result<Vec<GeneratedFile>> {
        let mut files = Vec::new();

        // client.rs
        files.push(GeneratedFile {
            path: "client.rs".to_string(),
            content: self.generate_client(),
        });

        // Group queries by source_file → one _queries.rs per file
        let mut grouped: std::collections::BTreeMap<String, Vec<&QueryDef>> =
            std::collections::BTreeMap::new();
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
                path: format!("{}_queries.rs", basename),
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
        let gen = SqlxGenerator;
        let content = gen.generate_client();
        assert!(content.contains("sqlx"));
        assert!(content.contains("DO NOT EDIT"));
        insta::assert_snapshot!("sqlx_client", content);
    }

    #[test]
    fn generates_query_functions() {
        let ir = parse_fixture_ir();
        let gen = SqlxGenerator;
        let content = gen.generate_query_functions(&ir.queries);
        assert!(content.contains("pub async fn get_user"));
        assert!(content.contains("pub struct GetUserRow"));
        assert!(content.contains("GET_USER_SQL"));
        assert!(content.contains("pub async fn list_users"));
        assert!(content.contains("pub async fn create_user"));
        assert!(content.contains("pub async fn delete_user"));
        assert!(content.contains("DeleteUserResult"));
        insta::assert_snapshot!("sqlx_queries", content);
    }

    #[test]
    fn snake_case_conversion() {
        assert_eq!(snake_case("GetUser"), "get_user");
        assert_eq!(snake_case("get_user"), "get_user");
        assert_eq!(snake_case("ListUsers"), "list_users");
        assert_eq!(snake_case("CreateUser"), "create_user");
    }

    #[test]
    fn param_type_uses_references_for_strings() {
        let sql_type = SqlType {
            raw: "text".to_string(),
            normalized: "text".to_string(),
            category: SqlTypeCategory::String,
            element_type: None,
            enum_name: None,
            enum_values: None,
            json_shape: None,
        };
        assert_eq!(param_type(&sql_type), "&str");
    }

    #[test]
    fn param_type_keeps_primitives_by_value() {
        let sql_type = SqlType {
            raw: "integer".to_string(),
            normalized: "integer".to_string(),
            category: SqlTypeCategory::Number,
            element_type: None,
            enum_name: None,
            enum_values: None,
            json_shape: None,
        };
        assert_eq!(param_type(&sql_type), "i32");
    }
}
