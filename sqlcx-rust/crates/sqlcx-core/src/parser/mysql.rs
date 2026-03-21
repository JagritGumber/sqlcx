use std::collections::HashMap;

use regex::Regex;

use crate::annotations::extract_annotations;
use crate::error::Result;
use crate::ir::{
    ColumnDef, EnumDef, ParamDef, QueryCommand, QueryDef, SqlType, SqlTypeCategory, TableDef,
};
use crate::param_naming::{resolve_param_names, RawParam};
use crate::parser::DatabaseParser;

// ── Type mapping ─────────────────────────────────────────────────────────────

fn type_category(normalized: &str) -> Option<SqlTypeCategory> {
    match normalized {
        "text" | "varchar" | "char" | "character" | "tinytext" | "mediumtext" | "longtext" => {
            Some(SqlTypeCategory::String)
        }
        "int" | "integer" | "tinyint" | "smallint" | "mediumint" | "bigint" | "serial"
        | "float" | "double" | "real" | "decimal" | "numeric" => Some(SqlTypeCategory::Number),
        "boolean" | "bool" => Some(SqlTypeCategory::Boolean),
        "date" | "datetime" | "timestamp" | "time" | "year" => Some(SqlTypeCategory::Date),
        "json" => Some(SqlTypeCategory::Json),
        "binary" | "varbinary" | "blob" | "tinyblob" | "mediumblob" | "longblob" => {
            Some(SqlTypeCategory::Binary)
        }
        _ => None,
    }
}

