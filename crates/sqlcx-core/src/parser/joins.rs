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
// Handles: LEFT/RIGHT/FULL/NATURAL/CROSS [INNER|OUTER]? JOIN, plain
// OUTER JOIN, and USING(col) clauses. The optional INNER/OUTER between
// the modifier and JOIN catches forms like `NATURAL INNER JOIN` and
// `LEFT OUTER JOIN` that the previous regex missed.
static UNSUPPORTED_JOIN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(LEFT|RIGHT|FULL|NATURAL|CROSS)\s+(?:(?:INNER|OUTER)\s+)?JOIN\b|\bOUTER\s+JOIN\b|\bUSING\s*\(",
    )
    .unwrap()
});

// Case-insensitive ` ON ` and ` AS ` separators. We use regex rather than
// `to_lowercase().find(...)` because lowercasing can change byte offsets
// for non-ASCII characters, making subsequent slicing of the original
// string panic at non-char-boundary positions.
static ON_SEP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\s+ON\s+").unwrap());
static AS_SEP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\s+AS\s+").unwrap());

// Cheap predicate: matches the JOIN keyword anywhere in a string.
// Kept private — callers should use [`has_outer_join`] instead, which
// scopes the match to the outer FROM body so subquery JOINs don't
// false-positive.
static JOIN_DETECT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bJOIN\b").unwrap());

/// Returns true if the query's *outer* FROM clause contains a JOIN.
/// Run this instead of matching `\bJOIN\b` against the full SQL —
/// matching against the full SQL false-positives on JOINs inside subqueries.
pub fn has_outer_join(sql: &str) -> bool {
    let Some(caps) = FROM_CLAUSE_RE.captures(sql) else {
        return false;
    };
    let from_body = caps.get(1).unwrap().as_str();
    JOIN_DETECT_RE.is_match(from_body)
}

/// Resolve a SELECT column list against a multi-table JOIN context.
/// Shared across dialect parsers (postgres, mysql, sqlite): they detect
/// the JOIN via [`has_outer_join`], pull the columns-part out of the
/// SELECT, and call this function to build the typed `ColumnDef` list.
///
/// Rejects `SELECT *` across joins with a v1.2 pointer — listing
/// qualified columns explicitly is required.
///
/// Also rejects unaliased name collisions across joined tables. If two
/// selected columns share the same effective name (e.g. `users.id` and
/// `orgs.id`) without an explicit `AS` alias, the generated row type
/// would have duplicate fields and the underlying driver (sqlx derive,
/// Go `db` tag, etc) couldn't scan them correctly. Users must write
/// `SELECT users.id AS user_id, orgs.id AS org_id ...`, matching sqlc's
/// convention.
pub fn resolve_multi_table_columns(
    cols_part: &str,
    sql: &str,
    schema_tables: &[TableDef],
    source_file: &str,
) -> Result<Vec<ColumnDef>> {
    if cols_part.trim() == "*" {
        return Err(SqlcxError::ParseError {
            file: source_file.to_string(),
            message:
                "SELECT * across multi-table JOINs is not supported in v1.1 — list qualified columns explicitly (users.id, orgs.slug). `SELECT *` across joins ships in v1.2."
                    .to_string(),
        });
    }

    let alias_map = parse_join_clauses(sql, schema_tables, source_file)?;

    let columns: Vec<ColumnDef> = cols_part
        .split(',')
        .map(|s| resolve_multi_table_select_column(s.trim(), &alias_map, source_file))
        .collect::<Result<_>>()?;

    reject_unaliased_collisions(&columns, source_file)?;

    Ok(columns)
}

