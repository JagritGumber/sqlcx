use crate::error::Result;
use crate::generator::{GeneratedFile, SchemaGenerator};
use crate::ir::{ColumnDef, Overrides, SqlType, SqlTypeCategory, SqlcxIR};
use crate::utils::pascal_case;

pub struct SerdeStructGenerator;

// ── Type mapping ──────────────────────────────────────────────────────────────

/// Map a SQL type to its Rust type string, considering the raw SQL type for
/// numeric precision.
fn rust_type(sql_type: &SqlType, _overrides: &Overrides) -> String {
    if let Some(elem) = &sql_type.element_type {
        return format!("Vec<{}>", rust_type(elem, _overrides));
    }

    match &sql_type.category {
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

/// Determine the Rust numeric type from the raw SQL type name.
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
        // SERIAL, INT, INTEGER, SMALLINT, TINYINT, MEDIUMINT
        "i32".to_string()
    }
}

/// Determine the Rust chrono type from the raw SQL type name.
fn date_type(raw: &str) -> String {
    let upper = raw.to_uppercase();
    if upper.contains("TIMESTAMP") {
        "chrono::NaiveDateTime".to_string()
    } else if upper.contains("TIME") {
        "chrono::NaiveTime".to_string()
    } else {
        // DATE
        "chrono::NaiveDate".to_string()
    }
}

/// Build a select-row field type: nullable columns get `Option<T>`.
fn select_field_type(col: &ColumnDef, overrides: &Overrides) -> String {
    let base = rust_type(&col.sql_type, overrides);
    if col.nullable {
        format!("Option<{}>", base)
    } else {
        base
    }
}

/// Build an insert-row field type: columns with defaults become `Option<T>`.
fn insert_field_type(col: &ColumnDef, overrides: &Overrides) -> String {
    let base = rust_type(&col.sql_type, overrides);
    if col.has_default || col.nullable {
        format!("Option<{}>", base)
    } else {
        base
    }
}

// ── Import collection ─────────────────────────────────────────────────────────

/// Collect the set of imports needed based on column types.
fn collect_imports(tables: &[crate::ir::TableDef]) -> Vec<String> {
    let mut needs_chrono = false;
    let mut needs_serde_json = false;

    for table in tables {
        for col in &table.columns {
            check_type_imports(&col.sql_type, &mut needs_chrono, &mut needs_serde_json);
        }
    }

    let mut imports = vec!["use serde::{Deserialize, Serialize};".to_string()];
    if needs_chrono {
        imports.push("use chrono;".to_string());
    }
    if needs_serde_json {
        imports.push("use serde_json;".to_string());
    }
    imports
}

fn check_type_imports(sql_type: &SqlType, needs_chrono: &mut bool, needs_serde_json: &mut bool) {
    if let Some(elem) = &sql_type.element_type {
        check_type_imports(elem, needs_chrono, needs_serde_json);
        return;
    }
    match &sql_type.category {
        SqlTypeCategory::Date => *needs_chrono = true,
        SqlTypeCategory::Json | SqlTypeCategory::Unknown => *needs_serde_json = true,
        _ => {}
    }
}

// ── Struct generation ─────────────────────────────────────────────────────────

fn generate_select_struct(table: &crate::ir::TableDef, overrides: &Overrides) -> String {
    let name = pascal_case(&table.name);
    let fields: Vec<String> = table
        .columns
        .iter()
        .map(|col| {
            format!(
                "    pub {}: {},",
                col.name,
                select_field_type(col, overrides)
            )
        })
        .collect();

    format!(
        "#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]\npub struct {} {{\n{}\n}}",
        name,
        fields.join("\n")
    )
}

fn generate_insert_struct(table: &crate::ir::TableDef, overrides: &Overrides) -> String {
    let name = format!("Insert{}", pascal_case(&table.name));
    let fields: Vec<String> = table
        .columns
        .iter()
        .map(|col| {
            format!(
                "    pub {}: {},",
                col.name,
                insert_field_type(col, overrides)
            )
        })
        .collect();

    format!(
        "/// Insert type for {} - columns with defaults are optional\n#[derive(Debug, Clone, Serialize, Deserialize)]\npub struct {} {{\n{}\n}}",
        table.name,
        name,
        fields.join("\n")
    )
}

