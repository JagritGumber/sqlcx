use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;

use crate::annotations::extract_annotations;
use crate::error::Result;
use crate::ir::{ColumnDef, EnumDef, QueryDef, SqlType, SqlTypeCategory, TableDef};
use crate::parser::joins::{has_outer_join, resolve_multi_table_columns};
use crate::parser::{
    DatabaseParser, build_params, resolve_single_table_select_column, split_column_defs,
    split_query_blocks,
};

// ── Static regex patterns ────────────────────────────────────────────────────

static ENUM_DEF_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)CREATE\s+TYPE\s+(\w+)\s+AS\s+ENUM\s*\(\s*((?:'[^']*'(?:\s*,\s*'[^']*')*)?)\s*\)",
    )
    .unwrap()
});

static ENUM_VAL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"'([^']*)'").unwrap());

static CONSTRAINT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(PRIMARY\s+KEY|CONSTRAINT|UNIQUE|CHECK|FOREIGN\s+KEY)").unwrap()
});

static COL_NAME_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\w+)\s+").unwrap());

static COL_TYPE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\w+(?:\[\])?)").unwrap());

static NOT_NULL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bNOT\s+NULL\b").unwrap());

static DEFAULT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bDEFAULT\b").unwrap());

static PK_INLINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bPRIMARY\s+KEY\b").unwrap());

static UNIQUE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bUNIQUE\b").unwrap());

static TABLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?(\w+)\s*\(([\s\S]*?)\)\s*;")
        .unwrap()
});

static TABLE_PK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^PRIMARY\s+KEY\s*\(\s*([\w\s,]+)\s*\)").unwrap());

static PARAM_INDEX_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$(\d+)").unwrap());

static INSERT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)INSERT\s+INTO\s+\w+\s*\(\s*([\w\s,]+)\s*\)\s*VALUES\s*\(\s*([\$\d\s,]+)\s*\)")
        .unwrap()
});

static WHERE_PARAM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:(\w+)\s*\(\s*(\w+)\s*\)|(\w+))\s*(?:=|!=|<>|<=?|>=?|(?:NOT\s+)?(?:I?LIKE|IN|IS))\s*\$(\d+)",
    )
    .unwrap()
});

static FROM_TABLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:FROM|INTO|UPDATE)\s+(\w+)").unwrap());

static RETURNING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bRETURNING\s+([\s\S]+?)(?:;?\s*)$").unwrap());

static SELECT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\s*SELECT\b").unwrap());

static SELECT_COLS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)SELECT\s+([\s\S]+?)\s+FROM\b").unwrap());

// ── Type mapping ─────────────────────────────────────────────────────────────

fn type_category(normalized: &str) -> Option<SqlTypeCategory> {
    match normalized {
        "text" | "varchar" | "char" | "character varying" | "character" | "name" => {
            Some(SqlTypeCategory::String)
        }
        "integer" | "int" | "int2" | "int4" | "int8" | "smallint" | "bigint" | "serial"
        | "bigserial" | "real" | "double precision" | "numeric" | "decimal" | "float"
        | "float4" | "float8" => Some(SqlTypeCategory::Number),
        "boolean" | "bool" => Some(SqlTypeCategory::Boolean),
        "timestamp"
        | "timestamptz"
        | "date"
        | "time"
        | "timetz"
        | "timestamp without time zone"
        | "timestamp with time zone" => Some(SqlTypeCategory::Date),
        "json" | "jsonb" => Some(SqlTypeCategory::Json),
        "uuid" => Some(SqlTypeCategory::Uuid),
        "bytea" => Some(SqlTypeCategory::Binary),
        _ => None,
    }
}

fn is_serial(normalized: &str) -> bool {
    matches!(normalized, "serial" | "bigserial")
}

