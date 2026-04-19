// SQL→Rust type mapping shared across Rust drivers (sqlx, tokio-postgres).

use crate::ir::{ColumnDef, SqlType, SqlTypeCategory};

/// Map a SQL type to its Rust type for query row structs.
pub fn rust_type(sql_type: &SqlType) -> String {
    if let Some(elem) = &sql_type.element_type {
        return format!("Vec<{}>", rust_type(elem));
    }

    match sql_type.category {
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

pub fn number_type(raw: &str) -> String {
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
        "i32".to_string()
    }
}

pub fn date_type(raw: &str) -> String {
    let upper = raw.to_uppercase();
    if upper.contains("TIMESTAMP") {
        "chrono::NaiveDateTime".to_string()
    } else if upper.contains("TIME") {
        "chrono::NaiveTime".to_string()
    } else {
        "chrono::NaiveDate".to_string()
    }
}

/// Build a field type for a return column: nullable → `Option<T>`.
pub fn row_field_type(col: &ColumnDef) -> String {
    let base = rust_type(&col.sql_type);
    if col.nullable {
        format!("Option<{}>", base)
    } else {
        base
    }
}

/// Map a SQL type to its Rust function parameter type (using references).
pub fn param_type(sql_type: &SqlType) -> String {
    let base = rust_type(sql_type);
    match base.as_str() {
        "String" => "&str".to_string(),
        "Vec<u8>" => "&[u8]".to_string(),
        _ => base,
    }
}
