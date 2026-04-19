//! Multi-table JOIN support scaffolding.
//!
//! This module introduces two helpers that per-dialect parsers (postgres,
//! mysql, sqlite) can call when they detect a JOIN in a query:
//!
//! - [`AliasMap`] — a lookup from alias-or-table-name to a `TableDef`
//!   reference, built by scanning the FROM/JOIN clauses.
//! - [`parse_join_clauses`] — walks a query's FROM clause, returning the
//!   alias map. Errors on OUTER/USING/NATURAL joins with a pointer to the
//!   v1.2 roadmap.
//! - [`resolve_multi_table_select_column`] — given a qualified select
//!   expression like `users.id` or `u.name AS username`, look up the
//!   table and column in the alias map and return a fully-typed
//!   [`ColumnDef`] with `source_table` populated.
//!
//! The helpers are **not yet wired into any dialect parser**. The existing
//! [`ensure_supported_select_expr`](super::ensure_supported_select_expr)
//! guard still rejects qualified selects in every dialect. A follow-up PR
//! per dialect (postgres, mysql, sqlite) will flip each to call into
//! these helpers when JOIN clauses are present.
//!
//! Scope for v1.1: INNER JOIN only, qualified columns only, no `SELECT *`
//! across joins. OUTER JOIN nullability propagation, `USING`, NATURAL
//! JOIN, lateral joins, and self-joins with aliases are v1.2 work — they
//! would require `ColumnDef.nullable` to become per-query-context rather
//! than per-schema.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::error::{Result, SqlcxError};
use crate::ir::{ColumnDef, TableDef};

/// Maps alias (or bare table name) to the underlying `TableDef`. Both the
/// alias and the table name are valid qualifiers for a column, so both
/// are stored when an alias is present.
#[derive(Debug)]
pub struct AliasMap<'a> {
    entries: HashMap<String, &'a TableDef>,
}

impl<'a> AliasMap<'a> {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn insert(&mut self, qualifier: &str, table: &'a TableDef) {
        self.entries.insert(qualifier.to_lowercase(), table);
    }

    pub fn lookup(&self, qualifier: &str) -> Option<&&'a TableDef> {
        self.entries.get(&qualifier.to_lowercase())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<'a> Default for AliasMap<'a> {
    fn default() -> Self {
        Self::new()
    }
}

// Match the tail of a FROM clause up to WHERE/GROUP/ORDER/HAVING/LIMIT or end.
// The captured group is the raw FROM-clause body (including JOINs).
static FROM_CLAUSE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?is)\bFROM\s+(.+?)(?:\bWHERE\b|\bGROUP\s+BY\b|\bORDER\s+BY\b|\bHAVING\b|\bLIMIT\b|\bRETURNING\b|;|$)",
    )
    .unwrap()
});

// Match `<table> [AS] <alias>` segments. Captures: 1=table, 2=alias-or-empty.
static TABLE_REF_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)^\s*([A-Za-z_][A-Za-z0-9_]*)(?:\s+(?:AS\s+)?([A-Za-z_][A-Za-z0-9_]*))?\s*$")
        .unwrap()
});

// Match unsupported join flavors so we can reject with a clear message.
static UNSUPPORTED_JOIN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(LEFT|RIGHT|FULL|OUTER|NATURAL|CROSS)\s+(OUTER\s+)?JOIN\b|\bUSING\s*\(")
        .unwrap()
});

/// Walk a query's FROM clause and return the alias → table mapping.
/// Returns an empty map (no join detected) when the query has no FROM clause.
/// Returns an error for OUTER / USING / NATURAL / CROSS joins with a message
/// pointing to the v1.2 roadmap.
pub fn parse_join_clauses<'a>(
    sql: &str,
    schema_tables: &'a [TableDef],
    source_file: &str,
) -> Result<AliasMap<'a>> {
    let mut map = AliasMap::new();

    let Some(caps) = FROM_CLAUSE_RE.captures(sql) else {
        return Ok(map);
    };
    let from_body = caps.get(1).unwrap().as_str();

    if let Some(bad) = UNSUPPORTED_JOIN_RE.find(from_body) {
        return Err(SqlcxError::ParseError {
            file: source_file.to_string(),
            message: format!(
                "unsupported join flavor `{}`: v1.1 supports INNER JOIN only. \
                 OUTER JOIN nullability propagation, USING, NATURAL, and CROSS \
                 joins are on the v1.2 roadmap.",
                bad.as_str().trim()
            ),
        });
    }

    let inner_join_re = Regex::new(r"(?i)\s+(?:INNER\s+)?JOIN\s+").unwrap();
    let segments: Vec<&str> = inner_join_re.split(from_body).collect();

    for segment in segments {
        // Strip ON conditions: take only the part before `ON` (case-insensitive).
        let ref_part = split_off_on_clause(segment);
        insert_table_ref(&mut map, ref_part, schema_tables, source_file)?;
    }

    Ok(map)
}

