use crate::error::Result;
use crate::generator::{GeneratedFile, SchemaGenerator};
use crate::ir::{ColumnDef, EnumDef, JsonShape, Overrides, SqlType, SqlTypeCategory, SqlcxIR};
use crate::utils::{escape_string, pascal_case, snake_case};
use std::collections::BTreeSet;

pub struct PydanticGenerator;

// ── Type mapping ──────────────────────────────────────────────────────────────

fn json_shape_to_python(shape: &JsonShape) -> String {
    match shape {
        JsonShape::String => "str".to_string(),
        JsonShape::Number => "float".to_string(),
        JsonShape::Boolean => "bool".to_string(),
        JsonShape::Object { .. } => "dict[str, Any]".to_string(),
        JsonShape::Array { element } => {
            format!("list[{}]", json_shape_to_python(element))
        }
        JsonShape::Nullable { inner } => {
            format!("{} | None", json_shape_to_python(inner))
        }
    }
}

fn python_type(sql_type: &SqlType, overrides: &Overrides) -> String {
    // Check overrides first
    if let Some(override_type) = overrides.get(&sql_type.normalized) {
        return match override_type.as_str() {
            "string" | "str" => "str".to_string(),
            "number" | "int" => "int".to_string(),
            "float" => "float".to_string(),
            "boolean" | "bool" => "bool".to_string(),
            other => other.to_string(),
        };
    }

    // Inline @enum annotation
    if let Some(enum_values) = &sql_type.enum_values {
        let literals = enum_values
            .iter()
            .map(|v| format!("\"{}\"", escape_string(v)))
            .collect::<Vec<_>>()
            .join(", ");
        return format!("Literal[{}]", literals);
    }

    // Inline @json annotation
    if let Some(json_shape) = &sql_type.json_shape {
        return json_shape_to_python(json_shape);
    }

    // Array type
    if let Some(element_type) = &sql_type.element_type {
        return format!("list[{}]", python_type(element_type, overrides));
    }

    match &sql_type.category {
        SqlTypeCategory::String | SqlTypeCategory::Uuid => "str".to_string(),
        SqlTypeCategory::Number => {
            let raw_upper = sql_type.raw.to_uppercase();
            if raw_upper.contains("REAL")
                || raw_upper.contains("FLOAT")
                || raw_upper.contains("DOUBLE")
                || raw_upper.contains("DECIMAL")
                || raw_upper.contains("NUMERIC")
            {
                "float".to_string()
            } else {
                "int".to_string()
            }
        }
        SqlTypeCategory::Boolean => "bool".to_string(),
        SqlTypeCategory::Date => {
            let raw_upper = sql_type.raw.to_uppercase();
            if raw_upper.contains("TIMESTAMP") {
                "datetime".to_string()
            } else if raw_upper.contains("TIME") {
                "time".to_string()
            } else {
                "date".to_string()
            }
        }
        SqlTypeCategory::Json => "Any".to_string(),
        SqlTypeCategory::Binary => "bytes".to_string(),
        SqlTypeCategory::Enum => {
            if let Some(enum_name) = &sql_type.enum_name {
                pascal_case(enum_name)
            } else {
                "str".to_string()
            }
        }
        SqlTypeCategory::Unknown => "Any".to_string(),
    }
}

fn select_field(col: &ColumnDef, overrides: &Overrides) -> String {
    let base = python_type(&col.sql_type, overrides);
    let field_name = snake_case(&col.name);
    if col.nullable {
        format!("    {}: {} | None", field_name, base)
    } else {
        format!("    {}: {}", field_name, base)
    }
}

fn insert_field(col: &ColumnDef, overrides: &Overrides) -> String {
    let base = python_type(&col.sql_type, overrides);
    let field_name = snake_case(&col.name);
    if col.has_default {
        if col.nullable {
            format!("    {}: {} | None = None", field_name, base)
        } else {
            format!("    {}: {} | None = None", field_name, base)
        }
    } else if col.nullable {
        format!("    {}: {} | None = None", field_name, base)
    } else {
        format!("    {}: {}", field_name, base)
    }
}