fn resolve_sql_type(raw: &str, enum_names: &HashSet<String>) -> SqlType {
    let trimmed = raw.trim();

    // Array detection
    if let Some(base_raw) = trimmed.strip_suffix("[]") {
        let element = resolve_sql_type(base_raw, enum_names);
        return SqlType {
            raw: trimmed.to_string(),
            normalized: trimmed.to_lowercase(),
            category: element.category.clone(),
            element_type: Some(Box::new(element)),
            enum_name: None,
            enum_values: None,
            json_shape: None,
        };
    }

    let normalized = trimmed.to_lowercase();

    if let Some(cat) = type_category(&normalized) {
        return SqlType {
            raw: trimmed.to_string(),
            normalized,
            category: cat,
            element_type: None,
            enum_name: None,
            enum_values: None,
            json_shape: None,
        };
    }

    // Check for known enum
    if enum_names.contains(&normalized) {
        return SqlType {
            raw: trimmed.to_string(),
            normalized: normalized.clone(),
            category: SqlTypeCategory::Enum,
            element_type: None,
            enum_name: Some(normalized),
            enum_values: None,
            json_shape: None,
        };
    }

    SqlType {
        raw: trimmed.to_string(),
        normalized,
        category: SqlTypeCategory::Unknown,
        element_type: None,
        enum_name: None,
        enum_values: None,
        json_shape: None,
    }
}

// ── Enum parsing ─────────────────────────────────────────────────────────────

fn parse_enum_defs(sql: &str) -> Vec<EnumDef> {
    let mut enums = Vec::new();
    for cap in ENUM_DEF_RE.captures_iter(sql) {
        let name = cap[1].to_lowercase();
        let values_raw = &cap[2];
        let values: Vec<String> = ENUM_VAL_RE
            .captures_iter(values_raw)
            .map(|v| v[1].to_string())
            .collect();
        enums.push(EnumDef { name, values });
    }
    enums
}

// ── Schema parsing (regex-based, matching TS) ────────────────────────────────

const MULTI_WORD_TYPES: &[&str] = &[
    "character varying",
    "double precision",
    "timestamp without time zone",
    "timestamp with time zone",
];

struct ParsedColumn {
    col: ColumnDef,
    is_pk: bool,
    is_unique: bool,
}

fn parse_column_line(line: &str, enum_names: &HashSet<String>) -> Option<ParsedColumn> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    // Skip constraint lines
    if CONSTRAINT_RE.is_match(line) {
        return None;
    }

    // Extract column name (first word)
    let name_cap = COL_NAME_RE.captures(line)?;
    let col_name = name_cap[1].to_lowercase();
    let after_name = &line[name_cap[0].len()..];

    // Determine the type - check multi-word types first
    let mut raw_type: Option<String> = None;
    for mwt in MULTI_WORD_TYPES {
        if after_name.to_lowercase().starts_with(mwt) {
            raw_type = Some(mwt.to_string());
            break;
        }
    }
    if raw_type.is_none()
        && let Some(cap) = COL_TYPE_RE.captures(after_name)
    {
        raw_type = Some(cap[1].to_string());
    }
    let raw_type = raw_type.unwrap_or_else(|| "unknown".to_string());

    let rest = &after_name[raw_type.len()..];

    let is_not_null = NOT_NULL_RE.is_match(rest);
    let has_default_kw = DEFAULT_RE.is_match(rest);
    let is_serial_type = is_serial(&raw_type.to_lowercase());
    let is_pk = PK_INLINE_RE.is_match(rest);
    let is_unique = UNIQUE_RE.is_match(rest);

    let sql_type = resolve_sql_type(&raw_type, enum_names);

    Some(ParsedColumn {
        col: ColumnDef {
            name: col_name,
            alias: None,
            source_table: None,
            sql_type,
            nullable: !is_not_null,
            has_default: has_default_kw || is_serial_type,
        },
        is_pk,
        is_unique,
    })
}