fn split_off_on_clause(segment: &str) -> &str {
    let bytes = segment.as_bytes();
    let lower = segment.to_lowercase();
    if let Some(idx) = lower.find(" on ") {
        std::str::from_utf8(&bytes[..idx]).unwrap_or(segment)
    } else {
        segment
    }
}

fn insert_table_ref<'a>(
    map: &mut AliasMap<'a>,
    ref_part: &str,
    schema_tables: &'a [TableDef],
    source_file: &str,
) -> Result<()> {
    let caps = TABLE_REF_RE
        .captures(ref_part)
        .ok_or_else(|| SqlcxError::ParseError {
            file: source_file.to_string(),
            message: format!("could not parse table reference `{}`", ref_part.trim()),
        })?;

    let table_name = caps.get(1).unwrap().as_str();
    let alias = caps.get(2).map(|m| m.as_str());

    let table = schema_tables
        .iter()
        .find(|t| t.name.eq_ignore_ascii_case(table_name))
        .ok_or_else(|| SqlcxError::ParseError {
            file: source_file.to_string(),
            message: format!(
                "table `{}` referenced in FROM/JOIN but not defined in schema",
                table_name
            ),
        })?;

    map.insert(table_name, table);
    if let Some(a) = alias {
        map.insert(a, table);
    }
    Ok(())
}

/// Resolve a qualified select expression (like `users.id` or `u.name AS username`)
/// against an `AliasMap`. Returns a fully-typed [`ColumnDef`] with
/// `source_table` populated so codegen can disambiguate colliding names.
pub fn resolve_multi_table_select_column(
    expr: &str,
    alias_map: &AliasMap<'_>,
    source_file: &str,
) -> Result<ColumnDef> {
    let trimmed = expr.trim();

    // Split optional `AS <alias>` suffix (case-insensitive).
    let (lhs, alias) = split_as_alias(trimmed);

    let (qualifier, col_name) = lhs.split_once('.').ok_or_else(|| SqlcxError::ParseError {
        file: source_file.to_string(),
        message: format!(
            "multi-table resolver requires qualified columns, got `{}`",
            trimmed
        ),
    })?;

    let qualifier = qualifier.trim();
    let col_name = col_name.trim();

    let table = *alias_map
        .lookup(qualifier)
        .ok_or_else(|| SqlcxError::ParseError {
            file: source_file.to_string(),
            message: format!(
                "unknown table qualifier `{}` in expression `{}` — not in FROM/JOIN clause",
                qualifier, trimmed
            ),
        })?;

    let column = table
        .columns
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(col_name))
        .ok_or_else(|| SqlcxError::ParseError {
            file: source_file.to_string(),
            message: format!("column `{}` not found on table `{}`", col_name, table.name),
        })?;

    Ok(ColumnDef {
        name: column.name.clone(),
        alias: alias.map(|a| a.to_string()),
        source_table: Some(table.name.clone()),
        sql_type: column.sql_type.clone(),
        nullable: column.nullable,
        has_default: column.has_default,
    })
}

