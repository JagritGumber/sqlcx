pub mod joins;
pub mod mysql;
pub mod postgres;
pub mod sqlite;

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::annotations::extract_annotations;
use crate::error::Result;
use crate::ir::{
    ColumnDef, EnumDef, ParamDef, QueryCommand, QueryDef, SqlType, SqlTypeCategory, TableDef,
};
use crate::param_naming::{RawParam, resolve_param_names};

pub trait DatabaseParser {
    fn parse_schema(&self, sql: &str) -> Result<(Vec<TableDef>, Vec<EnumDef>)>;
    fn parse_queries(
        &self,
        sql: &str,
        tables: &[TableDef],
        enums: &[EnumDef],
        source_file: &str,
    ) -> Result<Vec<QueryDef>>;
}

pub fn resolve_parser(name: &str) -> Result<Box<dyn DatabaseParser>> {
    match name {
        "postgres" => Ok(Box::new(postgres::PostgresParser::new())),
        "mysql" => Ok(Box::new(mysql::MySqlParser::new())),
        "sqlite" => Ok(Box::new(sqlite::SqliteParser::new())),
        _ => Err(crate::error::SqlcxError::UnknownParser(name.to_string())),
    }
}

pub(crate) fn ensure_supported_select_expr(expr: &str, source_file: &str) -> Result<()> {
    let trimmed = expr.trim();
    if trimmed.contains('.') {
        return Err(crate::error::SqlcxError::ParseError {
            file: source_file.to_string(),
            message: format!(
                "qualified select expressions are not supported yet: `{}`",
                trimmed
            ),
        });
    }
    Ok(())
}

// ── Shared regex for split_query_blocks ──────────────────────────────────────

static QUERY_HEADER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"--\s*name:\s*(\w+)\s+:(one|many|execresult|exec)").unwrap());

// ── Shared utilities ─────────────────────────────────────────────────────────

/// Split CREATE TABLE body by commas, respecting nested parens.
pub(crate) fn split_column_defs(body: &str) -> Vec<String> {
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

pub(crate) struct QueryBlock {
    pub name: String,
    pub command: QueryCommand,
    pub sql: String,
    pub comments: String,
}

pub(crate) fn split_query_blocks(sql: &str) -> Vec<QueryBlock> {
    let header_re = &*QUERY_HEADER_RE;

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
        } else if let Some(ref mut block) = current
            && !trimmed.is_empty()
        {
            if !block.sql.is_empty() {
                block.sql.push(' ');
            }
            block.sql.push_str(trimmed);
        }
    }

    if let Some(block) = current {
        blocks.push(block);
    }

    blocks
}

pub(crate) fn build_params(
    comments: &str,
    table: Option<&TableDef>,
    param_indices: Vec<u32>,
    inferred_cols: HashMap<u32, String>,
) -> Vec<ParamDef> {
    if param_indices.is_empty() {
        return Vec::new();
    }

    let (_, ann) = extract_annotations(comments);

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

pub(crate) fn make_unknown_column(name: &str) -> ColumnDef {
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

pub(crate) fn make_unknown_type() -> SqlType {
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
