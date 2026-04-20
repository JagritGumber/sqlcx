use std::path::Path;

use crate::error::Result;
use crate::generator::rust_lang::common::{param_type, row_field_type};
use crate::generator::{DriverGenerator, GeneratedFile};
use crate::ir::{QueryCommand, QueryDef, SqlcxIR};
use crate::utils::{pascal_case, snake_case};

/// sqlx supports multiple database backends through a single API shape.
/// We parameterize the Rust sqlx generator on which backend is targeted
/// — only the pool type and the query placeholder style differ.
#[derive(Debug, Clone, Copy)]
pub enum SqlxBackend {
    Postgres,
    MySql,
    Sqlite,
}

impl SqlxBackend {
    fn pool_type(self) -> &'static str {
        match self {
            SqlxBackend::Postgres => "sqlx::PgPool",
            SqlxBackend::MySql => "sqlx::MySqlPool",
            SqlxBackend::Sqlite => "sqlx::SqlitePool",
        }
    }

    /// Rewrite Postgres `$N` placeholders to the form this backend expects.
    /// Returns `(rewritten_sql, occurrence_indices)` where `occurrence_indices`
    /// is empty for Postgres (no rewrite, bind order = unique-index order)
    /// and for MySQL/SQLite is the sequence of `$N` *as they appeared in
    /// document order*. Callers use it to emit `.bind(...)` in the correct
    /// positional order, which matters for two cases:
    /// - Reused: `WHERE x = $1 OR y = $1` becomes `? ? ` (two placeholders,
    ///   both bind to params[index=1]).
    /// - Out-of-order: `WHERE b = $2 AND a = $1` becomes `? ?` where the
    ///   first `?` binds params[index=2] and the second binds params[index=1].
    fn rewrite_placeholders(self, sql: &str) -> (String, Vec<u32>) {
        match self {
            SqlxBackend::Postgres => (sql.to_string(), Vec::new()),
            SqlxBackend::MySql | SqlxBackend::Sqlite => {
                let mut out = String::with_capacity(sql.len());
                let mut indices = Vec::new();
                let mut chars = sql.chars().peekable();
                // Track whether we're currently inside a single-quoted SQL string
                // literal. `$N` inside a string is just literal text — don't rewrite.
                // SQL escapes single quotes by doubling (`''`), so consecutive quotes
                // stay inside the string.
                let mut in_string = false;
                while let Some(c) = chars.next() {
                    if c == '\'' {
                        if in_string && chars.peek() == Some(&'\'') {
                            // Escaped quote `''` — consume both, stay in string.
                            out.push(c);
                            out.push(chars.next().unwrap());
                            continue;
                        }
                        in_string = !in_string;
                        out.push(c);
                        continue;
                    }
                    if !in_string && c == '$' && chars.peek().is_some_and(|ch| ch.is_ascii_digit())
                    {
                        let mut num = String::new();
                        while chars.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                            num.push(chars.next().unwrap());
                        }
                        indices.push(num.parse::<u32>().unwrap_or(0));
                        out.push('?');
                    } else {
                        out.push(c);
                    }
                }
                (out, indices)
            }
        }
    }
}

pub struct SqlxGenerator {
    backend: SqlxBackend,
}

impl SqlxGenerator {
    pub fn postgres() -> Self {
        Self {
            backend: SqlxBackend::Postgres,
        }
    }

    pub fn mysql() -> Self {
        Self {
            backend: SqlxBackend::MySql,
        }
    }

    pub fn sqlite() -> Self {
        Self {
            backend: SqlxBackend::Sqlite,
        }
    }
}

impl Default for SqlxGenerator {
    fn default() -> Self {
        Self::postgres()
    }
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
        "#[derive(Debug, Clone)]\npub struct {} {{\n    pub rows_affected: u64,\n}}",
        type_name
    )
}

