use crate::error::Result;
use crate::generator::{GeneratedFile, SchemaGenerator};
use crate::ir::{ColumnDef, Overrides, SqlType, SqlTypeCategory, SqlcxIR};
use crate::utils::pascal_case;
use std::collections::BTreeSet;

pub struct GoStructGenerator;

// ── Type mapping ──────────────────────────────────────────────────────────────

/// Map a SQL type to a Go type string (non-nullable).
fn go_type(sql_type: &SqlType) -> String {
    if let Some(elem) = &sql_type.element_type {
        return format!("[]{}", go_type(elem));
    }
    match sql_type.category {
        SqlTypeCategory::String | SqlTypeCategory::Uuid | SqlTypeCategory::Enum => {
            "string".to_string()
        }
        SqlTypeCategory::Number => {
            let raw_upper = sql_type.raw.to_uppercase();
            if raw_upper.contains("REAL")
                || raw_upper.contains("FLOAT")
                || raw_upper.contains("DOUBLE")
                || raw_upper.contains("DECIMAL")
                || raw_upper.contains("NUMERIC")
            {
                "float64".to_string()
            } else {
                "int64".to_string()
            }
        }
        SqlTypeCategory::Boolean => "bool".to_string(),
        SqlTypeCategory::Date => "time.Time".to_string(),
        SqlTypeCategory::Json => "json.RawMessage".to_string(),
        SqlTypeCategory::Binary => "[]byte".to_string(),
        SqlTypeCategory::Unknown => "interface{}".to_string(),
    }
}

/// Wrap a Go type for nullable columns (pointer type).
fn nullable_go_type(sql_type: &SqlType) -> String {
    let base = go_type(sql_type);
    // Slices are already nullable in Go, but we use pointer for consistency
    format!("*{}", base)
}

/// Collect Go imports needed for a set of columns.
fn collect_imports(columns: &[ColumnDef]) -> BTreeSet<String> {
    let mut imports = BTreeSet::new();
    for col in columns {
        collect_type_imports(&col.sql_type, &mut imports);
    }
    imports
}

fn collect_type_imports(sql_type: &SqlType, imports: &mut BTreeSet<String>) {
    if let Some(elem) = &sql_type.element_type {
        collect_type_imports(elem, imports);
        return;
    }
    match sql_type.category {
        SqlTypeCategory::Date => {
            imports.insert("time".to_string());
        }
        SqlTypeCategory::Json => {
            imports.insert("encoding/json".to_string());
        }
        _ => {}
    }
}

fn format_imports(imports: &BTreeSet<String>) -> String {
    if imports.is_empty() {
        return String::new();
    }
    let lines: Vec<String> = imports.iter().map(|i| format!("\t\"{}\"", i)).collect();
    format!("import (\n{}\n)\n", lines.join("\n"))
}

// ── Struct generation ─────────────────────────────────────────────────────────

fn generate_select_struct(
    table_name: &str,
    columns: &[ColumnDef],
    _overrides: &Overrides,
) -> String {
    let struct_name = pascal_case(table_name);
    let fields: Vec<String> = columns
        .iter()
        .map(|col| {
            let field_name = pascal_case(&col.name);
            let field_type = if col.nullable {
                nullable_go_type(&col.sql_type)
            } else {
                go_type(&col.sql_type)
            };
            let pad = compute_padding(&field_name, columns, false);
            let type_pad = compute_type_padding(&field_type, columns, false);
            format!(
                "\t{}{}{}{}`db:\"{}\" json:\"{}\"`",
                field_name, pad, field_type, type_pad, col.name, col.name,
            )
        })
        .collect();
    format!(
        "type {} struct {{\n{}\n}}",
        struct_name,
        fields.join("\n")
    )
}

fn generate_insert_struct(
    table_name: &str,
    columns: &[ColumnDef],
    _overrides: &Overrides,
) -> String {
    // Filter to only insertable columns: skip PK with default and auto-timestamps
    let insertable: Vec<&ColumnDef> = columns
        .iter()
        .filter(|col| !(col.has_default && col.name == "id"))
        .filter(|col| !(col.has_default && col.name == "created_at"))
        .collect();

    let struct_name = format!("Insert{}", pascal_case(table_name));
    let fields: Vec<String> = insertable
        .iter()
        .map(|col| {
            let field_name = pascal_case(&col.name);
            let field_type = if col.nullable || col.has_default {
                nullable_go_type(&col.sql_type)
            } else {
                go_type(&col.sql_type)
            };
            let pad = compute_padding_refs(&field_name, &insertable);
            let type_pad = compute_type_padding_refs(&field_type, &insertable);
            format!(
                "\t{}{}{}{}`db:\"{}\" json:\"{}\"`",
                field_name, pad, field_type, type_pad, col.name, col.name,
            )
        })
        .collect();
    format!(
        "// {} has optional fields for columns with defaults\ntype {} struct {{\n{}\n}}",
        struct_name, struct_name,
        fields.join("\n")
    )
}

