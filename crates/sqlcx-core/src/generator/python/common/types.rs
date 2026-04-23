// SQL→Python type mapping and @dataclass row/params generation.
//
// The `PyTypeMap` trait captures per-driver divergence. psycopg, asyncpg,
// and mysql-connector use the default mapping (bool/int/datetime/bytes).
// sqlite3 overrides Boolean→int (SQLite stores booleans as 0/1), Date→str,
// and Json→str (SQLite stores these as text).

use crate::ir::{QueryDef, SqlType, SqlTypeCategory};
use crate::utils::pascal_case;

pub trait PyTypeMap {
    fn string_ty(&self) -> &'static str {
        "str"
    }
    fn int_ty(&self) -> &'static str {
        "int"
    }
    fn float_ty(&self) -> &'static str {
        "float"
    }
    fn boolean_ty(&self) -> &'static str {
        "bool"
    }
    fn date_ty(&self) -> &'static str {
        "datetime"
    }
    fn json_ty(&self) -> &'static str {
        "Any"
    }
    fn binary_ty(&self) -> &'static str {
        "bytes"
    }
    fn unknown_ty(&self) -> &'static str {
        "Any"
    }
}

pub fn py_type<M: PyTypeMap + ?Sized>(map: &M, sql_type: &SqlType) -> String {
    if let Some(elem) = &sql_type.element_type {
        return format!("list[{}]", py_type(map, elem));
    }
    match sql_type.category {
        SqlTypeCategory::String | SqlTypeCategory::Uuid | SqlTypeCategory::Enum => {
            map.string_ty().to_string()
        }
        SqlTypeCategory::Number => {
            let upper = sql_type.raw.to_uppercase();
            if upper.contains("REAL")
                || upper.contains("FLOAT")
                || upper.contains("DOUBLE")
                || upper.contains("DECIMAL")
                || upper.contains("NUMERIC")
            {
                map.float_ty().to_string()
            } else {
                map.int_ty().to_string()
            }
        }
        SqlTypeCategory::Boolean => map.boolean_ty().to_string(),
        SqlTypeCategory::Date => map.date_ty().to_string(),
        SqlTypeCategory::Json => map.json_ty().to_string(),
        SqlTypeCategory::Binary => map.binary_ty().to_string(),
        SqlTypeCategory::Unknown => map.unknown_ty().to_string(),
    }
}

pub fn generate_row_class<M: PyTypeMap + ?Sized>(map: &M, query: &QueryDef) -> String {
    if query.returns.is_empty() {
        return String::new();
    }
    let class_name = format!("{}Row", pascal_case(&query.name));
    let fields: Vec<String> = query
        .returns
        .iter()
        .map(|col| {
            let name = col.alias.as_deref().unwrap_or(&col.name);
            let ty = py_type(map, &col.sql_type);
            if col.nullable {
                format!("    {}: {} | None", name, ty)
            } else {
                format!("    {}: {}", name, ty)
            }
        })
        .collect();
    format!("@dataclass\nclass {}:\n{}", class_name, fields.join("\n"))
}

pub fn generate_params_class<M: PyTypeMap + ?Sized>(map: &M, query: &QueryDef) -> String {
    if query.params.is_empty() {
        return String::new();
    }
    let class_name = format!("{}Params", pascal_case(&query.name));
    let fields: Vec<String> = query
        .params
        .iter()
        .map(|p| format!("    {}: {}", p.name, py_type(map, &p.sql_type)))
        .collect();
    format!("@dataclass\nclass {}:\n{}", class_name, fields.join("\n"))
}
