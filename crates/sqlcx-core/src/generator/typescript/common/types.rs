// SQL→TS type mapping and row/params interface generation.
//
// The `TsTypeMap` trait captures per-driver divergence: mysql2 and
// better-sqlite3 map Binary to `Buffer` instead of `Uint8Array`, and
// better-sqlite3 stores dates/booleans/json as text/number. The defaults
// match the pg/bun-sql behavior.

use crate::ir::{QueryDef, SqlType, SqlTypeCategory};
use crate::utils::pascal_case;

pub trait TsTypeMap {
    fn string_ty(&self) -> &'static str {
        "string"
    }
    fn number_ty(&self) -> &'static str {
        "number"
    }
    fn boolean_ty(&self) -> &'static str {
        "boolean"
    }
    fn date_ty(&self) -> &'static str {
        "Date"
    }
    fn json_ty(&self) -> &'static str {
        "unknown"
    }
    fn binary_ty(&self) -> &'static str {
        "Uint8Array"
    }
    fn unknown_ty(&self) -> &'static str {
        "unknown"
    }
}

pub fn ts_type<M: TsTypeMap + ?Sized>(map: &M, sql_type: &SqlType) -> String {
    if let Some(elem) = &sql_type.element_type {
        return format!("{}[]", ts_type(map, elem));
    }
    match sql_type.category {
        SqlTypeCategory::String | SqlTypeCategory::Uuid | SqlTypeCategory::Enum => {
            map.string_ty().to_string()
        }
        SqlTypeCategory::Number => map.number_ty().to_string(),
        SqlTypeCategory::Boolean => map.boolean_ty().to_string(),
        SqlTypeCategory::Date => map.date_ty().to_string(),
        SqlTypeCategory::Json => map.json_ty().to_string(),
        SqlTypeCategory::Binary => map.binary_ty().to_string(),
        SqlTypeCategory::Unknown => map.unknown_ty().to_string(),
    }
}

pub fn generate_row_type<M: TsTypeMap + ?Sized>(map: &M, query: &QueryDef) -> String {
    if query.returns.is_empty() {
        return String::new();
    }
    let type_name = format!("{}Row", pascal_case(&query.name));
    let fields: Vec<String> = query
        .returns
        .iter()
        .map(|col| {
            let field_name = col.alias.as_deref().unwrap_or(&col.name);
            let ts = ts_type(map, &col.sql_type);
            let nullable = if col.nullable { " | null" } else { "" };
            format!("  {field_name}: {ts}{nullable};")
        })
        .collect();
    format!("export interface {type_name} {{\n{}\n}}", fields.join("\n"))
}

pub fn generate_params_type<M: TsTypeMap + ?Sized>(map: &M, query: &QueryDef) -> String {
    if query.params.is_empty() {
        return String::new();
    }
    let type_name = format!("{}Params", pascal_case(&query.name));
    let fields: Vec<String> = query
        .params
        .iter()
        .map(|p| format!("  {}: {};", p.name, ts_type(map, &p.sql_type)))
        .collect();
    format!("export interface {type_name} {{\n{}\n}}", fields.join("\n"))
}