fn parse_schema_tables(sql: &str, enum_names: &HashSet<String>) -> Vec<TableDef> {
    let mut tables = Vec::new();

    for cap in TABLE_RE.captures_iter(sql) {
        let table_name = cap[1].to_lowercase();
        let body = &cap[2];

        let mut columns = Vec::new();
        let mut primary_key: Vec<String> = Vec::new();
        let mut unique_constraints: Vec<Vec<String>> = Vec::new();

        // Split body into lines, track comments for annotations
        let raw_lines: Vec<&str> = body.lines().collect();
        let mut pending_comment = String::new();
        let mut non_comment_buf = String::new();
        let mut comment_map: HashMap<usize, String> = HashMap::new();

        for raw_line in &raw_lines {
            let trimmed = raw_line.trim();
            if trimmed.starts_with("--") {
                if !pending_comment.is_empty() {
                    pending_comment.push('\n');
                }
                pending_comment.push_str(trimmed);
            } else {
                let before = split_column_defs(&non_comment_buf)
                    .iter()
                    .filter(|d| !d.is_empty())
                    .count();
                if !non_comment_buf.is_empty() {
                    non_comment_buf.push('\n');
                }
                non_comment_buf.push_str(raw_line);
                let after = split_column_defs(&non_comment_buf)
                    .iter()
                    .filter(|d| !d.is_empty())
                    .count();

                if after > before && !pending_comment.is_empty() {
                    comment_map.insert(before, pending_comment.clone());
                    pending_comment.clear();
                } else if after == before {
                    // Still accumulating same def
                } else {
                    pending_comment.clear();
                }
            }
        }

        let lines = split_column_defs(&non_comment_buf);

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Table-level PRIMARY KEY constraint
            if let Some(pk_cap) = TABLE_PK_RE.captures(trimmed) {
                for col in pk_cap[1].split(',') {
                    primary_key.push(col.trim().to_lowercase());
                }
                continue;
            }

            let Some(mut parsed) = parse_column_line(trimmed, enum_names) else {
                continue;
            };

            // Apply annotations from comment above this column
            if let Some(comment) = comment_map.get(&i) {
                let (_, ann) = extract_annotations(comment);
                if let Some(values) = ann.enums.get(&parsed.col.name) {
                    parsed.col.sql_type.category = SqlTypeCategory::Enum;
                    parsed.col.sql_type.enum_values = Some(values.clone());
                }
                if let Some(shape) = ann.json_shapes.get(&parsed.col.name) {
                    parsed.col.sql_type.json_shape = Some(shape.clone());
                }
            }

            if parsed.is_pk {
                primary_key.push(parsed.col.name.clone());
            }
            if parsed.is_unique {
                unique_constraints.push(vec![parsed.col.name.clone()]);
            }
            columns.push(parsed.col);
        }

        // PK columns are implicitly NOT NULL
        for col in &mut columns {
            if primary_key.contains(&col.name) {
                col.nullable = false;
                if is_serial(&col.sql_type.normalized) {
                    col.has_default = true;
                }
            }
        }

        tables.push(TableDef {
            name: table_name,
            columns,
            primary_key,
            unique_constraints,
        });
    }

    tables
}

// ── Query parsing ────────────────────────────────────────────────────────────

fn extract_param_indices(sql: &str) -> Vec<u32> {
    let mut indices: HashSet<u32> = HashSet::new();
    for cap in PARAM_INDEX_RE.captures_iter(sql) {
        if let Ok(idx) = cap[1].parse::<u32>() {
            indices.insert(idx);
        }
    }
    let mut sorted: Vec<u32> = indices.into_iter().collect();
    sorted.sort();
    sorted
}