/// Resolve a raw MySQL type string into a SqlType.
/// Handles ENUM('a','b'), TINYINT(1) → Boolean, and standard types.
fn resolve_sql_type(raw: &str) -> SqlType {
    let trimmed = raw.trim();

    // Inline ENUM: ENUM('a', 'b', 'c')
    let enum_re = Regex::new(r"(?i)^ENUM\s*\(\s*((?:'[^']*'(?:\s*,\s*'[^']*')*)?)\s*\)").unwrap();
    if let Some(cap) = enum_re.captures(trimmed) {
        let val_re = Regex::new(r"'([^']*)'").unwrap();
        let values: Vec<String> = val_re
            .captures_iter(&cap[1])
            .map(|v| v[1].to_string())
            .collect();
        return SqlType {
            raw: trimmed.to_string(),
            normalized: "enum".to_string(),
            category: SqlTypeCategory::Enum,
            element_type: None,
            enum_name: None,
            enum_values: Some(values),
            json_shape: None,
        };
    }

    // TINYINT(1) → Boolean
    let tinyint_bool_re = Regex::new(r"(?i)^TINYINT\s*\(\s*1\s*\)").unwrap();
    if tinyint_bool_re.is_match(trimmed) {
        return SqlType {
            raw: trimmed.to_string(),
            normalized: "tinyint(1)".to_string(),
            category: SqlTypeCategory::Boolean,
            element_type: None,
            enum_name: None,
            enum_values: None,
            json_shape: None,
        };
    }

    // Strip parenthesized size/precision: VARCHAR(255) → varchar, DECIMAL(10,2) → decimal
    let base_re = Regex::new(r"(?i)^(\w+)").unwrap();
    let base = base_re
        .captures(trimmed)
        .map(|c| c[1].to_lowercase())
        .unwrap_or_else(|| trimmed.to_lowercase());

    // Strip trailing UNSIGNED/SIGNED/ZEROFILL from the base if present in rest
    let normalized = base.clone();

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

// ── Schema parsing ───────────────────────────────────────────────────────────

/// Split CREATE TABLE body by commas, respecting nested parens.
fn split_column_defs(body: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();

    for ch in body.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 => {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    parts.push(trimmed);
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        parts.push(trimmed);
    }
    parts
}

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
    let constraint_re =
        Regex::new(r"(?i)^(PRIMARY\s+KEY|CONSTRAINT|UNIQUE|CHECK|FOREIGN\s+KEY|KEY\s+)").unwrap();
    if constraint_re.is_match(line) {
        return None;
    }

    // Extract column name — may be backtick-quoted
    let name_re = Regex::new(r"^(?:`(\w+)`|(\w+))\s+").unwrap();
    let name_cap = name_re.captures(line)?;
    let col_name = name_cap
        .get(1)
        .or_else(|| name_cap.get(2))?
        .as_str()
        .to_lowercase();
    let after_name = &line[name_cap[0].len()..];

    // Extract the type — could be ENUM(...), TINYINT(1), or simple word with optional (N)
    let raw_type: String;
    let rest: &str;

    let enum_re = Regex::new(r"(?i)^(ENUM\s*\([^)]*\))").unwrap();
    if let Some(cap) = enum_re.captures(after_name) {
        raw_type = cap[1].to_string();
        rest = &after_name[cap[0].len()..];
    } else {
        // Match type with optional parenthesized params: INT, VARCHAR(255), DECIMAL(10,2)
        let type_re = Regex::new(r"(?i)^(\w+(?:\s*\([^)]*\))?)").unwrap();
        if let Some(cap) = type_re.captures(after_name) {
            raw_type = cap[1].to_string();
            rest = &after_name[cap[0].len()..];
        } else {
            raw_type = "unknown".to_string();
            rest = after_name;
        }
    }

    let not_null_re = Regex::new(r"(?i)\bNOT\s+NULL\b").unwrap();
    let default_re = Regex::new(r"(?i)\bDEFAULT\b").unwrap();
    let pk_re = Regex::new(r"(?i)\bPRIMARY\s+KEY\b").unwrap();
    let unique_re = Regex::new(r"(?i)\bUNIQUE\b").unwrap();
    let auto_inc_re = Regex::new(r"(?i)\bAUTO_INCREMENT\b").unwrap();
    let generated_re = Regex::new(r"(?i)\bGENERATED\s+ALWAYS\s+AS\b").unwrap();

    let is_not_null = not_null_re.is_match(rest);
    let has_default_kw = default_re.is_match(rest);
    let is_pk = pk_re.is_match(rest);
    let is_unique = unique_re.is_match(rest);
    let is_auto_inc = auto_inc_re.is_match(rest);
    let is_generated = generated_re.is_match(rest);

    let sql_type = resolve_sql_type(&raw_type);

    Some(ParsedColumn {
        col: ColumnDef {
            name: col_name,
            alias: None,
            source_table: None,
            sql_type,
            nullable: !is_not_null,
            has_default: has_default_kw || is_auto_inc || is_generated,
        },
        is_pk,
        is_unique,
    })
}

