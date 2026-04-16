use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::annotations::extract_annotations;
use crate::error::Result;
use crate::ir::{ColumnDef, EnumDef, QueryDef, SqlType, SqlTypeCategory, TableDef};
use crate::parser::{
    build_params, ensure_supported_select_expr, make_unknown_column, split_column_defs,
    split_query_blocks, DatabaseParser,
};

// ── Static regex patterns ────────────────────────────────────────────────────

static BASE_TYPE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(\w+)").unwrap());

static CONSTRAINT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(PRIMARY\s+KEY|CONSTRAINT|UNIQUE|CHECK|FOREIGN\s+KEY)").unwrap()
});

static COL_NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^(?:`(\w+)`|"(\w+)"|\[(\w+)\]|(\w+))\s+"#).unwrap());

static COL_TYPE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(\w+(?:\s*\([^)]*\))?)").unwrap());

static NOT_NULL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bNOT\s+NULL\b").unwrap());

static DEFAULT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bDEFAULT\b").unwrap());

static PK_INLINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bPRIMARY\s+KEY\b").unwrap());

static UNIQUE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bUNIQUE\b").unwrap());

static AUTO_INC_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bAUTOINCREMENT\b").unwrap());

static TABLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?(?:`?(\w+)`?)\s*\(([\s\S]*?)\)\s*;")
        .unwrap()
});

static TABLE_PK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^PRIMARY\s+KEY\s*\(\s*([\w\s,`]+)\s*\)").unwrap());

static INSERT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)INSERT\s+INTO\s+`?\w+`?\s*\(\s*([\w\s,`]+)\s*\)\s*VALUES\s*\(\s*([?,\s]+)\s*\)",
    )
    .unwrap()
});

static WHERE_PARAM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:(\w+)\s*\(\s*(\w+)\s*\)|(\w+))\s*(?:=|!=|<>|<=?|>=?|(?:NOT\s+)?(?:I?LIKE|IN|IS))\s*\?",
    )
    .unwrap()
});

static FROM_TABLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:FROM|INTO|UPDATE)\s+`?(\w+)`?").unwrap());

static SELECT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\s*SELECT\b").unwrap());

static SELECT_COLS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)SELECT\s+([\s\S]+?)\s+FROM\b").unwrap());

static ALIAS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^`?(\w+)`?\s+as\s+`?(\w+)`?$").unwrap());

// -- Type mapping --------------------------------------------------------

fn type_category(normalized: &str) -> SqlTypeCategory {
    match normalized {
        "integer" | "int" | "tinyint" | "smallint" | "mediumint" | "bigint" => {
            SqlTypeCategory::Number
        }
        "real" | "double" | "float" | "decimal" | "numeric" => SqlTypeCategory::Number,
        "text" | "varchar" | "char" | "character" | "clob" | "nchar" | "nvarchar" => {
            SqlTypeCategory::String
        }
        "blob" => SqlTypeCategory::Binary,
        "boolean" | "bool" => SqlTypeCategory::Boolean,
        "datetime" | "timestamp" | "date" | "time" => SqlTypeCategory::Date,
        "json" => SqlTypeCategory::Json,
        _ => SqlTypeCategory::Unknown,
    }
}

fn resolve_sql_type(raw: &str) -> SqlType {
    let trimmed = raw.trim();

    // Strip parenthesized size/precision: VARCHAR(255) -> varchar
    let normalized = BASE_TYPE_RE
        .captures(trimmed)
        .map(|c| c[1].to_lowercase())
        .unwrap_or_else(|| trimmed.to_lowercase());

    let category = type_category(&normalized);

    SqlType {
        raw: trimmed.to_string(),
        normalized,
        category,
        element_type: None,
        enum_name: None,
        enum_values: None,
        json_shape: None,
    }
}

// -- Schema parsing ------------------------------------------------------

struct ParsedColumn {
    col: ColumnDef,
    is_pk: bool,
    is_unique: bool,
}

fn parse_column_line(line: &str) -> Option<ParsedColumn> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    // Skip constraint lines
    if CONSTRAINT_RE.is_match(line) {
        return None;
    }

    // Extract column name — may be backtick or bracket-quoted
    let name_cap = COL_NAME_RE.captures(line)?;
    let col_name = name_cap
        .get(1)
        .or_else(|| name_cap.get(2))
        .or_else(|| name_cap.get(3))
        .or_else(|| name_cap.get(4))?
        .as_str()
        .to_lowercase();
    let after_name = &line[name_cap[0].len()..];

    // Extract the type — simple word with optional (N)
    let (raw_type, rest) = if let Some(cap) = COL_TYPE_RE.captures(after_name) {
        (cap[1].to_string(), &after_name[cap[0].len()..])
    } else {
        ("blob".to_string(), after_name)
    };

    let is_not_null = NOT_NULL_RE.is_match(rest);
    let has_default_kw = DEFAULT_RE.is_match(rest);
    let is_pk = PK_INLINE_RE.is_match(rest);
    let is_unique = UNIQUE_RE.is_match(rest);
    let is_auto_inc = AUTO_INC_RE.is_match(rest);

    let sql_type = resolve_sql_type(&raw_type);

    Some(ParsedColumn {
        col: ColumnDef {
            name: col_name,
            alias: None,
            source_table: None,
            sql_type,
            nullable: !is_not_null,
            has_default: has_default_kw || is_auto_inc,
        },
        is_pk,
        is_unique,
    })
}