fn infer_param_columns(sql: &str) -> HashMap<u32, String> {
    let mut result = HashMap::new();

    // INSERT pattern
    if let Some(cap) = INSERT_RE.captures(sql) {
        let cols: Vec<String> = cap[1].split(',').map(|s| s.trim().to_lowercase()).collect();
        let params: Vec<u32> = PARAM_INDEX_RE
            .captures_iter(&cap[2])
            .filter_map(|m| m[1].parse().ok())
            .collect();

        for (i, idx) in params.iter().enumerate() {
            if i < cols.len() {
                result.insert(*idx, cols[i].clone());
            }
        }
        return result;
    }

    // WHERE/SET pattern
    let sql_keywords: HashSet<&str> = [
        "not", "and", "or", "where", "set", "when", "then", "else", "case", "between", "exists",
        "any", "all", "some", "having",
    ]
    .into_iter()
    .collect();

    for cap in WHERE_PARAM_RE.captures_iter(sql) {
        if let Ok(idx) = cap[4].parse::<u32>() {
            if cap.get(1).is_some() && cap.get(2).is_some() {
                // FUNC(col) pattern
                result.insert(idx, cap[2].to_lowercase());
            } else if let Some(m) = cap.get(3) {
                let word = m.as_str().to_lowercase();
                if !sql_keywords.contains(word.as_str()) {
                    result.insert(idx, word);
                }
            }
        }
    }

    result
}

fn find_from_table<'a>(sql: &str, tables: &'a [TableDef]) -> Option<&'a TableDef> {
    let cap = FROM_TABLE_RE.captures(sql)?;
    let table_name = cap[1].to_lowercase();
    tables.iter().find(|t| t.name == table_name)
}

fn extract_table_alias<'a>(sql: &'a str, table: &TableDef) -> Option<&'a str> {
    let lower = sql.to_lowercase();
    let table_name = &table.name;
    let patterns = [
        format!("from {} as ", table_name),
        format!("into {} as ", table_name),
        format!("update {} as ", table_name),
        format!("from {} ", table_name),
        format!("into {} ", table_name),
        format!("update {} ", table_name),
    ];

    for pattern in patterns {
        if let Some(idx) = lower.find(&pattern) {
            let remainder = sql[idx + pattern.len()..].trim_start();
            let alias = remainder
                .split(|ch: char| ch.is_whitespace() || ch == ';' || ch == ',')
                .next()
                .unwrap_or("");
            if !alias.is_empty()
                && !matches!(
                    alias.to_lowercase().as_str(),
                    "where" | "join" | "order" | "group" | "limit" | "returning"
                )
            {
                return Some(alias);
            }
        }
    }

    None
}

fn resolve_returning_columns(
    sql: &str,
    table: Option<&TableDef>,
    source_file: &str,
) -> Result<Option<Vec<ColumnDef>>> {
    let Some(cap) = RETURNING_RE.captures(sql) else {
        return Ok(None);
    };
    let cols_part = cap[1].trim();

    if cols_part == "*" {
        return Ok(Some(table.map(|t| t.columns.clone()).unwrap_or_default()));
    }

    let Some(table) = table else {
        return Ok(None);
    };
    let alias = extract_table_alias(sql, table);
    let mut allowed_prefixes = vec![table.name.as_str()];
    if let Some(alias) = alias {
        allowed_prefixes.push(alias);
    }
    Ok(Some(
        cols_part
            .split(',')
            .map(|s| resolve_single_table_select_column(s, &allowed_prefixes, table, source_file))
            .collect::<Result<Vec<_>>>()?,
    ))
}