fn parse_schema_tables(sql: &str) -> Vec<TableDef> {
    let table_re = Regex::new(
        r"(?is)CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?(?:`?(\w+)`?)\s*\(([\s\S]*?)\)\s*(?:ENGINE\s*=\s*\w+\s*)?;",
    )
    .unwrap();

    // Fallback: also try without trailing ENGINE/;
    let table_re_fallback = Regex::new(
        r"(?is)CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?(?:`?(\w+)`?)\s*\(([\s\S]*?)\)\s*;",
    )
    .unwrap();

    let mut tables = Vec::new();
    let captures: Vec<_> = table_re.captures_iter(sql).collect();
    let captures = if captures.is_empty() {
        table_re_fallback.captures_iter(sql).collect()
    } else {
        captures
    };

    for cap in &captures {
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
            let pk_re =
                Regex::new(r"(?i)^PRIMARY\s+KEY\s*\(\s*([\w\s,`]+)\s*\)").unwrap();
            if let Some(pk_cap) = pk_re.captures(trimmed) {
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

// ── Query parsing ────────────────────────────────────────────────────────────

struct QueryBlock {
    name: String,
    command: QueryCommand,
    sql: String,
    comments: String,
}

fn split_query_blocks(sql: &str) -> Vec<QueryBlock> {
    let header_re =
        Regex::new(r"--\s*name:\s*(\w+)\s+:(one|many|execresult|exec)").unwrap();

    let lines: Vec<&str> = sql.lines().collect();
    let mut blocks: Vec<QueryBlock> = Vec::new();
    let mut current: Option<QueryBlock> = None;
    let mut comment_buffer = String::new();

    for line in &lines {
        let trimmed = line.trim();

        if let Some(cap) = header_re.captures(trimmed) {
            if let Some(block) = current.take() {
                blocks.push(block);
            }

            let command = match &cap[2] {
                "one" => QueryCommand::One,
                "many" => QueryCommand::Many,
                "execresult" => QueryCommand::ExecResult,
                _ => QueryCommand::Exec,
            };

            let mut comments = comment_buffer.clone();
            comments.push_str(trimmed);
            comments.push('\n');
            comment_buffer.clear();

            current = Some(QueryBlock {
                name: cap[1].to_string(),
                command,
                sql: String::new(),
                comments,
            });
        } else if trimmed.starts_with("--") {
            if let Some(ref mut block) = current {
                block.comments.push_str(trimmed);
                block.comments.push('\n');
            } else {
                comment_buffer.push_str(trimmed);
                comment_buffer.push('\n');
            }
        } else if let Some(ref mut block) = current {
            if !trimmed.is_empty() {
                if !block.sql.is_empty() {
                    block.sql.push(' ');
                }
                block.sql.push_str(trimmed);
            }
        }
    }

    if let Some(block) = current {
        blocks.push(block);
    }

    blocks
}

/// Count `?` placeholders left-to-right, returning 1-based indices.
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

/// For MySQL `?` placeholders, infer which column each `?` maps to.
fn infer_param_columns(sql: &str) -> HashMap<u32, String> {
    let mut result = HashMap::new();

    // INSERT pattern: INSERT INTO tbl (col1, col2) VALUES (?, ?)
    let insert_re = Regex::new(
        r"(?i)INSERT\s+INTO\s+`?\w+`?\s*\(\s*([\w\s,`]+)\s*\)\s*VALUES\s*\(\s*([?,\s]+)\s*\)",
    )
    .unwrap();
    if let Some(cap) = insert_re.captures(sql) {
        let cols: Vec<String> = cap[1]
            .split(',')
            .map(|s| s.trim().trim_matches('`').to_lowercase())
            .collect();
        // Count ?'s in the VALUES clause
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

    // Find column = ? patterns by walking through and counting ?'s
    let where_re = Regex::new(
        r"(?i)(?:(\w+)\s*\(\s*(\w+)\s*\)|(\w+))\s*(?:=|!=|<>|<=?|>=?|(?:NOT\s+)?(?:I?LIKE|IN|IS))\s*\?",
    )
    .unwrap();

    // We need to count ? positions to get the right index
    // Strategy: find all ? positions, then match patterns to assign columns
    let mut question_positions: Vec<usize> = Vec::new();
    for (i, ch) in sql.char_indices() {
        if ch == '?' {
            question_positions.push(i);
        }
    }

    for cap in where_re.captures_iter(sql) {
        // Find which ? this match refers to by position
        let match_end = cap.get(0).unwrap().end();
        // The ? is at match_end - 1
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
    let re = Regex::new(r"(?i)(?:FROM|INTO|UPDATE)\s+`?(\w+)`?").unwrap();
    let cap = re.captures(sql)?;
    let table_name = cap[1].to_lowercase();
    tables.iter().find(|t| t.name == table_name)
}

fn resolve_return_columns(sql: &str, table: Option<&TableDef>) -> Vec<ColumnDef> {
    let select_re = Regex::new(r"(?i)^\s*SELECT\b").unwrap();
    if !select_re.is_match(sql) {
        return Vec::new();
    }

    let cols_re = Regex::new(r"(?i)SELECT\s+([\s\S]+?)\s+FROM\b").unwrap();
    let Some(cap) = cols_re.captures(sql) else {
        return Vec::new();
    };
    let cols_part = cap[1].trim();

    if cols_part == "*" {
        return table.map(|t| t.columns.clone()).unwrap_or_default();
    }

    let Some(table) = table else {
        return Vec::new();
    };

    let col_names: Vec<&str> = cols_part.split(',').map(|s| s.trim()).collect();
    let alias_re = Regex::new(r"(?i)^`?(\w+)`?\s+as\s+`?(\w+)`?$").unwrap();

    col_names
        .iter()
        .map(|&col_expr| {
            let expr_lower = col_expr.to_lowercase();
            if let Some(alias_cap) = alias_re.captures(&expr_lower) {
                let actual = &alias_cap[1];
                let alias = alias_cap[2].to_string();
                table
                    .columns
                    .iter()
                    .find(|c| c.name == actual)
                    .map(|c| {
                        let mut col = c.clone();
                        col.alias = Some(alias);
                        col
                    })
                    .unwrap_or_else(|| make_unknown_column(actual))
            } else {
                let name = expr_lower.trim_matches('`');
                table
                    .columns
                    .iter()
                    .find(|c| c.name == name)
                    .cloned()
                    .unwrap_or_else(|| make_unknown_column(name))
            }
        })
        .collect()
}

fn make_unknown_column(name: &str) -> ColumnDef {
    ColumnDef {
        name: name.to_string(),
        alias: None,
        source_table: None,
        sql_type: SqlType {
            raw: "unknown".to_string(),
            normalized: "unknown".to_string(),
            category: SqlTypeCategory::Unknown,
            element_type: None,
            enum_name: None,
            enum_values: None,
            json_shape: None,
        },
        nullable: true,
        has_default: false,
    }
}

fn make_unknown_type() -> SqlType {
    SqlType {
        raw: "unknown".to_string(),
        normalized: "unknown".to_string(),
        category: SqlTypeCategory::Unknown,
        element_type: None,
        enum_name: None,
        enum_values: None,
        json_shape: None,
    }
}

fn build_params(sql: &str, comments: &str, table: Option<&TableDef>) -> Vec<ParamDef> {
    let param_indices = extract_param_indices(sql);
    if param_indices.is_empty() {
        return Vec::new();
    }

    let (_, ann) = extract_annotations(comments);
    let inferred_cols = infer_param_columns(sql);

    let raw_params: Vec<RawParam> = param_indices
        .iter()
        .map(|&idx| RawParam {
            index: idx,
            column: inferred_cols.get(&idx).cloned(),
            r#override: ann.param_overrides.get(&idx).cloned(),
        })
        .collect();

    let names = resolve_param_names(&raw_params);

    param_indices
        .iter()
        .enumerate()
        .map(|(i, &idx)| {
            let col_name = inferred_cols.get(&idx);
            let sql_type = if let (Some(tbl), Some(cn)) = (table, col_name) {
                tbl.columns
                    .iter()
                    .find(|c| c.name == *cn)
                    .map(|c| c.sql_type.clone())
                    .unwrap_or_else(make_unknown_type)
            } else {
                make_unknown_type()
            };

            ParamDef {
                index: idx,
                name: names[i].clone(),
                sql_type,
            }
        })
        .collect()
}

// ── Public API ───────────────────────────────────────────────────────────────

pub struct MySqlParser;

impl MySqlParser {
    pub fn new() -> Self {
        Self
    }
}

impl DatabaseParser for MySqlParser {
    fn parse_schema(&self, sql: &str) -> Result<(Vec<TableDef>, Vec<EnumDef>)> {
        // MySQL has no standalone CREATE TYPE ... AS ENUM; enums are inline on columns
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
            let params = build_params(&block.sql, &block.comments, table);
            let returns = resolve_return_columns(&block.sql, table);

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

    const SCHEMA_SQL: &str = include_str!("../../../../tests/fixtures/mysql_schema.sql");
    const QUERIES_SQL: &str = include_str!("../../../../tests/fixtures/mysql_queries/users.sql");

    #[test]
    fn parses_users_table() {
        let parser = MySqlParser::new();
        let (tables, _) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let users = tables.iter().find(|t| t.name == "users").unwrap();
        assert_eq!(users.columns.len(), 11);
        assert_eq!(users.primary_key, vec!["id"]);
        let id = &users.columns[0];
        assert_eq!(id.sql_type.category, SqlTypeCategory::Number);
        assert!(id.has_default); // AUTO_INCREMENT
    }

    #[test]
    fn parses_inline_enum() {
        let parser = MySqlParser::new();
        let (tables, _) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let users = tables.iter().find(|t| t.name == "users").unwrap();
        let role = users.columns.iter().find(|c| c.name == "role").unwrap();
        assert_eq!(role.sql_type.category, SqlTypeCategory::Enum);
        assert_eq!(
            role.sql_type.enum_values,
            Some(vec!["admin".into(), "user".into(), "guest".into()])
        );
    }

    #[test]
    fn parses_tinyint_as_boolean() {
        let parser = MySqlParser::new();
        let (tables, _) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let users = tables.iter().find(|t| t.name == "users").unwrap();
        let active = users.columns.iter().find(|c| c.name == "is_active").unwrap();
        assert_eq!(active.sql_type.category, SqlTypeCategory::Boolean);
    }

    #[test]
    fn parses_json_column() {
        let parser = MySqlParser::new();
        let (tables, _) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let users = tables.iter().find(|t| t.name == "users").unwrap();
        let prefs = users.columns.iter().find(|c| c.name == "preferences").unwrap();
        assert_eq!(prefs.sql_type.category, SqlTypeCategory::Json);
        assert!(prefs.nullable);
    }

    #[test]
    fn parses_blob_as_binary() {
        let parser = MySqlParser::new();
        let (tables, _) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let users = tables.iter().find(|t| t.name == "users").unwrap();
        let avatar = users.columns.iter().find(|c| c.name == "avatar").unwrap();
        assert_eq!(avatar.sql_type.category, SqlTypeCategory::Binary);
    }

    #[test]
    fn parses_generated_column_as_default() {
        let parser = MySqlParser::new();
        let (tables, _) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let users = tables.iter().find(|t| t.name == "users").unwrap();
        let full_name = users.columns.iter().find(|c| c.name == "full_name").unwrap();
        assert!(full_name.has_default);
    }

    #[test]
    fn parses_posts_table() {
        let parser = MySqlParser::new();
        let (tables, _) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let posts = tables.iter().find(|t| t.name == "posts").unwrap();
        assert_eq!(posts.columns.len(), 6);
    }

    #[test]
    fn parses_query_with_positional_params() {
        let parser = MySqlParser::new();
        let (tables, enums) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let queries = parser
            .parse_queries(QUERIES_SQL, &tables, &enums, "mysql_queries/users.sql")
            .unwrap();
        let get_user = queries.iter().find(|q| q.name == "GetUser").unwrap();
        assert_eq!(get_user.command, QueryCommand::One);
        assert_eq!(get_user.params.len(), 1);
        assert_eq!(get_user.params[0].name, "id");
        assert_eq!(get_user.returns.len(), 11);
    }

    #[test]
    fn parses_insert_params() {
        let parser = MySqlParser::new();
        let (tables, enums) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let queries = parser
            .parse_queries(QUERIES_SQL, &tables, &enums, "mysql_queries/users.sql")
            .unwrap();
        let create = queries.iter().find(|q| q.name == "CreateUser").unwrap();
        assert_eq!(create.command, QueryCommand::Exec);
        assert_eq!(create.params.len(), 3);
    }

    #[test]
    fn parses_param_overrides() {
        let parser = MySqlParser::new();
        let (tables, enums) = parser.parse_schema(SCHEMA_SQL).unwrap();
        let queries = parser
            .parse_queries(QUERIES_SQL, &tables, &enums, "mysql_queries/users.sql")
            .unwrap();
        let dr = queries.iter().find(|q| q.name == "ListUsersByDateRange").unwrap();
        assert_eq!(dr.params[0].name, "start_date");
        assert_eq!(dr.params[1].name, "end_date");
    }
}
