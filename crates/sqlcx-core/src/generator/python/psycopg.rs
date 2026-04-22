// psycopg (psycopg3) driver. Emits queries.py only. Named %(name)s
// placeholders + dict params arg. Sync functions against psycopg.Connection.

use crate::error::Result;
use crate::generator::python::common::{
    PyBodyCtx, PyDriverShape, PyTypeMap, generate_driver_files,
};
use crate::generator::{DriverGenerator, GeneratedFile};
use crate::ir::{ParamDef, QueryCommand, QueryDef, SqlcxIR};

pub struct PsycopgGenerator;

impl PyTypeMap for PsycopgGenerator {}

fn rewrite_named(sql: &str, params: &[ParamDef]) -> String {
    let mut result = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' && chars.peek().is_some_and(|ch| ch.is_ascii_digit()) {
            let mut num_str = String::new();
            while chars.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                num_str.push(chars.next().unwrap());
            }
            let idx: u32 = num_str.parse().unwrap_or(0);
            let name = params
                .iter()
                .find(|p| p.index == idx)
                .map(|p| p.name.as_str())
                .unwrap_or("unknown");
            result.push_str(&format!("%({name})s"));
        } else {
            result.push(c);
        }
    }
    result
}

impl PyDriverShape for PsycopgGenerator {
    fn driver_import(&self) -> &'static str {
        "from psycopg import Connection"
    }
    fn connection_type(&self) -> &'static str {
        "Connection"
    }
    fn is_async(&self) -> bool {
        false
    }
    fn rewrite_sql(&self, query: &QueryDef) -> String {
        rewrite_named(&query.sql, &query.params)
    }
    fn build_params_arg(&self, query: &QueryDef) -> String {
        if query.params.is_empty() {
            return "{}".to_string();
        }
        let entries: Vec<String> = query
            .params
            .iter()
            .map(|p| format!("\"{}\": params.{}", p.name, p.name))
            .collect();
        format!("{{{}}}", entries.join(", "))
    }
    fn render_body(&self, ctx: &PyBodyCtx<'_>) -> (String, String) {
        let (sc, rt, pa) = (ctx.sql_const, ctx.row_type, ctx.params_arg);
        match ctx.command {
            QueryCommand::One => (
                format!("{rt} | None"),
                format!(
                    "    cur = conn.execute({sc}, {pa})\n    row = cur.fetchone()\n    if row is None:\n        return None\n    return {rt}(*row)"
                ),
            ),
            QueryCommand::Many => (
                format!("list[{rt}]"),
                format!(
                    "    cur = conn.execute({sc}, {pa})\n    return [{rt}(*row) for row in cur.fetchall()]"
                ),
            ),
            QueryCommand::Exec => ("None".to_string(), format!("    conn.execute({sc}, {pa})")),
            QueryCommand::ExecResult => (
                "int".to_string(),
                format!("    cur = conn.execute({sc}, {pa})\n    return cur.rowcount"),
            ),
        }
    }
}

impl DriverGenerator for PsycopgGenerator {
    fn generate(&self, ir: &SqlcxIR) -> Result<Vec<GeneratedFile>> {
        generate_driver_files(self, ir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::python::common::generate_queries_file;
    use crate::parser::DatabaseParser;
    use crate::parser::postgres::PostgresParser;

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
    fn generates_psycopg_query_functions() {
        let ir = parse_fixture_ir();
        let content = generate_queries_file(&PsycopgGenerator, &ir.queries);
        assert!(content.contains("from psycopg import Connection"));
        assert!(content.contains("def get_user"));
        assert!(content.contains("%(id)s"));
        insta::assert_snapshot!("psycopg_queries", content);
    }
}