fn resolve_return_columns(
    sql: &str,
    table: Option<&TableDef>,
    schema_tables: &[TableDef],
    source_file: &str,
) -> Result<Vec<ColumnDef>> {
    // Check RETURNING clause first
    if let Some(returning) = resolve_returning_columns(sql, table, source_file)? {
        return Ok(returning);
    }

    if !SELECT_RE.is_match(sql) {
        return Ok(Vec::new());
    }

    let Some(cap) = SELECT_COLS_RE.captures(sql) else {
        return Ok(Vec::new());
    };
    let cols_part = cap[1].trim();

    // Multi-table JOIN path: when the outer FROM contains a JOIN, route
    // each select expression through the shared multi-table resolver.
    // `has_outer_join` scopes the check to the outer FROM body so that
    // subqueries with JOINs (e.g. `WHERE id IN (SELECT ... JOIN ...)`)
    // don't false-trigger.
    if has_outer_join(sql) {
        return resolve_multi_table_columns(cols_part, sql, schema_tables, source_file);
    }

    if cols_part == "*" {
        return Ok(table.map(|t| t.columns.clone()).unwrap_or_default());
    }

    let Some(table) = table else {
        return Ok(Vec::new());
    };
    let alias = extract_table_alias(sql, table);
    let mut allowed_prefixes = vec![table.name.as_str()];
    if let Some(alias) = alias {
        allowed_prefixes.push(alias);
    }

    let col_names: Vec<&str> = cols_part.split(',').map(|s| s.trim()).collect();

    col_names
        .iter()
        .map(|&col_expr| -> Result<ColumnDef> {
            resolve_single_table_select_column(col_expr, &allowed_prefixes, table, source_file)
        })
        .collect()
}

// ── Public API ───────────────────────────────────────────────────────────────

pub struct PostgresParser;

impl PostgresParser {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PostgresParser {
    fn default() -> Self {
        Self::new()
    }
}

impl DatabaseParser for PostgresParser {
    fn parse_schema(&self, sql: &str) -> Result<(Vec<TableDef>, Vec<EnumDef>)> {
        let enums = parse_enum_defs(sql);
        let enum_names: HashSet<String> = enums.iter().map(|e| e.name.clone()).collect();
        let tables = parse_schema_tables(sql, &enum_names);
        Ok((tables, enums))
    }