fn reject_unaliased_collisions(columns: &[ColumnDef], source_file: &str) -> Result<()> {
    use std::collections::HashMap;

    // Value tuple: (actual column name, source table, whether user supplied an alias).
    // We need the column name separately from the effective field name because
    // when a collision happens via aliases (e.g. `users.id AS uid` and `orgs.id AS uid`),
    // the effective name `uid` is NOT a valid column reference — the error message
    // must fall back to the real column + source table it came from.
    let mut first_seen: HashMap<String, (String, String, bool)> = HashMap::new();
    for col in columns {
        let effective = col.alias.as_deref().unwrap_or(&col.name).to_lowercase();
        if let Some((prev_col, prev_source, prev_had_alias)) = first_seen.get(&effective) {
            let this_source = col.source_table.as_deref().unwrap_or("?").to_string();
            let this_col = &col.name;
            let this_had_alias = col.alias.is_some();

            let message = if *prev_had_alias || this_had_alias {
                format!(
                    "two joined columns produce the same field name `{effective}` — one or \
                     both use an explicit AS alias that collides. Choose distinct aliases \
                     so the generated row type has unique fields."
                )
            } else {
                format!(
                    "joined columns `{prev_source}.{prev_col}` and `{this_source}.{this_col}` \
                     produce the same field name. Add explicit `AS` aliases to disambiguate, \
                     e.g. `{prev_source}.{prev_col} AS {prev_source}_{prev_col}, \
                     {this_source}.{this_col} AS {this_source}_{this_col}`."
                )
            };
            return Err(SqlcxError::ParseError {
                file: source_file.to_string(),
                message,
            });
        }
        first_seen.insert(
            effective,
            (
                col.name.clone(),
                col.source_table.as_deref().unwrap_or("?").to_string(),
                col.alias.is_some(),
            ),
        );
    }
    Ok(())
}

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
    match ON_SEP_RE.find(segment) {
        Some(m) => &segment[..m.start()],
        None => segment,
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
    // Locate the LAST ` AS ` separator case-insensitively using regex,
    // which returns byte positions valid on the original (possibly
    // non-ASCII) string so slicing never panics.
    if let Some(m) = AS_SEP_RE.find_iter(expr).last() {
        let lhs = &expr[..m.start()];
        let alias = expr[m.end()..].trim();
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
    fn split_as_alias_does_not_panic_on_non_ascii() {
        // Input contains non-ASCII bytes before " AS " — previous
        // implementation lowercased the string (which changes byte
        // widths for some scripts) then sliced the original, which
        // could panic at non-char-boundary. Regex-based version is safe.
        let (lhs, alias) = split_as_alias("u.name_İ AS user_name");
        assert_eq!(lhs, "u.name_İ");
        assert_eq!(alias, Some("user_name"));
    }

    #[test]
    fn parse_join_clauses_does_not_panic_on_non_ascii_on_clause() {
        let tables = vec![table("users", &["id"]), table("orgs", &["id"])];
        // The ON condition contains non-ASCII identifiers.
        let sql = "SELECT u.id FROM users u INNER JOIN orgs o ON u.İd = o.id";
        let map = parse_join_clauses(sql, &tables, "q.sql").unwrap();
        assert!(map.lookup("u").is_some());
        assert!(map.lookup("o").is_some());
    }

    #[test]
    fn unsupported_join_rejects_natural_inner_join() {
        let tables = vec![table("users", &["id"]), table("orgs", &["id"])];
        let sql = "SELECT u.id FROM users u NATURAL INNER JOIN orgs o";
        let err = parse_join_clauses(sql, &tables, "q.sql").unwrap_err();
        assert!(err.to_string().contains("v1.1 supports INNER JOIN only"));
    }

    #[test]
    fn unsupported_join_rejects_left_outer_join() {
        let tables = vec![table("users", &["id"]), table("orgs", &["id"])];
        let sql = "SELECT u.id FROM users u LEFT OUTER JOIN orgs o ON u.id = o.id";
        let err = parse_join_clauses(sql, &tables, "q.sql").unwrap_err();
        assert!(err.to_string().contains("v1.1 supports INNER JOIN only"));
    }

    #[test]
    fn has_outer_join_true_when_from_contains_join() {
        assert!(has_outer_join(
            "SELECT u.id FROM users u INNER JOIN orgs o ON u.org_id = o.id"
        ));
    }

    #[test]
    fn has_outer_join_false_when_no_from() {
        assert!(!has_outer_join("INSERT INTO users VALUES (1, 'foo')"));
    }

    #[test]
    fn has_outer_join_false_when_join_only_in_subquery() {
        // The outer FROM has only `users`. The JOIN lives inside a
        // subquery. has_outer_join must NOT be fooled.
        let sql = "SELECT id FROM users WHERE id IN (SELECT user_id FROM orgs INNER JOIN something ON orgs.id = something.org_id)";
        assert!(!has_outer_join(sql));
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

    #[test]
    fn rejects_unaliased_collision() {
        let tables = vec![table("users", &["id"]), table("orgs", &["id"])];
        let sql = "SELECT users.id, orgs.id FROM users INNER JOIN orgs ON users.id = orgs.id";
        let err =
            resolve_multi_table_columns("users.id, orgs.id", sql, &tables, "q.sql").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("produce the same field name"));
        assert!(msg.contains("AS"));
        assert!(msg.contains("users_id"));
    }

    #[test]
    fn accepts_colliding_columns_with_aliases() {
        let tables = vec![table("users", &["id"]), table("orgs", &["id"])];
        let sql = "SELECT users.id AS user_id, orgs.id AS org_id FROM users INNER JOIN orgs ON users.id = orgs.id";
        let cols = resolve_multi_table_columns(
            "users.id AS user_id, orgs.id AS org_id",
            sql,
            &tables,
            "q.sql",
        )
        .unwrap();
        assert_eq!(cols.len(), 2);
        assert_eq!(cols[0].alias.as_deref(), Some("user_id"));
        assert_eq!(cols[1].alias.as_deref(), Some("org_id"));
    }

    #[test]
    fn accepts_distinct_names_without_aliases() {
        let tables = vec![table("users", &["id"]), table("orgs", &["slug"])];
        let sql = "SELECT users.id, orgs.slug FROM users INNER JOIN orgs ON users.id = orgs.id";
        let cols =
            resolve_multi_table_columns("users.id, orgs.slug", sql, &tables, "q.sql").unwrap();
        assert_eq!(cols.len(), 2);
    }

    #[test]
    fn rejects_alias_collision_with_non_column_message() {
        // When users alias two different columns to the same name, the
        // error message must NOT reference the alias as if it were a
        // column (e.g. `users.uid` is nonsensical when uid is an alias).
        let tables = vec![table("users", &["id"]), table("orgs", &["id"])];
        let sql = "SELECT users.id AS uid, orgs.id AS uid FROM users INNER JOIN orgs ON users.id = orgs.id";
        let err =
            resolve_multi_table_columns("users.id AS uid, orgs.id AS uid", sql, &tables, "q.sql")
                .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("field name `uid`"), "msg: {msg}");
        assert!(
            !msg.contains("users.uid"),
            "must not reference alias as column: {msg}"
        );
        assert!(
            !msg.contains("orgs.uid"),
            "must not reference alias as column: {msg}"
        );
    }
}