/// Compute field name padding for alignment.
fn compute_padding(field_name: &str, columns: &[ColumnDef], _nullable_insert: bool) -> String {
    let max_len = columns
        .iter()
        .map(|c| pascal_case(&c.name).len())
        .max()
        .unwrap_or(0);
    let pad = max_len - field_name.len() + 1;
    " ".repeat(pad)
}

fn compute_padding_refs(field_name: &str, columns: &[&ColumnDef]) -> String {
    let max_len = columns
        .iter()
        .map(|c| pascal_case(&c.name).len())
        .max()
        .unwrap_or(0);
    let pad = max_len - field_name.len() + 1;
    " ".repeat(pad)
}

/// Compute type padding for struct tag alignment.
fn compute_type_padding(field_type: &str, columns: &[ColumnDef], _nullable_insert: bool) -> String {
    let max_len = columns
        .iter()
        .map(|c| {
            if c.nullable {
                nullable_go_type(&c.sql_type).len()
            } else {
                go_type(&c.sql_type).len()
            }
        })
        .max()
        .unwrap_or(0);
    let pad = max_len.saturating_sub(field_type.len()) + 1;
    " ".repeat(pad)
}

fn compute_type_padding_refs(field_type: &str, columns: &[&ColumnDef]) -> String {
    let max_len = columns
        .iter()
        .map(|c| {
            if c.nullable || c.has_default {
                nullable_go_type(&c.sql_type).len()
            } else {
                go_type(&c.sql_type).len()
            }
        })
        .max()
        .unwrap_or(0);
    let pad = max_len.saturating_sub(field_type.len()) + 1;
    " ".repeat(pad)
}

// ── Generator ─────────────────────────────────────────────────────────────────

impl GoStructGenerator {
    pub fn generate_models_file(&self, ir: &SqlcxIR, overrides: &Overrides) -> String {
        let mut parts: Vec<String> = Vec::new();

        parts.push("// Code generated by sqlcx. DO NOT EDIT.".to_string());
        parts.push("package db".to_string());

        // Collect all imports
        let mut all_imports = BTreeSet::new();
        for table in &ir.tables {
            let sel_imports = collect_imports(&table.columns);
            let ins_imports = collect_imports(&table.columns);
            all_imports.extend(sel_imports);
            all_imports.extend(ins_imports);
        }
        let imports_str = format_imports(&all_imports);
        if !imports_str.is_empty() {
            parts.push(imports_str);
        }

        for table in &ir.tables {
            parts.push(generate_select_struct(&table.name, &table.columns, overrides));
            parts.push(generate_insert_struct(&table.name, &table.columns, overrides));
        }

        parts.join("\n\n") + "\n"
    }
}

impl SchemaGenerator for GoStructGenerator {
    fn generate(&self, ir: &SqlcxIR, overrides: &Overrides) -> Result<GeneratedFile> {
        Ok(GeneratedFile {
            path: "models.go".to_string(),
            content: self.generate_models_file(ir, overrides),
        })
    }
}

// ── Public helpers for driver generator ───────────────────────────────────────

/// Get the Go type for a column (used by the driver generator for scan types).
pub fn go_column_type(col: &ColumnDef) -> String {
    if col.nullable {
        nullable_go_type(&col.sql_type)
    } else {
        go_type(&col.sql_type)
    }
}

/// Get the base Go type for a SqlType (non-nullable).
pub fn go_base_type(sql_type: &SqlType) -> String {
    go_type(sql_type)
}

/// Collect imports needed for a set of columns.
pub fn go_imports_for_columns(columns: &[ColumnDef]) -> BTreeSet<String> {
    collect_imports(columns)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::postgres::PostgresParser;
    use crate::parser::DatabaseParser;
    use std::collections::HashMap;

    fn parse_fixture_ir() -> SqlcxIR {
        let schema_sql = include_str!("../../../../../tests/fixtures/schema.sql");
        let parser = PostgresParser::new();
        let (tables, enums) = parser.parse_schema(schema_sql).unwrap();
        SqlcxIR {
            tables,
            queries: vec![],
            enums,
        }
    }

    #[test]
    fn generates_models_file() {
        let ir = parse_fixture_ir();
        let gen = GoStructGenerator;
        let content = gen.generate_models_file(&ir, &HashMap::new());
        assert!(content.contains("package db"));
        assert!(content.contains("type Users struct {"));
        assert!(content.contains("type InsertUsers struct {"));
        assert!(content.contains("type Posts struct {"));
        assert!(content.contains("type InsertPosts struct {"));
        insta::assert_snapshot!("go_structs_models", content);
    }
}