    fn parse_queries(
        &self,
        sql: &str,
        tables: &[TableDef],
        enums: &[EnumDef],
        source_file: &str,
    ) -> Result<Vec<QueryDef>> {
        let _ = enums; // available for future use
        let blocks = split_query_blocks(sql);
        let mut queries = Vec::new();

        for block in blocks {
            let table = find_from_table(&block.sql, tables);
            let param_indices = extract_param_indices(&block.sql);
            let inferred_cols = infer_param_columns(&block.sql);
            let params = build_params(&block.comments, table, param_indices, inferred_cols);
            let returns = resolve_return_columns(&block.sql, table, tables, source_file)?;

            let clean_sql = block
                .sql
                .trim_end()
                .trim_end_matches(';')
                .trim()
                .to_string();

            queries.push(QueryDef {
                name: block.name,
                command: block.command,
                sql: clean_sql,
                params,
                returns,
                source_file: source_file.to_string(),
            });
        }

        Ok(queries)
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{QueryCommand, SqlTypeCategory};
    use crate::parser::DatabaseParser;

    const SCHEMA_SQL: &str = include_str!("../../../../tests/fixtures/schema.sql");
    const QUERIES_SQL: &str = include_str!("../../../../tests/fixtures/queries/users.sql");

    #[test]
    fn parses_enum_type() {
        let parser = PostgresParser::new();
        let (_, enums) = parser.parse_schema(SCHEMA_SQL).unwrap();
        assert_eq!(enums.len(), 1);
        assert_eq!(enums[0].name, "user_status");
        assert_eq!(enums[0].values, vec!["active", "inactive", "banned"]);
    }

    #[test]
    fn parses_users_table() {
        let parser = PostgresParser::new();
        let (tables, _) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let users = tables.iter().find(|t| t.name == "users").unwrap();
        assert_eq!(users.columns.len(), 7);
        assert_eq!(users.primary_key, vec!["id"]);

        let id_col = &users.columns[0];
        assert_eq!(id_col.name, "id");
        assert_eq!(id_col.sql_type.category, SqlTypeCategory::Number);
        assert!(id_col.has_default); // SERIAL has implicit default
        assert!(!id_col.nullable);

        let bio_col = users.columns.iter().find(|c| c.name == "bio").unwrap();
        assert!(bio_col.nullable);

        let tags_col = users.columns.iter().find(|c| c.name == "tags").unwrap();
        assert!(tags_col.sql_type.element_type.is_some());
    }

    #[test]
    fn parses_posts_table() {
        let parser = PostgresParser::new();
        let (tables, _) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let posts = tables.iter().find(|t| t.name == "posts").unwrap();
        assert_eq!(posts.columns.len(), 6);
    }

    #[test]
    fn parses_get_user_query() {
        let parser = PostgresParser::new();
        let (tables, enums) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let queries = parser
            .parse_queries(QUERIES_SQL, &tables, &enums, "queries/users.sql")
            .unwrap();
        let get_user = queries.iter().find(|q| q.name == "GetUser").unwrap();
        assert_eq!(get_user.command, QueryCommand::One);
        assert_eq!(get_user.params.len(), 1);
        assert_eq!(get_user.params[0].name, "id");
        assert_eq!(get_user.returns.len(), 7); // SELECT * returns all columns
    }

    #[test]
    fn parses_list_users_partial_select() {
        let parser = PostgresParser::new();
        let (tables, enums) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let queries = parser
            .parse_queries(QUERIES_SQL, &tables, &enums, "queries/users.sql")
            .unwrap();
        let list_users = queries.iter().find(|q| q.name == "ListUsers").unwrap();
        assert_eq!(list_users.command, QueryCommand::Many);
        assert_eq!(list_users.returns.len(), 3); // SELECT id, name, email
    }

    #[test]
    fn parses_qualified_single_table_select() {
        let parser = PostgresParser::new();
        let (tables, enums) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let sql = "-- name: ListUsersQualified :many\nSELECT users.id, users.name AS user_name FROM users;";
        let queries = parser
            .parse_queries(sql, &tables, &enums, "queries/users.sql")
            .unwrap();
        let query = queries
            .iter()
            .find(|q| q.name == "ListUsersQualified")
            .unwrap();
        assert_eq!(query.returns.len(), 2);
        assert_eq!(query.returns[0].name, "id");
        assert_eq!(query.returns[0].alias, None);
        assert_eq!(query.returns[1].name, "name");
        assert_eq!(query.returns[1].alias.as_deref(), Some("user_name"));
    }

    #[test]
    fn parses_create_user_exec() {
        let parser = PostgresParser::new();
        let (tables, enums) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let queries = parser
            .parse_queries(QUERIES_SQL, &tables, &enums, "queries/users.sql")
            .unwrap();
        let create_user = queries.iter().find(|q| q.name == "CreateUser").unwrap();
        assert_eq!(create_user.command, QueryCommand::Exec);
        assert_eq!(create_user.params.len(), 3);
        assert!(create_user.returns.is_empty());
    }

    #[test]
    fn parses_delete_user_execresult() {
        let parser = PostgresParser::new();
        let (tables, enums) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let queries = parser
            .parse_queries(QUERIES_SQL, &tables, &enums, "queries/users.sql")
            .unwrap();
        let delete_user = queries.iter().find(|q| q.name == "DeleteUser").unwrap();
        assert_eq!(delete_user.command, QueryCommand::ExecResult);
    }

    #[test]
    fn parses_param_overrides() {
        let parser = PostgresParser::new();
        let (tables, enums) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let queries = parser
            .parse_queries(QUERIES_SQL, &tables, &enums, "queries/users.sql")
            .unwrap();
        let date_range = queries
            .iter()
            .find(|q| q.name == "ListUsersByDateRange")
            .unwrap();
        assert_eq!(date_range.params[0].name, "start_date");
        assert_eq!(date_range.params[1].name, "end_date");
    }

    #[test]
    fn resolve_type_maps_common_types() {
        let enums = HashSet::new();

        let text = resolve_sql_type("TEXT", &enums);
        assert_eq!(text.category, SqlTypeCategory::String);

        let int = resolve_sql_type("INTEGER", &enums);
        assert_eq!(int.category, SqlTypeCategory::Number);

        let bool_t = resolve_sql_type("BOOLEAN", &enums);
        assert_eq!(bool_t.category, SqlTypeCategory::Boolean);

        let ts = resolve_sql_type("TIMESTAMP", &enums);
        assert_eq!(ts.category, SqlTypeCategory::Date);

        let json = resolve_sql_type("JSONB", &enums);
        assert_eq!(json.category, SqlTypeCategory::Json);

        let uuid = resolve_sql_type("UUID", &enums);
        assert_eq!(uuid.category, SqlTypeCategory::Uuid);

        let bytea = resolve_sql_type("BYTEA", &enums);
        assert_eq!(bytea.category, SqlTypeCategory::Binary);
    }

    #[test]
    fn resolve_type_array() {
        let enums = HashSet::new();
        let arr = resolve_sql_type("TEXT[]", &enums);
        assert_eq!(arr.category, SqlTypeCategory::String);
        assert!(arr.element_type.is_some());
        assert_eq!(arr.element_type.unwrap().category, SqlTypeCategory::String);
    }

    #[test]
    fn resolve_type_enum() {
        let mut enums = HashSet::new();
        enums.insert("user_status".to_string());
        let t = resolve_sql_type("user_status", &enums);
        assert_eq!(t.category, SqlTypeCategory::Enum);
        assert_eq!(t.enum_name, Some("user_status".to_string()));
    }

    #[test]
    fn infer_insert_params() {
        let sql = "INSERT INTO users (name, email, bio) VALUES ($1, $2, $3)";
        let cols = infer_param_columns(sql);
        assert_eq!(cols.get(&1), Some(&"name".to_string()));
        assert_eq!(cols.get(&2), Some(&"email".to_string()));
        assert_eq!(cols.get(&3), Some(&"bio".to_string()));
    }

    #[test]
    fn infer_where_params() {
        let sql = "SELECT * FROM users WHERE id = $1";
        let cols = infer_param_columns(sql);
        assert_eq!(cols.get(&1), Some(&"id".to_string()));
    }

    #[test]
    fn split_query_blocks_basic() {
        let blocks = split_query_blocks(
            "-- name: GetUser :one\nSELECT * FROM users WHERE id = $1;\n\n-- name: ListUsers :many\nSELECT id, name FROM users;",
        );
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].name, "GetUser");
        assert_eq!(blocks[1].name, "ListUsers");
    }