// ── Generator ─────────────────────────────────────────────────────────────────

impl SerdeStructGenerator {
    pub fn generate_schema_file(&self, ir: &SqlcxIR, overrides: &Overrides) -> String {
        let mut parts: Vec<String> = Vec::new();

        parts.push("// Code generated by sqlcx. DO NOT EDIT.".to_string());

        // Imports
        let imports = collect_imports(&ir.tables);
        parts.push(imports.join("\n"));

        // Structs for each table
        for table in &ir.tables {
            parts.push(generate_select_struct(table, overrides));
            parts.push(generate_insert_struct(table, overrides));
        }

        parts.join("\n\n") + "\n"
    }
}

impl SchemaGenerator for SerdeStructGenerator {
    fn generate(&self, ir: &SqlcxIR, overrides: &Overrides) -> Result<GeneratedFile> {
        Ok(GeneratedFile {
            path: "models.rs".to_string(),
            content: self.generate_schema_file(ir, overrides),
        })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::*;
    use crate::parser::DatabaseParser;
    use crate::parser::postgres::PostgresParser;
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
    fn generates_schema_file() {
        let ir = parse_fixture_ir();
        let gen_ = SerdeStructGenerator;
        let content = gen_.generate_schema_file(&ir, &HashMap::new());
        assert!(content.contains("use serde::{Deserialize, Serialize};"));
        assert!(content.contains("pub struct Users {"));
        assert!(content.contains("pub struct InsertUsers {"));
        assert!(content.contains("pub struct Posts {"));
        assert!(content.contains("pub struct InsertPosts {"));
        assert!(content.contains("sqlx::FromRow"));
        insta::assert_snapshot!("serde_structs_schema", content);
    }

    #[test]
    fn select_field_nullable_wraps_option() {
        let col = ColumnDef {
            name: "bio".to_string(),
            alias: None,
            source_table: None,
            sql_type: SqlType {
                raw: "text".to_string(),
                normalized: "text".to_string(),
                category: SqlTypeCategory::String,
                element_type: None,
                enum_name: None,
                enum_values: None,
                json_shape: None,
            },
            nullable: true,
            has_default: false,
        };
        assert_eq!(select_field_type(&col, &HashMap::new()), "Option<String>");
    }

    #[test]
    fn insert_field_default_wraps_option() {
        let col = ColumnDef {
            name: "status".to_string(),
            alias: None,
            source_table: None,
            sql_type: SqlType {
                raw: "text".to_string(),
                normalized: "text".to_string(),
                category: SqlTypeCategory::String,
                element_type: None,
                enum_name: None,
                enum_values: None,
                json_shape: None,
            },
            nullable: false,
            has_default: true,
        };
        assert_eq!(insert_field_type(&col, &HashMap::new()), "Option<String>");
    }

    #[test]
    fn number_type_mapping() {
        assert_eq!(number_type("SERIAL"), "i32");
        assert_eq!(number_type("INTEGER"), "i32");
        assert_eq!(number_type("SMALLINT"), "i32");
        assert_eq!(number_type("BIGINT"), "i64");
        assert_eq!(number_type("BIGSERIAL"), "i64");
        assert_eq!(number_type("REAL"), "f64");
        assert_eq!(number_type("DOUBLE PRECISION"), "f64");
        assert_eq!(number_type("NUMERIC"), "f64");
    }

    #[test]
    fn date_type_mapping() {
        assert_eq!(date_type("TIMESTAMP"), "chrono::NaiveDateTime");
        assert_eq!(
            date_type("TIMESTAMP WITHOUT TIME ZONE"),
            "chrono::NaiveDateTime"
        );
        assert_eq!(date_type("DATE"), "chrono::NaiveDate");
        assert_eq!(date_type("TIME"), "chrono::NaiveTime");
    }
}
