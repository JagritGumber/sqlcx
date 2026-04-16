// tokio-postgres driver generator for Rust

use std::path::Path;

use crate::error::Result;
use crate::generator::{DriverGenerator, GeneratedFile};
use crate::ir::{ColumnDef, QueryCommand, QueryDef, SqlType, SqlTypeCategory, SqlcxIR};
use crate::utils::{pascal_case, snake_case};

pub struct TokioPostgresGenerator;

fn rust_type(sql_type: &SqlType) -> String {
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
        "i32".to_string()
    }
}

fn date_type(raw: &str) -> String {
    let upper = raw.to_uppercase();
    if upper.contains("TIMESTAMP") {
        "chrono::NaiveDateTime".to_string()
    } else if upper.contains("TIME") {
        "chrono::NaiveTime".to_string()
    } else {
        "chrono::NaiveDate".to_string()
    }
}

fn row_field_type(col: &ColumnDef) -> String {
    let base = rust_type(&col.sql_type);
    if col.nullable {
        format!("Option<{}>", base)
    } else {
        base
    }
}

fn param_type(sql_type: &SqlType) -> String {
    let base = rust_type(sql_type);
    match base.as_str() {
        "String" => "&str".to_string(),
        "Vec<u8>" => "&[u8]".to_string(),
        _ => base,
    }
}

fn generate_row_struct(query: &QueryDef) -> String {
    if query.returns.is_empty() {
        return String::new();
    }
    let type_name = format!("{}Row", pascal_case(&query.name));
    let fields: Vec<String> = query
        .returns
        .iter()
        .map(|col| {
            let field_name = col.alias.as_deref().unwrap_or(&col.name);
            format!("    pub {}: {},", field_name, row_field_type(col))
        })
        .collect();
    format!(
        "#[derive(Debug, Clone)]\npub struct {} {{\n{}\n}}",
        type_name,
        fields.join("\n")
    )
}

fn generate_row_from_impl(query: &QueryDef) -> String {
    if query.returns.is_empty() {
        return String::new();
    }
    let type_name = format!("{}Row", pascal_case(&query.name));
    let field_mappings: Vec<String> = query
        .returns
        .iter()
        .enumerate()
        .map(|(i, col)| {
            let field_name = col.alias.as_deref().unwrap_or(&col.name);
            format!("            {}: row.get({}),", field_name, i)
        })
        .collect();
    format!(
        "impl {} {{\n    fn from_row(row: &tokio_postgres::Row) -> Self {{\n        Self {{\n{}\n        }}\n    }}\n}}",
        type_name,
        field_mappings.join("\n")
    )
}

fn generate_query_function(query: &QueryDef) -> String {
    let fn_name = snake_case(&query.name);
    let sql_const_name = format!("{}_SQL", fn_name.to_uppercase());

    let mut parts: Vec<String> = Vec::new();

    parts.push(format!(
        "pub const {}: &str = {:?};",
        sql_const_name, query.sql
    ));

    let row_struct = generate_row_struct(query);
    if !row_struct.is_empty() {
        parts.push(row_struct);
        parts.push(generate_row_from_impl(query));
    }

    let mut params_sig = String::from("client: &tokio_postgres::Client");
    for p in &query.params {
        let ptype = param_type(&p.sql_type);
        params_sig.push_str(&format!(", {}: {}", p.name, ptype));
    }

    let params_array = if query.params.is_empty() {
        "&[]".to_string()
    } else {
        let args: Vec<String> = query
            .params
            .iter()
            .map(|p| format!("&{}", p.name))
            .collect();
        format!("&[{}]", args.join(", "))
    };

    let (return_type, body) = match query.command {
        QueryCommand::One => {
            let type_name = format!("{}Row", pascal_case(&query.name));
            (
                format!("std::result::Result<Option<{}>, tokio_postgres::Error>", type_name),
                format!(
                    "    let row = client.query_opt({}, {}).await?;\n    Ok(row.map(|r| {}::from_row(&r)))",
                    sql_const_name, params_array, type_name
                ),
            )
        }
        QueryCommand::Many => {
            let type_name = format!("{}Row", pascal_case(&query.name));
            (
                format!("std::result::Result<Vec<{}>, tokio_postgres::Error>", type_name),
                format!(
                    "    let rows = client.query({}, {}).await?;\n    Ok(rows.iter().map(|r| {}::from_row(r)).collect())",
                    sql_const_name, params_array, type_name
                ),
            )
        }
        QueryCommand::Exec => (
            "std::result::Result<(), tokio_postgres::Error>".to_string(),
            format!(
                "    client.execute({}, {}).await?;\n    Ok(())",
                sql_const_name, params_array
            ),
        ),
        QueryCommand::ExecResult => (
            "std::result::Result<u64, tokio_postgres::Error>".to_string(),
            format!(
                "    let count = client.execute({}, {}).await?;\n    Ok(count)",
                sql_const_name, params_array
            ),
        ),
    };

    parts.push(format!(
        "pub async fn {}({}) -> {} {{\n{}\n}}",
        fn_name, params_sig, return_type, body
    ));

    parts.join("\n\n")
}

