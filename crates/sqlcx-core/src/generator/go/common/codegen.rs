// Shared codegen helpers for Go drivers.
//
// Row struct, result struct, scan-field list, function signature, and SQL
// escape are identical across database/sql and pgx. Client interfaces and
// query-function bodies stay per-driver.

use crate::generator::go::structs::{go_base_type, go_column_type};
use crate::ir::{ColumnDef, QueryDef};
use crate::utils::pascal_case;

/// Generate the row struct for queries that return non-table columns.
pub fn generate_row_struct(query: &QueryDef) -> Option<String> {
    if query.returns.is_empty() {
        return None;
    }
    let type_name = format!("{}Row", pascal_case(&query.name));
    let fields: Vec<String> = query
        .returns
        .iter()
        .map(|col| {
            let field_name = pascal_case(col.alias.as_deref().unwrap_or(&col.name));
            let field_type = go_column_type(col);
            format!(
                "\t{} {} `db:\"{}\" json:\"{}\"`",
                field_name,
                field_type,
                col.alias.as_deref().unwrap_or(&col.name),
                col.alias.as_deref().unwrap_or(&col.name),
            )
        })
        .collect();
    Some(format!(
        "type {} struct {{\n{}\n}}",
        type_name,
        fields.join("\n")
    ))
}

/// Generate a result struct for :execresult queries.
pub fn generate_result_struct(query: &QueryDef) -> String {
    let type_name = format!("{}Result", pascal_case(&query.name));
    format!("type {} struct {{\n\tRowsAffected int64\n}}", type_name)
}

/// Generate scan fields (&i.FieldName) for a row.
pub fn scan_fields(columns: &[ColumnDef]) -> String {
    columns
        .iter()
        .map(|col| {
            let field_name = pascal_case(col.alias.as_deref().unwrap_or(&col.name));
            format!("&i.{}", field_name)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Generate the function signature params for a query.
pub fn func_params(query: &QueryDef) -> String {
    if query.params.is_empty() {
        return "ctx context.Context".to_string();
    }
    let params: Vec<String> = query
        .params
        .iter()
        .map(|p| {
            let col = ColumnDef {
                name: p.name.clone(),
                alias: None,
                source_table: None,
                sql_type: p.sql_type.clone(),
                nullable: false,
                has_default: false,
            };
            format!("{} {}", p.name, param_go_type(&col))
        })
        .collect();
    format!("ctx context.Context, {}", params.join(", "))
}

/// Generate the args list for ExecContext/QueryContext/Exec/Query calls.
pub fn query_args(query: &QueryDef) -> String {
    if query.params.is_empty() {
        return String::new();
    }
    let args: Vec<String> = query.params.iter().map(|p| p.name.clone()).collect();
    format!(", {}", args.join(", "))
}

/// Escape a SQL string for embedding as a Go string literal.
pub fn escape_sql(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Generate a Go function parameter type from a column (pointer if nullable).
fn param_go_type(col: &ColumnDef) -> String {
    if col.nullable {
        format!("*{}", go_base_type(&col.sql_type))
    } else {
        go_base_type(&col.sql_type)
    }
}