    #[test]
    fn resolve_parser_postgres() {
        let parser = crate::parser::resolve_parser("postgres");
        assert!(parser.is_ok());
    }

    #[test]
    fn resolve_parser_mysql() {
        let parser = crate::parser::resolve_parser("mysql");
        assert!(parser.is_ok());
    }

    #[test]
    fn resolve_parser_sqlite() {
        let parser = crate::parser::resolve_parser("sqlite");
        assert!(parser.is_ok());
    }

    #[test]
    fn resolve_parser_unknown() {
        let parser = crate::parser::resolve_parser("oracle");
        assert!(parser.is_err());
    }

    // ── INNER JOIN path tests ────────────────────────────────────────────────

    fn join_schema() -> &'static str {
        r#"
        CREATE TABLE users (
          id INTEGER PRIMARY KEY,
          name TEXT NOT NULL,
          org_id INTEGER NOT NULL
        );
        CREATE TABLE orgs (
          id INTEGER PRIMARY KEY,
          slug TEXT NOT NULL
        );
        "#
    }

    #[test]
    fn inner_join_resolves_qualified_columns() {
        let parser = PostgresParser::new();
        let (tables, enums) = parser.parse_schema(join_schema()).unwrap();
        let sql = "-- name: GetUserWithOrg :one\nSELECT users.name, orgs.slug FROM users INNER JOIN orgs ON users.org_id = orgs.id WHERE users.id = $1;";
        let queries = parser.parse_queries(sql, &tables, &enums, "q.sql").unwrap();
        assert_eq!(queries.len(), 1);
        let q = &queries[0];
        assert_eq!(q.returns.len(), 2);
        assert_eq!(q.returns[0].name, "name");
        assert_eq!(q.returns[0].source_table.as_deref(), Some("users"));
        assert_eq!(q.returns[1].name, "slug");
        assert_eq!(q.returns[1].source_table.as_deref(), Some("orgs"));
    }

    #[test]
    fn inner_join_accepts_aliases_and_as() {
        let parser = PostgresParser::new();
        let (tables, enums) = parser.parse_schema(join_schema()).unwrap();
        let sql = "-- name: Listing :many\nSELECT u.id AS user_id, o.slug AS org_slug FROM users u INNER JOIN orgs o ON u.org_id = o.id;";
        let queries = parser.parse_queries(sql, &tables, &enums, "q.sql").unwrap();
        let q = &queries[0];
        assert_eq!(q.returns[0].name, "id");
        assert_eq!(q.returns[0].alias.as_deref(), Some("user_id"));
        assert_eq!(q.returns[0].source_table.as_deref(), Some("users"));
        assert_eq!(q.returns[1].alias.as_deref(), Some("org_slug"));
        assert_eq!(q.returns[1].source_table.as_deref(), Some("orgs"));
    }

    #[test]
    fn inner_join_rejects_select_star() {
        let parser = PostgresParser::new();
        let (tables, enums) = parser.parse_schema(join_schema()).unwrap();
        let sql = "-- name: Everything :many\nSELECT * FROM users INNER JOIN orgs ON users.org_id = orgs.id;";
        let err = parser
            .parse_queries(sql, &tables, &enums, "q.sql")
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("SELECT * across multi-table JOINs")
        );
    }

    #[test]
    fn left_join_rejected_with_v12_pointer() {
        let parser = PostgresParser::new();
        let (tables, enums) = parser.parse_schema(join_schema()).unwrap();
        let sql = "-- name: WithLeft :many\nSELECT users.id FROM users LEFT JOIN orgs ON users.org_id = orgs.id;";
        let err = parser
            .parse_queries(sql, &tables, &enums, "q.sql")
            .unwrap_err();
        assert!(err.to_string().contains("v1.1 supports INNER JOIN only"));
    }

    #[test]
    fn single_table_path_accepts_qualified_selects() {
        // The single-table path now resolves `table.column` against the base
        // table via resolve_single_table_select_column. (Prior to PR #32 this
        // returned a parse error.)
        let parser = PostgresParser::new();
        let (tables, enums) = parser.parse_schema(join_schema()).unwrap();
        let sql = "-- name: GetUserId :one\nSELECT users.id FROM users WHERE users.id = $1;";
        let queries = parser.parse_queries(sql, &tables, &enums, "q.sql").unwrap();
        assert_eq!(queries[0].returns.len(), 1);
        assert_eq!(queries[0].returns[0].name, "id");
    }

    #[test]
    fn join_in_subquery_does_not_route_outer_to_multi_table() {
        // The outer FROM is single-table (`users`). The JOIN lives inside
        // a subquery. The outer query must use the single-table path — if
        // we routed to the multi-table resolver, the unqualified outer
        // `id` select would fail with "requires qualified columns".
        let parser = PostgresParser::new();
        let (tables, enums) = parser.parse_schema(join_schema()).unwrap();
        let sql = "-- name: SubquerySafe :many\nSELECT id FROM users WHERE id IN (SELECT users.id FROM users INNER JOIN orgs ON users.org_id = orgs.id);";
        let queries = parser.parse_queries(sql, &tables, &enums, "q.sql").unwrap();
        assert_eq!(queries[0].returns.len(), 1);
        assert_eq!(queries[0].returns[0].name, "id");
        assert_eq!(queries[0].returns[0].source_table, None);
    }
}