impl TokioPostgresGenerator {
    pub fn generate_client(&self) -> String {
        "// Code generated by sqlcx. DO NOT EDIT.\n\n\
         // This module uses tokio-postgres for database access.\n\
         // Pass a &tokio_postgres::Client to the query functions below."
            .to_string()
    }

    pub fn generate_query_functions(&self, queries: &[QueryDef]) -> String {
        let header = "// Code generated by sqlcx. DO NOT EDIT.";
        let functions: Vec<String> = queries.iter().map(generate_query_function).collect();
        if functions.is_empty() {
            return format!("{header}\n");
        }
        format!("{header}\n\n{}", functions.join("\n\n"))
    }
}

impl DriverGenerator for TokioPostgresGenerator {
    fn generate(&self, ir: &SqlcxIR) -> Result<Vec<GeneratedFile>> {
        let mut files = Vec::new();

        files.push(GeneratedFile {
            path: "client.rs".to_string(),
            content: self.generate_client(),
        });

        let mut grouped: std::collections::BTreeMap<String, Vec<&QueryDef>> =
            std::collections::BTreeMap::new();
        for query in &ir.queries {
            grouped
                .entry(query.source_file.clone())
                .or_default()
                .push(query);
        }
        for (source_file, queries) in &grouped {
            let basename = Path::new(source_file)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            let owned: Vec<QueryDef> = queries.iter().map(|q| (*q).clone()).collect();
            files.push(GeneratedFile {
                path: format!("{}_queries.rs", basename),
                content: self.generate_query_functions(&owned),
            });
        }

        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::*;
    use crate::parser::postgres::PostgresParser;
    use crate::parser::DatabaseParser;

    fn parse_fixture_ir() -> SqlcxIR {
        let schema_sql = include_str!("../../../../../tests/fixtures/schema.sql");
        let queries_sql = include_str!("../../../../../tests/fixtures/queries/users.sql");
        let parser = PostgresParser::new();
        let (tables, enums) = parser.parse_schema(schema_sql).unwrap();
        let queries = parser
            .parse_queries(queries_sql, &tables, &enums, "queries/users.sql")
            .unwrap();
        SqlcxIR {
            tables,
            queries,
            enums,
        }
    }

    #[test]
    fn generates_client_file() {
        let gen = TokioPostgresGenerator;
        let content = gen.generate_client();
        assert!(content.contains("tokio-postgres"));
        assert!(content.contains("DO NOT EDIT"));
        insta::assert_snapshot!("tokio_postgres_client", content);
    }

    #[test]
    fn generates_query_functions() {
        let ir = parse_fixture_ir();
        let gen = TokioPostgresGenerator;
        let content = gen.generate_query_functions(&ir.queries);
        assert!(content.contains("pub async fn get_user"));
        assert!(content.contains("pub struct GetUserRow"));
        assert!(content.contains("query_opt"));
        assert!(content.contains("from_row"));
        insta::assert_snapshot!("tokio_postgres_queries", content);
    }
}