// ── Import collection ─────────────────────────────────────────────────────────

fn collect_imports(ir: &SqlcxIR, overrides: &Overrides) -> BTreeSet<String> {
    let mut imports = BTreeSet::new();
    imports.insert("from pydantic import BaseModel".to_string());

    let mut needs_datetime = false;
    let mut needs_time = false;
    let mut needs_date = false;
    let mut needs_any = false;
    let mut needs_literal = false;

    for table in &ir.tables {
        for col in &table.columns {
            collect_type_imports(&col.sql_type, overrides, &mut needs_datetime, &mut needs_time, &mut needs_date, &mut needs_any, &mut needs_literal);
        }
    }

    if !ir.enums.is_empty() {
        imports.insert("from enum import Enum".to_string());
    }

    let mut typing_imports = Vec::new();
    if needs_any {
        typing_imports.push("Any");
    }
    if needs_literal {
        typing_imports.push("Literal");
    }
    if !typing_imports.is_empty() {
        imports.insert(format!("from typing import {}", typing_imports.join(", ")));
    }

    let mut dt_imports = Vec::new();
    if needs_datetime {
        dt_imports.push("datetime");
    }
    if needs_date {
        dt_imports.push("date");
    }
    if needs_time {
        dt_imports.push("time");
    }
    if !dt_imports.is_empty() {
        imports.insert(format!("from datetime import {}", dt_imports.join(", ")));
    }

    imports
}

fn collect_type_imports(
    sql_type: &SqlType,
    overrides: &Overrides,
    needs_datetime: &mut bool,
    needs_time: &mut bool,
    needs_date: &mut bool,
    needs_any: &mut bool,
    needs_literal: &mut bool,
) {
    if overrides.contains_key(&sql_type.normalized) {
        return;
    }

    if sql_type.enum_values.is_some() {
        *needs_literal = true;
        return;
    }

    if sql_type.json_shape.is_some() {
        *needs_any = true;
        return;
    }

    if let Some(elem) = &sql_type.element_type {
        collect_type_imports(elem, overrides, needs_datetime, needs_time, needs_date, needs_any, needs_literal);
        return;
    }

    match sql_type.category {
        SqlTypeCategory::Date => {
            let raw_upper = sql_type.raw.to_uppercase();
            if raw_upper.contains("TIMESTAMP") {
                *needs_datetime = true;
            } else if raw_upper.contains("TIME") {
                *needs_time = true;
            } else {
                *needs_date = true;
            }
        }
        SqlTypeCategory::Json | SqlTypeCategory::Unknown => {
            *needs_any = true;
        }
        _ => {}
    }
}

// ── Generator ─────────────────────────────────────────────────────────────────

fn generate_enum(enum_def: &EnumDef) -> String {
    let name = pascal_case(&enum_def.name);
    let variants: Vec<String> = enum_def
        .values
        .iter()
        .map(|v| {
            let variant_name = v.to_uppercase();
            format!("    {} = \"{}\"", variant_name, escape_string(v))
        })
        .collect();
    format!(
        "class {}(str, Enum):\n{}",
        name,
        variants.join("\n")
    )
}

fn generate_select_model(table: &crate::ir::TableDef, overrides: &Overrides) -> String {
    let name = format!("Select{}", pascal_case(&table.name));
    let fields: Vec<String> = table
        .columns
        .iter()
        .map(|col| select_field(col, overrides))
        .collect();
    format!(
        "class {}(BaseModel):\n{}",
        name,
        fields.join("\n")
    )
}

