// SQL→Python type mapping and @dataclass row/params generation.
//
// Identical across all Python drivers today. If a future driver needs to
// diverge (e.g. mysql-connector mapping BIT → int), promote this to a
// trait like the TypeScript layer does.

use crate::ir::{QueryDef, SqlType, SqlTypeCategory};
use crate::utils::pascal_case;

pub fn py_type(sql_type: &SqlType) -> String {
    if let Some(elem) = &sql_type.element_type {
        return format!("list[{}]", py_type(elem));
    }
    match sql_type.category {
        SqlTypeCategory::String | SqlTypeCategory::Uuid | SqlTypeCategory::Enum => {
            "str".to_string()
        }
        SqlTypeCategory::Number => {
            let upper = sql_type.raw.to_uppercase();
            if upper.contains("REAL")
                || upper.contains("FLOAT")
                || upper.contains("DOUBLE")
                || upper.contains("DECIMAL")
                || upper.contains("NUMERIC")
            {
                "float".to_string()
            } else {
                "int".to_string()
            }
        }
        SqlTypeCategory::Boolean => "bool".to_string(),
        SqlTypeCategory::Date => "datetime".to_string(),
        SqlTypeCategory::Json => "Any".to_string(),
        SqlTypeCategory::Binary => "bytes".to_string(),
        SqlTypeCategory::Unknown => "Any".to_string(),
    }
}

pub fn generate_row_class(query: &QueryDef) -> String {
    if query.returns.is_empty() {
        return String::new();
    }
    let class_name = format!("{}Row", pascal_case(&query.name));
    let fields: Vec<String> = query
        .returns
        .iter()
        .map(|col| {
            let name = col.alias.as_deref().unwrap_or(&col.name);
            let ty = py_type(&col.sql_type);
            if col.nullable {
                format!("    {}: {} | None", name, ty)
            } else {
                format!("    {}: {}", name, ty)
            }
        })
        .collect();
    format!("@dataclass\nclass {}:\n{}", class_name, fields.join("\n"))
}

pub fn generate_params_class(query: &QueryDef) -> String {
    if query.params.is_empty() {
        return String::new();
    }
    let class_name = format!("{}Params", pascal_case(&query.name));
    let fields: Vec<String> = query
        .params
        .iter()
        .map(|p| format!("    {}: {}", p.name, py_type(&p.sql_type)))
        .collect();
    format!("@dataclass\nclass {}:\n{}", class_name, fields.join("\n"))
}
