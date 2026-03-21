use crate::error::Result;
use crate::generator::{GeneratedFile, SchemaGenerator};
use crate::ir::{ColumnDef, EnumDef, JsonShape, Overrides, SqlType, SqlTypeCategory, SqlcxIR};
use crate::utils::{escape_string, pascal_case};

pub struct TypeBoxGenerator;

// ── Type mapping ──────────────────────────────────────────────────────────────

fn json_shape_to_typebox(shape: &JsonShape) -> String {
    match shape {
        JsonShape::String => "Type.String()".to_string(),
        JsonShape::Number => "Type.Number()".to_string(),
        JsonShape::Boolean => "Type.Boolean()".to_string(),
        JsonShape::Object { fields } => {
            // Sort keys for deterministic output
            let mut entries: Vec<_> = fields.iter().collect();
            entries.sort_by_key(|(k, _)| k.as_str());
            let inner = entries
                .iter()
                .map(|(key, val)| format!("\"{}\": {}", escape_string(key), json_shape_to_typebox(val)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("Type.Object({{ {} }})", inner)
        }
        JsonShape::Array { element } => {
            format!("Type.Array({})", json_shape_to_typebox(element))
        }
        JsonShape::Nullable { inner } => {
            format!("Type.Union([{}, Type.Null()])", json_shape_to_typebox(inner))
        }
    }
}

pub fn type_box_type(sql_type: &SqlType, overrides: &Overrides) -> String {
    // Check overrides first — e.g., "uuid" → "string" means emit Type.String()
    if let Some(override_type) = overrides.get(&sql_type.normalized) {
        return match override_type.as_str() {
            "string" => "Type.String()".to_string(),
            "number" => "Type.Number()".to_string(),
            "boolean" => "Type.Boolean()".to_string(),
            other => format!("Type.Unknown() /* override: {} */", other),
        };
    }

    // Inline @enum annotation takes precedence
    if let Some(enum_values) = &sql_type.enum_values {
        let literals = enum_values
            .iter()
            .map(|v| format!("Type.Literal(\"{}\")", escape_string(v)))
            .collect::<Vec<_>>()
            .join(", ");
        return format!("Type.Union([{}])", literals);
    }

    // Inline @json annotation takes precedence over generic json category
    if let Some(json_shape) = &sql_type.json_shape {
        return json_shape_to_typebox(json_shape);
    }

    // Array type
    if let Some(element_type) = &sql_type.element_type {
        return format!("Type.Array({})", type_box_type(element_type, overrides));
    }

    match &sql_type.category {
        SqlTypeCategory::String => "Type.String()".to_string(),
        SqlTypeCategory::Number => "Type.Number()".to_string(),
        SqlTypeCategory::Boolean => "Type.Boolean()".to_string(),
        SqlTypeCategory::Date => "Type.Date()".to_string(),
        SqlTypeCategory::Json => "Type.Any()".to_string(),
        SqlTypeCategory::Uuid => "Type.String()".to_string(),
        SqlTypeCategory::Binary => "Type.Uint8Array()".to_string(),
        SqlTypeCategory::Enum => {
            if let Some(enum_name) = &sql_type.enum_name {
                pascal_case(enum_name)
            } else {
                "Type.String()".to_string()
            }
        }
        SqlTypeCategory::Unknown => "Type.Unknown()".to_string(),
    }
}

pub fn select_column(col: &ColumnDef, overrides: &Overrides) -> String {
    let base = type_box_type(&col.sql_type, overrides);
    if col.nullable {
        format!("Type.Union([{}, Type.Null()])", base)
    } else {
        base
    }
}

pub fn insert_column(col: &ColumnDef, overrides: &Overrides) -> String {
    let base = type_box_type(&col.sql_type, overrides);
    if col.has_default {
        if col.nullable {
            format!("Type.Optional(Type.Union([{}, Type.Null()]))", base)
        } else {
            format!("Type.Optional({})", base)
        }
    } else if col.nullable {
        format!("Type.Optional(Type.Union([{}, Type.Null()]))", base)
    } else {
        base
    }
}

fn object_body(columns: &[ColumnDef], overrides: &Overrides, mapper: fn(&ColumnDef, &Overrides) -> String) -> String {
    let fields = columns
        .iter()
        .map(|col| format!("  \"{}\": {}", escape_string(&col.name), mapper(col, overrides)))
        .collect::<Vec<_>>()
        .join(",\n");
    format!("{{\n{}\n}}", fields)
}

// ── Generator ─────────────────────────────────────────────────────────────────

fn generate_imports() -> String {
    "import { Type, type Static } from \"@sinclair/typebox\";\n\n// Requires @sinclair/typebox >= 0.31.0 (for Type.Date and Type.Uint8Array)\n\ntype Prettify<T> = { [K in keyof T]: T[K] } & {};".to_string()
}

fn generate_enum_schema(enum_def: &EnumDef) -> String {
    let name = pascal_case(&enum_def.name);
    let literals = enum_def
        .values
        .iter()
        .map(|v| format!("Type.Literal(\"{}\")", escape_string(v)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("export const {} = Type.Union([{}]);", name, literals)
}

fn generate_select_schema(table: &crate::ir::TableDef, overrides: &Overrides) -> String {
    let name = format!("Select{}", pascal_case(&table.name));
    let body = object_body(&table.columns, overrides, select_column);
    format!("export const {} = Type.Object({});", name, body)
}

fn generate_insert_schema(table: &crate::ir::TableDef, overrides: &Overrides) -> String {
    let name = format!("Insert{}", pascal_case(&table.name));
    let body = object_body(&table.columns, overrides, insert_column);
    format!("export const {} = Type.Object({});", name, body)
}

fn generate_type_alias(name: &str, schema_var_name: &str) -> String {
    format!(
        "export type {} = Prettify<Static<typeof {}>>;",
        name, schema_var_name
    )
}

impl TypeBoxGenerator {
    pub fn generate_schema_file(&self, ir: &SqlcxIR, overrides: &Overrides) -> String {
        let mut parts: Vec<String> = Vec::new();

        parts.push(generate_imports());

        for enum_def in &ir.enums {
            parts.push(generate_enum_schema(enum_def));
        }

        for table in &ir.tables {
            parts.push(generate_select_schema(table, overrides));
            parts.push(generate_insert_schema(table, overrides));
        }

        for table in &ir.tables {
            let select_name = format!("Select{}", pascal_case(&table.name));
            let insert_name = format!("Insert{}", pascal_case(&table.name));
            parts.push(generate_type_alias(&select_name, &select_name));
            parts.push(generate_type_alias(&insert_name, &insert_name));
        }

        for enum_def in &ir.enums {
            let name = pascal_case(&enum_def.name);
            parts.push(generate_type_alias(&name, &name));
        }

        parts.join("\n\n") + "\n"
    }
}

impl SchemaGenerator for TypeBoxGenerator {
    fn generate(&self, ir: &SqlcxIR, overrides: &Overrides) -> Result<GeneratedFile> {
        Ok(GeneratedFile {
            path: "schema.ts".to_string(),
            content: self.generate_schema_file(ir, overrides),
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
    fn generates_schema_file() {
        let ir = parse_fixture_ir();
        let gen = TypeBoxGenerator;
        let content = gen.generate_schema_file(&ir, &HashMap::new());
        // Verify key patterns exist
        assert!(content.contains("import { Type, type Static }"));
        assert!(content.contains("export const UserStatus = Type.Union("));
        assert!(content.contains("export const SelectUsers = Type.Object("));
        assert!(content.contains("export const InsertUsers = Type.Object("));
        assert!(
            content.contains("export type SelectUsers = Prettify<Static<typeof SelectUsers>>;")
        );
        // Snapshot for exact comparison
        insta::assert_snapshot!("typebox_schema", content);
    }

    #[test]
    fn pascal_case_converts_snake_case() {
        assert_eq!(pascal_case("user_status"), "UserStatus");
        assert_eq!(pascal_case("my_table_name"), "MyTableName");
        assert_eq!(pascal_case("users"), "Users");
        assert_eq!(pascal_case(""), "");
    }

    #[test]
    fn select_column_wraps_nullable() {
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
        let result = select_column(&col, &HashMap::new());
        assert_eq!(result, "Type.Union([Type.String(), Type.Null()])");
    }

    #[test]
    fn insert_column_optional_with_default() {
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
        let result = insert_column(&col, &HashMap::new());
        assert_eq!(result, "Type.Optional(Type.String())");
    }

    #[test]
    fn type_box_type_enum_category_uses_pascal_case() {
        let sql_type = SqlType {
            raw: "user_status".to_string(),
            normalized: "user_status".to_string(),
            category: SqlTypeCategory::Enum,
            element_type: None,
            enum_name: Some("user_status".to_string()),
            enum_values: None,
            json_shape: None,
        };
        let result = type_box_type(&sql_type, &HashMap::new());
        assert_eq!(result, "UserStatus");
    }

    #[test]
    fn type_box_type_array_wraps_element() {
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
        let result = type_box_type(&sql_type, &HashMap::new());
        assert_eq!(result, "Type.Array(Type.String())");
    }
}