fn split_as_alias(expr: &str) -> (&str, Option<&str>) {
    let lower = expr.to_lowercase();
    // Locate the ` AS ` separator case-insensitively, then skip exactly
    // 4 bytes (space + A + S + space) in the original string so mixed-case
    // forms like `As`, `aS`, `AS`, `as` are all handled.
    if let Some(idx) = lower.rfind(" as ") {
        let lhs = &expr[..idx];
        let alias = expr[idx + 4..].trim();
        if !alias.is_empty() && alias.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return (lhs.trim(), Some(alias));
        }
    }
    (expr.trim(), None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{SqlType, SqlTypeCategory};

    fn col(name: &str) -> ColumnDef {
        ColumnDef {
            name: name.to_string(),
            alias: None,
            source_table: None,
            sql_type: SqlType {
                raw: "integer".to_string(),
                normalized: "integer".to_string(),
                category: SqlTypeCategory::Number,
                element_type: None,
                enum_name: None,
                enum_values: None,
                json_shape: None,
            },
            nullable: false,
            has_default: false,
        }
    }

    fn table(name: &str, cols: &[&str]) -> TableDef {
        TableDef {
            name: name.to_string(),
            columns: cols.iter().map(|c| col(c)).collect(),
            primary_key: vec![],
            unique_constraints: vec![],
        }
    }

    #[test]
    fn parse_join_clauses_single_table_no_alias() {
        let tables = vec![table("users", &["id", "name"])];
        let map = parse_join_clauses("SELECT id FROM users", &tables, "q.sql").unwrap();
        assert_eq!(map.len(), 1);
        assert!(map.lookup("users").is_some());
    }

    #[test]
    fn parse_join_clauses_single_table_with_alias() {
        let tables = vec![table("users", &["id"])];
        let map = parse_join_clauses("SELECT u.id FROM users u", &tables, "q.sql").unwrap();
        assert_eq!(map.len(), 2);
        assert!(map.lookup("users").is_some());
        assert!(map.lookup("u").is_some());
    }

    #[test]
    fn parse_join_clauses_inner_join() {
        let tables = vec![table("users", &["id"]), table("orgs", &["id", "slug"])];
        let sql = "SELECT u.id, o.slug FROM users u INNER JOIN orgs o ON u.org_id = o.id";
        let map = parse_join_clauses(sql, &tables, "q.sql").unwrap();
        assert!(map.lookup("u").is_some());
        assert!(map.lookup("o").is_some());
        assert_eq!(map.lookup("u").unwrap().name, "users");
        assert_eq!(map.lookup("o").unwrap().name, "orgs");
    }

    #[test]
    fn parse_join_clauses_rejects_left_join() {
        let tables = vec![table("users", &["id"]), table("orgs", &["id"])];
        let sql = "SELECT u.id FROM users u LEFT JOIN orgs o ON u.org_id = o.id";
        let err = parse_join_clauses(sql, &tables, "q.sql").unwrap_err();
        assert!(err.to_string().contains("v1.1 supports INNER JOIN only"));
    }

    #[test]
    fn parse_join_clauses_rejects_using() {
        let tables = vec![table("users", &["id"]), table("orgs", &["id"])];
        let sql = "SELECT * FROM users JOIN orgs USING (id)";
        let err = parse_join_clauses(sql, &tables, "q.sql").unwrap_err();
        assert!(err.to_string().contains("v1.1 supports INNER JOIN only"));
    }

    #[test]
    fn parse_join_clauses_errors_on_unknown_table() {
        let tables = vec![table("users", &["id"])];
        let sql = "SELECT u.id FROM users u INNER JOIN ghost g ON u.x = g.x";
        let err = parse_join_clauses(sql, &tables, "q.sql").unwrap_err();
        assert!(err.to_string().contains("ghost"));
        assert!(err.to_string().contains("not defined in schema"));
    }

    #[test]
    fn resolve_multi_table_by_table_name() {
        let tables = vec![table("users", &["id", "email"])];
        let map = parse_join_clauses("SELECT * FROM users", &tables, "q.sql").unwrap();
        let col = resolve_multi_table_select_column("users.email", &map, "q.sql").unwrap();
        assert_eq!(col.name, "email");
        assert_eq!(col.source_table.as_deref(), Some("users"));
        assert_eq!(col.alias, None);
    }

    #[test]
    fn resolve_multi_table_by_alias() {
        let tables = vec![table("users", &["id"]), table("orgs", &["slug"])];
        let sql = "SELECT u.id, o.slug FROM users u INNER JOIN orgs o ON u.org_id = o.id";
        let map = parse_join_clauses(sql, &tables, "q.sql").unwrap();
        let col = resolve_multi_table_select_column("o.slug", &map, "q.sql").unwrap();
        assert_eq!(col.name, "slug");
        assert_eq!(col.source_table.as_deref(), Some("orgs"));
    }

    #[test]
    fn resolve_multi_table_with_as_alias() {
        let tables = vec![table("users", &["id"])];
        let map = parse_join_clauses("SELECT * FROM users u", &tables, "q.sql").unwrap();
        let col = resolve_multi_table_select_column("u.id AS user_id", &map, "q.sql").unwrap();
        assert_eq!(col.name, "id");
        assert_eq!(col.alias.as_deref(), Some("user_id"));
        assert_eq!(col.source_table.as_deref(), Some("users"));
    }

    #[test]
    fn resolve_multi_table_with_mixed_case_as() {
        let tables = vec![table("users", &["id"])];
        let map = parse_join_clauses("SELECT * FROM users u", &tables, "q.sql").unwrap();
        for form in ["u.id As user_id", "u.id aS user_id", "u.id as user_id"] {
            let col = resolve_multi_table_select_column(form, &map, "q.sql").unwrap();
            assert_eq!(col.name, "id", "form={form}");
            assert_eq!(col.alias.as_deref(), Some("user_id"), "form={form}");
        }
    }

    #[test]
    fn resolve_multi_table_errors_on_unknown_qualifier() {
        let tables = vec![table("users", &["id"])];
        let map = parse_join_clauses("SELECT * FROM users", &tables, "q.sql").unwrap();
        let err = resolve_multi_table_select_column("orgs.id", &map, "q.sql").unwrap_err();
        assert!(err.to_string().contains("unknown table qualifier"));
    }

    #[test]
    fn resolve_multi_table_errors_on_unknown_column() {
        let tables = vec![table("users", &["id"])];
        let map = parse_join_clauses("SELECT * FROM users", &tables, "q.sql").unwrap();
        let err = resolve_multi_table_select_column("users.ghost", &map, "q.sql").unwrap_err();
        assert!(err.to_string().contains("column `ghost` not found"));
    }
}