fn generate_query_function(backend: SqlxBackend, query: &QueryDef) -> String {
    let fn_name = snake_case(&query.name);
    let sql_const_name = format!("{}_SQL", fn_name.to_uppercase());

    let mut parts: Vec<String> = Vec::new();

    // SQL constant — placeholder style depends on backend.
    let (rewritten_sql, occurrence_indices) = backend.rewrite_placeholders(&query.sql);
    parts.push(format!(
        "pub const {}: &str = {:?};",
        sql_const_name, rewritten_sql
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

    // Function params — pool type depends on backend.
    let mut params_sig = format!("pool: &{}", backend.pool_type());
    for p in &query.params {
        let ptype = param_type(&p.sql_type);
        params_sig.push_str(&format!(", {}: {}", p.name, ptype));
    }

    // Bind calls. For Postgres, bind once per unique param (sqlx + Postgres
    // dedupe $N on the server side). For MySQL/SQLite, bind once per
    // placeholder occurrence in SQL order — reused or out-of-order $N in
    // the source Postgres SQL still produce correct behavior against the
    // rewritten `?` positional form.
    //
    // Non-Copy types (e.g. serde_json::Value for JSON columns, Vec<u8> for
    // binary) would fail to compile if a reused param produced two moves of
    // the same owned value. For any param index that appears more than once
    // in occurrence_indices we emit `.bind(name.clone())` for every use
    // after the first. Copy-typed params (i32, &str) accept `.clone()` too
    // (it's a no-op), so applying uniformly to duplicates is safe.
    let binds: String = if occurrence_indices.is_empty() {
        query
            .params
            .iter()
            .map(|p| format!("\n        .bind({})", p.name))
            .collect()
    } else {
        let mut seen: std::collections::HashSet<u32> = std::collections::HashSet::new();
        let reused: std::collections::HashSet<u32> = {
            let mut once = std::collections::HashSet::new();
            let mut dup = std::collections::HashSet::new();
            for idx in &occurrence_indices {
                if !once.insert(*idx) {
                    dup.insert(*idx);
                }
            }
            dup
        };
        occurrence_indices
            .iter()
            .map(|idx| {
                let param_name = query
                    .params
                    .iter()
                    .find(|p| p.index == *idx)
                    .map(|p| p.name.as_str())
                    .unwrap_or("unknown");
                let expr = if reused.contains(idx) && !seen.insert(*idx) {
                    // Second or later occurrence of a reused param → clone.
                    format!("{param_name}.clone()")
                } else {
                    // First occurrence (reused or not) → move/copy.
                    seen.insert(*idx);
                    param_name.to_string()
                };
                format!("\n        .bind({expr})")
            })
            .collect()
    };

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
        .execute(pool)
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
        let functions: Vec<String> = queries
            .iter()
            .map(|q| generate_query_function(self.backend, q))
            .collect();
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
        let gen_ = SqlxGenerator::postgres();
        let content = gen_.generate_client();
        assert!(content.contains("sqlx"));
        assert!(content.contains("DO NOT EDIT"));
        insta::assert_snapshot!("sqlx_client", content);
    }

    #[test]
    fn generates_query_functions() {
        let ir = parse_fixture_ir();
        let gen_ = SqlxGenerator::postgres();
        let content = gen_.generate_query_functions(&ir.queries);
        assert!(content.contains("pub async fn get_user"));
        assert!(content.contains("pub struct GetUserRow"));
        assert!(content.contains("GET_USER_SQL"));
        assert!(content.contains("pub async fn list_users"));
        assert!(content.contains("pub async fn create_user"));
        assert!(content.contains("pub async fn delete_user"));
        assert!(content.contains("DeleteUserResult"));
        assert!(content.contains("pool: &sqlx::PgPool"));
        insta::assert_snapshot!("sqlx_queries", content);
    }

    #[test]
    fn mysql_backend_rewrites_placeholders_and_uses_mysqlpool() {
        let ir = parse_fixture_ir();
        let gen_ = SqlxGenerator::mysql();
        let content = gen_.generate_query_functions(&ir.queries);
        assert!(content.contains("pool: &sqlx::MySqlPool"));
        assert!(!content.contains("pool: &sqlx::PgPool"));
        // The fixture query `SELECT * FROM users WHERE id = $1` must get `?`.
        assert!(content.contains("WHERE id = ?"));
        assert!(!content.contains("WHERE id = $1"));
        insta::assert_snapshot!("sqlx_mysql_queries", content);
    }

    #[test]
    fn sqlite_backend_rewrites_placeholders_and_uses_sqlitepool() {
        let ir = parse_fixture_ir();
        let gen_ = SqlxGenerator::sqlite();
        let content = gen_.generate_query_functions(&ir.queries);
        assert!(content.contains("pool: &sqlx::SqlitePool"));
        assert!(!content.contains("pool: &sqlx::PgPool"));
        assert!(content.contains("WHERE id = ?"));
        insta::assert_snapshot!("sqlx_sqlite_queries", content);
    }

    #[test]
    fn placeholder_rewrite_preserves_nonparam_dollars() {
        // The parser must only eat $N, not bare $ not followed by digits.
        let (sql, idx) =
            SqlxBackend::MySql.rewrite_placeholders("SELECT '$foo' FROM x WHERE a = $1");
        assert_eq!(sql, "SELECT '$foo' FROM x WHERE a = ?");
        assert_eq!(idx, vec![1]);

        let (sql, idx) =
            SqlxBackend::Postgres.rewrite_placeholders("SELECT '$foo' FROM x WHERE a = $1");
        assert_eq!(sql, "SELECT '$foo' FROM x WHERE a = $1");
        assert!(idx.is_empty()); // Postgres keeps native $N, no occurrence indices needed.
    }

    #[test]
    fn rewrite_tracks_occurrence_indices_for_reused_params() {
        // `WHERE x = $1 OR y = $1` must produce two `?` placeholders AND
        // report [1, 1] so the bind chain binds params[idx=1] twice.
        let (sql, idx) = SqlxBackend::MySql.rewrite_placeholders("WHERE x = $1 OR y = $1");
        assert_eq!(sql, "WHERE x = ? OR y = ?");
        assert_eq!(idx, vec![1, 1]);
    }

    #[test]
    fn rewrite_tracks_occurrence_indices_for_out_of_order_params() {
        // `WHERE b = $2 AND a = $1` must report [2, 1] so the bind chain
        // binds params[idx=2] first, then params[idx=1].
        let (sql, idx) = SqlxBackend::Sqlite.rewrite_placeholders("WHERE b = $2 AND a = $1");
        assert_eq!(sql, "WHERE b = ? AND a = ?");
        assert_eq!(idx, vec![2, 1]);
    }

    #[test]
    fn reused_param_in_mysql_body_emits_clone_on_duplicates() {
        // Synthesize a QueryDef that reuses $1 twice. The MySQL bind chain
        // must emit .bind(x).bind(x.clone()) so non-Copy types don't cause
        // a moved-value compile error in the generated code.
        let query = QueryDef {
            name: "SearchUsers".to_string(),
            command: QueryCommand::Many,
            sql: "SELECT * FROM users WHERE name ILIKE $1 OR email ILIKE $1".to_string(),
            params: vec![ParamDef {
                index: 1,
                name: "q".to_string(),
                sql_type: SqlType {
                    raw: "text".to_string(),
                    normalized: "text".to_string(),
                    category: SqlTypeCategory::String,
                    element_type: None,
                    enum_name: None,
                    enum_values: None,
                    json_shape: None,
                },
            }],
            returns: vec![],
            source_file: "q.sql".to_string(),
        };
        let out = generate_query_function(SqlxBackend::MySql, &query);
        // First occurrence moves, second occurrence clones.
        assert!(out.contains(".bind(q)"));
        assert!(out.contains(".bind(q.clone())"));
        // Postgres path never clones — it binds once per unique $N.
        let out = generate_query_function(SqlxBackend::Postgres, &query);
        assert!(out.contains(".bind(q)"));
        assert!(!out.contains(".bind(q.clone())"));
    }

    #[test]
    fn rewrite_preserves_dollar_n_inside_string_literals() {
        // `$1` inside a single-quoted string is literal text — must not be
        // rewritten, otherwise generated code would bind too many values.
        let (sql, idx) =
            SqlxBackend::MySql.rewrite_placeholders("SELECT '$1' FROM users WHERE id = $1");
        assert_eq!(sql, "SELECT '$1' FROM users WHERE id = ?");
        assert_eq!(idx, vec![1]);

        // SQL escaped single quotes `''` stay inside the string.
        let (sql, idx) =
            SqlxBackend::MySql.rewrite_placeholders("SELECT 'O''Brien $1' FROM x WHERE id = $1");
        assert_eq!(sql, "SELECT 'O''Brien $1' FROM x WHERE id = ?");
        assert_eq!(idx, vec![1]);
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
