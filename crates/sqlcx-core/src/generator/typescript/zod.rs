use crate::error::Result;
use crate::generator::{GeneratedFile, SchemaGenerator};
use crate::ir::{ColumnDef, EnumDef, JsonShape, Overrides, SqlType, SqlTypeCategory, SqlcxIR};
use crate::utils::{escape_string, pascal_case};

pub struct ZodGenerator;

// ── Type mapping ──────────────────────────────────────────────────────────────

fn json_shape_to_zod(shape: &JsonShape) -> String {
    match shape {
        JsonShape::String => "z.string()".to_string(),
        JsonShape::Number => "z.number()".to_string(),
        JsonShape::Boolean => "z.boolean()".to_string(),
        JsonShape::Object { fields } => {
            // Sort keys for deterministic output
            let mut entries: Vec<_> = fields.iter().collect();
            entries.sort_by_key(|(k, _)| k.as_str());
            let inner = entries
                .iter()
                .map(|(key, val)| format!("\"{}\":{}", escape_string(key), json_shape_to_zod(val)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("z.object({{ {} }})", inner)
        }
        JsonShape::Array { element } => {
            format!("z.array({})", json_shape_to_zod(element))
        }
        JsonShape::Nullable { inner } => {
            format!("{}.nullable()", json_shape_to_zod(inner))
        }
    }
}

fn zod_type(sql_type: &SqlType, overrides: &Overrides) -> String {
    // Check overrides first
    if let Some(override_type) = overrides.get(&sql_type.normalized) {
        return match override_type.as_str() {
            "string" => "z.string()".to_string(),
            "number" => "z.number()".to_string(),
            "boolean" => "z.boolean()".to_string(),
            other => format!("z.unknown() /* override: {} */", other),
        };
    }

    // Inline @enum annotation takes precedence
    if let Some(enum_values) = &sql_type.enum_values {
        let literals = enum_values
            .iter()
            .map(|v| format!("\"{}\"", escape_string(v)))
            .collect::<Vec<_>>()
            .join(", ");
        return format!("z.enum([{}])", literals);
    }

    // Inline @json annotation takes precedence over generic json category
    if let Some(json_shape) = &sql_type.json_shape {
        return json_shape_to_zod(json_shape);
    }

    // Array type
    if let Some(element_type) = &sql_type.element_type {
        return format!("z.array({})", zod_type(element_type, overrides));
    }

    match &sql_type.category {
        SqlTypeCategory::String => "z.string()".to_string(),
        SqlTypeCategory::Number => "z.number()".to_string(),
        SqlTypeCategory::Boolean => "z.boolean()".to_string(),
        SqlTypeCategory::Date => "z.date()".to_string(),
        SqlTypeCategory::Json => "z.unknown()".to_string(),
        SqlTypeCategory::Uuid => "z.string().uuid()".to_string(),
        SqlTypeCategory::Binary => "z.custom<Uint8Array>()".to_string(),
        SqlTypeCategory::Enum => {
            if let Some(enum_name) = &sql_type.enum_name {
                pascal_case(enum_name)
            } else {
                "z.string()".to_string()
            }
        }
        SqlTypeCategory::Unknown => "z.unknown()".to_string(),
    }
}

fn select_column(col: &ColumnDef, overrides: &Overrides) -> String {
    let base = zod_type(&col.sql_type, overrides);
    if col.nullable {
        format!("{}.nullable()", base)
    } else {
        base
    }
}

fn insert_column(col: &ColumnDef, overrides: &Overrides) -> String {
    let base = zod_type(&col.sql_type, overrides);
    if col.has_default {
        if col.nullable {
            format!("{}.nullable().optional()", base)
        } else {
            format!("{}.optional()", base)
        }
    } else if col.nullable {
        format!("{}.nullable().optional()", base)
    } else {
        base
    }
}

fn object_body(
    columns: &[ColumnDef],
    overrides: &Overrides,
    mapper: fn(&ColumnDef, &Overrides) -> String,
) -> String {
    let fields = columns
        .iter()
        .map(|col| {
            format!(
                "  \"{}\": {}",
                escape_string(&col.name),
                mapper(col, overrides)
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    format!("{{\n{}\n}}", fields)
}

// ── Generator ─────────────────────────────────────────────────────────────────

fn generate_imports() -> String {
    "import { z } from \"zod\";\n\ntype Prettify<T> = { [K in keyof T]: T[K] } & {};".to_string()
}

fn generate_enum_schema(enum_def: &EnumDef) -> String {
    let name = pascal_case(&enum_def.name);
    let literals = enum_def
        .values
        .iter()
        .map(|v| format!("\"{}\"", escape_string(v)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("export const {} = z.enum([{}]);", name, literals)
}

fn generate_select_schema(table: &crate::ir::TableDef, overrides: &Overrides) -> String {
    let name = format!("Select{}", pascal_case(&table.name));
    let body = object_body(&table.columns, overrides, select_column);
    format!("export const {} = z.object({});", name, body)
}

fn generate_insert_schema(table: &crate::ir::TableDef, overrides: &Overrides) -> String {
    let name = format!("Insert{}", pascal_case(&table.name));
    let body = object_body(&table.columns, overrides, insert_column);
    format!("export const {} = z.object({});", name, body)
}

fn generate_type_alias(name: &str, schema_var_name: &str) -> String {
    format!(
        "export type {} = Prettify<z.infer<typeof {}>>;",
        name, schema_var_name
    )
}

impl ZodGenerator {
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

impl SchemaGenerator for ZodGenerator {
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
    fn generates_zod_v4_schema() {
        let ir = parse_fixture_ir();
        let gen_ = ZodGenerator;
        let file = gen_.generate(&ir, &HashMap::new()).unwrap();
        assert!(file.content.contains("import { z } from \"zod\""));
        assert!(file.content.contains("z.enum(["));
        assert!(file.content.contains("z.object({"));
        assert!(file.content.contains("z.infer<typeof"));
        insta::assert_snapshot!("zod_v4_schema", file.content);
    }
}
