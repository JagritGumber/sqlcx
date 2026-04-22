// asyncpg driver. Emits queries.py only. Native $N placeholders, positional
// args, async functions against asyncpg.Connection. Rows hydrate via
// dict(row) because asyncpg Records are mapping-like.

use crate::error::Result;
use crate::generator::python::common::{
    PyBodyCtx, PyDriverShape, PyTypeMap, generate_driver_files,
};
use crate::generator::{DriverGenerator, GeneratedFile};
use crate::ir::{QueryCommand, QueryDef, SqlcxIR};

pub struct AsyncpgGenerator;

impl PyTypeMap for AsyncpgGenerator {}

impl PyDriverShape for AsyncpgGenerator {
    fn driver_import(&self) -> &'static str {
        "from asyncpg import Connection"
    }
    fn connection_type(&self) -> &'static str {
        "Connection"
    }
    fn is_async(&self) -> bool {
        true
    }
    fn rewrite_sql(&self, query: &QueryDef) -> String {
        // asyncpg uses native $N — no rewrite.
        query.sql.clone()
    }
    fn build_params_arg(&self, query: &QueryDef) -> String {
        if query.params.is_empty() {
            return String::new();
        }
        // Positional args appended after the SQL const: ", params.a, params.b"
        // in $N order (params already sorted by index).
        let args: Vec<String> = query
            .params
            .iter()
            .map(|p| format!("params.{}", p.name))
            .collect();
        format!(", {}", args.join(", "))
    }
    fn render_body(&self, ctx: &PyBodyCtx<'_>) -> (String, String) {
        let (sc, rt, pa) = (ctx.sql_const, ctx.row_type, ctx.params_arg);
        match ctx.command {
            QueryCommand::One => (
                format!("{rt} | None"),
                format!(
                    "    row = await conn.fetchrow({sc}{pa})\n    if row is None:\n        return None\n    return {rt}(**dict(row))"
                ),
            ),
            QueryCommand::Many => (
                format!("list[{rt}]"),
                format!(
                    "    rows = await conn.fetch({sc}{pa})\n    return [{rt}(**dict(row)) for row in rows]"
                ),
            ),
            QueryCommand::Exec => (
                "None".to_string(),
                format!("    await conn.execute({sc}{pa})"),
            ),
            QueryCommand::ExecResult => (
                "int".to_string(),
                format!(
                    "    result = await conn.execute({sc}{pa})\n    return int(result.split()[-1])"
                ),
            ),
        }
    }
}

impl DriverGenerator for AsyncpgGenerator {
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
    fn generates_asyncpg_query_functions() {
        let ir = parse_fixture_ir();
        let content = generate_queries_file(&AsyncpgGenerator, &ir.queries);
        assert!(content.contains("from asyncpg import Connection"));
        assert!(content.contains("async def get_user"));
        assert!(content.contains("await conn.fetchrow"));
        insta::assert_snapshot!("asyncpg_queries", content);
    }
}