fn parse_schema_tables(sql: &str) -> Vec<TableDef> {
    let mut tables = Vec::new();

    for cap in TABLE_RE.captures_iter(sql) {
        let table_name = cap[1].to_lowercase();
        let body = &cap[2];

        let mut columns = Vec::new();
        let mut primary_key: Vec<String> = Vec::new();
        let mut unique_constraints: Vec<Vec<String>> = Vec::new();

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
                    primary_key.push(col.trim().trim_matches('`').to_lowercase());
                }
                continue;
            }

            let Some(mut parsed) = parse_column_line(trimmed) else {
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

// -- Query parsing -------------------------------------------------------

fn extract_param_indices(sql: &str) -> Vec<u32> {
    let mut count = 0u32;
    let mut indices = Vec::new();
    for ch in sql.chars() {
        if ch == '?' {
            count += 1;
            indices.push(count);
        }
    }
    indices
}

fn infer_param_columns(sql: &str) -> HashMap<u32, String> {
    let mut result = HashMap::new();

    // INSERT pattern: INSERT INTO tbl (col1, col2) VALUES (?, ?)
    if let Some(cap) = INSERT_RE.captures(sql) {
        let cols: Vec<String> = cap[1]
            .split(',')
            .map(|s| s.trim().trim_matches('`').to_lowercase())
            .collect();
        let values_str = &cap[2];
        let mut idx = 0u32;
        for ch in values_str.chars() {
            if ch == '?' {
                idx += 1;
                if (idx as usize) <= cols.len() {
                    result.insert(idx, cols[idx as usize - 1].clone());
                }
            }
        }
        return result;
    }

    // WHERE/SET pattern: col = ? or col LIKE ?
    let sql_keywords: std::collections::HashSet<&str> = [
        "not", "and", "or", "where", "set", "when", "then", "else", "case", "between", "exists",
        "any", "all", "some", "having",
    ]
    .into_iter()
    .collect();

    let mut question_positions: Vec<usize> = Vec::new();
    for (i, ch) in sql.char_indices() {
        if ch == '?' {
            question_positions.push(i);
        }
    }

    for cap in WHERE_PARAM_RE.captures_iter(sql) {
        let match_end = cap.get(0).unwrap().end();
        let q_pos = match_end - 1;
        if let Some(idx_0based) = question_positions.iter().position(|&p| p == q_pos) {
            let idx = (idx_0based + 1) as u32;
            if cap.get(1).is_some() && cap.get(2).is_some() {
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

fn resolve_return_columns(
    sql: &str,
    table: Option<&TableDef>,
    source_file: &str,
) -> Result<Vec<ColumnDef>> {
    if !SELECT_RE.is_match(sql) {
        return Ok(Vec::new());
    }

    let Some(cap) = SELECT_COLS_RE.captures(sql) else {
        return Ok(Vec::new());
    };
    let cols_part = cap[1].trim();

    if cols_part == "*" {
        return Ok(table.map(|t| t.columns.clone()).unwrap_or_default());
    }

    let Some(table) = table else {
        return Ok(Vec::new());
    };

    let col_names: Vec<&str> = cols_part.split(',').map(|s| s.trim()).collect();

    col_names
        .iter()
        .map(|&col_expr| -> Result<ColumnDef> {
            ensure_supported_select_expr(col_expr, source_file)?;
            let expr_lower = col_expr.to_lowercase();
            if let Some(alias_cap) = ALIAS_RE.captures(&expr_lower) {
                let actual = &alias_cap[1];
                let alias = alias_cap[2].to_string();
                Ok(table
                    .columns
                    .iter()
                    .find(|c| c.name == actual)
                    .map(|c| {
                        let mut col = c.clone();
                        col.alias = Some(alias);
                        col
                    })
                    .unwrap_or_else(|| make_unknown_column(actual)))
            } else {
                let name = expr_lower.trim_matches('`');
                Ok(table
                    .columns
                    .iter()
                    .find(|c| c.name == name)
                    .cloned()
                    .unwrap_or_else(|| make_unknown_column(name)))
            }
        })
        .collect()
}

// -- Public API ----------------------------------------------------------

pub struct SqliteParser;

impl SqliteParser {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SqliteParser {
    fn default() -> Self {
        Self::new()
    }
}

impl DatabaseParser for SqliteParser {
    fn parse_schema(&self, sql: &str) -> Result<(Vec<TableDef>, Vec<EnumDef>)> {
        // SQLite has no standalone ENUM types
        let tables = parse_schema_tables(sql);
        Ok((tables, Vec::new()))
    }

    fn parse_queries(
        &self,
        sql: &str,
        tables: &[TableDef],
        enums: &[EnumDef],
        source_file: &str,
    ) -> Result<Vec<QueryDef>> {
        let _ = enums;
        let blocks = split_query_blocks(sql);
        let mut queries = Vec::new();

        for block in blocks {
            let table = find_from_table(&block.sql, tables);
            let param_indices = extract_param_indices(&block.sql);
            let inferred_cols = infer_param_columns(&block.sql);
            let params = build_params(&block.comments, table, param_indices, inferred_cols);
            let returns = resolve_return_columns(&block.sql, table, source_file)?;

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

// -- Tests ---------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{QueryCommand, SqlTypeCategory};
    use crate::parser::DatabaseParser;

    const SCHEMA_SQL: &str = include_str!("../../../../tests/fixtures/sqlite_schema.sql");
    const QUERIES_SQL: &str = include_str!("../../../../tests/fixtures/sqlite_queries/users.sql");

    #[test]
    fn parses_users_table() {
        let parser = SqliteParser::new();
        let (tables, _) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let users = tables.iter().find(|t| t.name == "users").unwrap();
        assert_eq!(users.columns.len(), 10);
        assert_eq!(users.primary_key, vec!["id"]);
        let id = &users.columns[0];
        assert_eq!(id.sql_type.category, SqlTypeCategory::Number);
        assert!(id.has_default); // AUTOINCREMENT
    }

    #[test]
    fn parses_text_as_string() {
        let parser = SqliteParser::new();
        let (tables, _) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let users = tables.iter().find(|t| t.name == "users").unwrap();
        let name = users.columns.iter().find(|c| c.name == "name").unwrap();
        assert_eq!(name.sql_type.category, SqlTypeCategory::String);
    }

    #[test]
    fn parses_boolean_convention() {
        let parser = SqliteParser::new();
        let (tables, _) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let users = tables.iter().find(|t| t.name == "users").unwrap();
        let active = users
            .columns
            .iter()
            .find(|c| c.name == "is_active")
            .unwrap();
        assert_eq!(active.sql_type.category, SqlTypeCategory::Boolean);
    }

    #[test]
    fn parses_datetime_convention() {
        let parser = SqliteParser::new();
        let (tables, _) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let users = tables.iter().find(|t| t.name == "users").unwrap();
        let created = users
            .columns
            .iter()
            .find(|c| c.name == "created_at")
            .unwrap();
        assert_eq!(created.sql_type.category, SqlTypeCategory::Date);
    }

    #[test]
    fn parses_blob_as_binary() {
        let parser = SqliteParser::new();
        let (tables, _) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let users = tables.iter().find(|t| t.name == "users").unwrap();
        let avatar = users.columns.iter().find(|c| c.name == "avatar").unwrap();
        assert_eq!(avatar.sql_type.category, SqlTypeCategory::Binary);
    }

    #[test]
    fn parses_nullable_columns() {
        let parser = SqliteParser::new();
        let (tables, _) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let users = tables.iter().find(|t| t.name == "users").unwrap();
        let bio = users.columns.iter().find(|c| c.name == "bio").unwrap();
        assert!(bio.nullable);
        let name = users.columns.iter().find(|c| c.name == "name").unwrap();
        assert!(!name.nullable);
    }

    #[test]
    fn parses_posts_table() {
        let parser = SqliteParser::new();
        let (tables, _) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let posts = tables.iter().find(|t| t.name == "posts").unwrap();
        assert_eq!(posts.columns.len(), 6);
    }

    #[test]
    fn no_enums_in_sqlite() {
        let parser = SqliteParser::new();
        let (_, enums) = parser.parse_schema(SCHEMA_SQL).unwrap();
        assert!(enums.is_empty());
    }

    #[test]
    fn parses_get_user_query() {
        let parser = SqliteParser::new();
        let (tables, enums) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let queries = parser
            .parse_queries(QUERIES_SQL, &tables, &enums, "sqlite_queries/users.sql")
            .unwrap();
        let get_user = queries.iter().find(|q| q.name == "GetUser").unwrap();
        assert_eq!(get_user.command, QueryCommand::One);
        assert_eq!(get_user.params.len(), 1);
        assert_eq!(get_user.params[0].name, "id");
        assert_eq!(get_user.returns.len(), 10);
    }

    #[test]
    fn parses_insert_params() {
        let parser = SqliteParser::new();
        let (tables, enums) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let queries = parser
            .parse_queries(QUERIES_SQL, &tables, &enums, "sqlite_queries/users.sql")
            .unwrap();
        let create = queries.iter().find(|q| q.name == "CreateUser").unwrap();
        assert_eq!(create.command, QueryCommand::Exec);
        assert_eq!(create.params.len(), 3);
    }

    #[test]
    fn parses_param_overrides() {
        let parser = SqliteParser::new();
        let (tables, enums) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let queries = parser
            .parse_queries(QUERIES_SQL, &tables, &enums, "sqlite_queries/users.sql")
            .unwrap();
        let dr = queries
            .iter()
            .find(|q| q.name == "ListUsersByDateRange")
            .unwrap();
        assert_eq!(dr.params[0].name, "start_date");
        assert_eq!(dr.params[1].name, "end_date");
    }
}