fn generate_insert_model(table: &crate::ir::TableDef, overrides: &Overrides) -> String {
    let name = format!("Insert{}", pascal_case(&table.name));

    // Required fields first, optional fields last (Python syntax)
    let mut required: Vec<String> = Vec::new();
    let mut optional: Vec<String> = Vec::new();

    for col in &table.columns {
        let field = insert_field(col, overrides);
        if col.has_default || col.nullable {
            optional.push(field);
        } else {
            required.push(field);
        }
    }

    let mut fields = required;
    fields.extend(optional);

    format!(
        "class {}(BaseModel):\n{}",
        name,
        fields.join("\n")
    )
}

impl PydanticGenerator {
    pub fn generate_models_file(&self, ir: &SqlcxIR, overrides: &Overrides) -> String {
        let mut parts: Vec<String> = Vec::new();

        parts.push("# Code generated by sqlcx. DO NOT EDIT.".to_string());

        // Imports
        let imports = collect_imports(ir, overrides);
        parts.push(imports.into_iter().collect::<Vec<_>>().join("\n"));

        // Enums
        for enum_def in &ir.enums {
            parts.push(generate_enum(enum_def));
        }

        // Models
        for table in &ir.tables {
            parts.push(generate_select_model(table, overrides));
            parts.push(generate_insert_model(table, overrides));
        }

        parts.join("\n\n") + "\n"
    }
}

impl SchemaGenerator for PydanticGenerator {
    fn generate(&self, ir: &SqlcxIR, overrides: &Overrides) -> Result<GeneratedFile> {
        Ok(GeneratedFile {
            path: "models.py".to_string(),
            content: self.generate_models_file(ir, overrides),
        })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::*;
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
        let gen = PydanticGenerator;
        let content = gen.generate_models_file(&ir, &HashMap::new());
        assert!(content.contains("from pydantic import BaseModel"));
        assert!(content.contains("from enum import Enum"));
        assert!(content.contains("class UserStatus(str, Enum):"));
        assert!(content.contains("class SelectUsers(BaseModel):"));
        assert!(content.contains("class InsertUsers(BaseModel):"));
        assert!(content.contains("class SelectPosts(BaseModel):"));
        assert!(content.contains("class InsertPosts(BaseModel):"));
        insta::assert_snapshot!("pydantic_models", content);
    }

    #[test]
    fn nullable_column_uses_optional() {
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
        let result = select_field(&col, &HashMap::new());
        assert_eq!(result, "    bio: str | None");
    }

    #[test]
    fn default_column_is_optional_in_insert() {
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
        let result = insert_field(&col, &HashMap::new());
        assert_eq!(result, "    status: str | None = None");
    }

    #[test]
    fn enum_type_uses_pascal_case() {
        let sql_type = SqlType {
            raw: "user_status".to_string(),
            normalized: "user_status".to_string(),
            category: SqlTypeCategory::Enum,
            element_type: None,
            enum_name: Some("user_status".to_string()),
            enum_values: None,
            json_shape: None,
        };
        let result = python_type(&sql_type, &HashMap::new());
        assert_eq!(result, "UserStatus");
    }

    #[test]
    fn array_type_maps_to_list() {
        let sql_type = SqlType {
            raw: "text[]".to_string(),
            normalized: "text[]".to_string(),
            category: SqlTypeCategory::String,
            element_type: Some(Box::new(SqlType {
                raw: "text".to_string(),
                normalized: "text".to_string(),
                category: SqlTypeCategory::String,
                element_type: None,
                enum_name: None,
                enum_values: None,
                json_shape: None,
            })),
            enum_name: None,
            enum_values: None,
            json_shape: None,
        };
        let result = python_type(&sql_type, &HashMap::new());
        assert_eq!(result, "list[str]");
    }

    #[test]
    fn timestamp_maps_to_datetime() {
        let sql_type = SqlType {
            raw: "TIMESTAMP".to_string(),
            normalized: "timestamp".to_string(),
            category: SqlTypeCategory::Date,
            element_type: None,
            enum_name: None,
            enum_values: None,
            json_shape: None,
        };
        let result = python_type(&sql_type, &HashMap::new());
        assert_eq!(result, "datetime");
    }
}
