// Bun.sql driver generator.
//
// Thin adapter over `common::generate_driver_files`. bun-sql uses the
// default TS type mapping and the async DatabaseClient body shape, so
// the only driver-specific piece is the client.ts content.

use crate::error::Result;
use crate::generator::typescript::common::{
    TsTypeMap, generate_driver_files, generate_query_functions_file,
};
use crate::generator::{DriverGenerator, GeneratedFile};
use crate::ir::{QueryDef, SqlcxIR};

pub struct BunSqlGenerator;

struct BunSqlTypeMap;
impl TsTypeMap for BunSqlTypeMap {}

impl BunSqlGenerator {
    /// Generate the client.ts file content (DatabaseClient interface + BunSqlClient adapter).
    pub fn generate_client(&self) -> String {
        r#"export interface DatabaseClient {
  query<T>(sql: string, params: unknown[]): Promise<T[]>;
  queryOne<T>(sql: string, params: unknown[]): Promise<T | null>;
  execute(sql: string, params: unknown[]): Promise<{ rowsAffected: number }>;
}

interface BunSqlDriver {
  unsafe(query: string, values?: unknown[]): Promise<any[] & { count: number }>;
}

export class BunSqlClient implements DatabaseClient {
  private sql: BunSqlDriver;

  constructor(sql: BunSqlDriver) {
    this.sql = sql;
  }

  async query<T>(text: string, values?: unknown[]): Promise<T[]> {
    const result = await this.sql.unsafe(text, values);
    return [...result] as T[];
  }

  async queryOne<T>(text: string, values?: unknown[]): Promise<T | null> {
    const rows = await this.query<T>(text, values);
    return rows[0] ?? null;
  }

  async execute(text: string, values?: unknown[]): Promise<{ rowsAffected: number }> {
    const result = await this.sql.unsafe(text, values);
    return { rowsAffected: result.count };
  }
}"#
        .to_string()
    }

    /// Exposed for tests that snapshot the queries file directly.
    pub fn generate_query_functions(&self, queries: &[QueryDef]) -> String {
        generate_query_functions_file(&BunSqlTypeMap, queries)
    }
}

impl DriverGenerator for BunSqlGenerator {
    fn generate(&self, ir: &SqlcxIR) -> Result<Vec<GeneratedFile>> {
        generate_driver_files(&BunSqlTypeMap, self.generate_client(), ir)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::*;
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
    fn generates_client_file() {
        let gen_ = BunSqlGenerator;
        let content = gen_.generate_client();
        assert!(content.contains("export interface DatabaseClient"));
        assert!(content.contains("export class BunSqlClient implements DatabaseClient"));
        insta::assert_snapshot!("bun_sql_client", content);
    }

    #[test]
    fn generates_query_functions() {
        let ir = parse_fixture_ir();
        let gen_ = BunSqlGenerator;
        let content = gen_.generate_query_functions(&ir.queries);
        assert!(content.contains("export async function getUser"));
        assert!(content.contains("export interface GetUserRow"));
        assert!(content.contains("getUserSql"));
        insta::assert_snapshot!("bun_sql_queries", content);
    }
}
