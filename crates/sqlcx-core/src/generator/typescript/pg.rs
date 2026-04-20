// pg (node-postgres) driver generator.
//
// Thin adapter over `common::generate_driver_files`. pg uses the default
// TS type mapping and the async DatabaseClient body shape, so the only
// driver-specific piece is the client.ts content.

use crate::error::Result;
use crate::generator::typescript::common::{TsTypeMap, generate_driver_files};
use crate::generator::{DriverGenerator, GeneratedFile};
use crate::ir::SqlcxIR;

pub struct PgGenerator;

struct PgTypeMap;
impl TsTypeMap for PgTypeMap {}

impl PgGenerator {
    /// Generate the client.ts file content (DatabaseClient interface + PgClient adapter).
    pub fn generate_client(&self) -> String {
        r#"import { Pool, type QueryResult } from "pg";

export interface DatabaseClient {
  query<T>(sql: string, params: unknown[]): Promise<T[]>;
  queryOne<T>(sql: string, params: unknown[]): Promise<T | null>;
  execute(sql: string, params: unknown[]): Promise<{ rowsAffected: number }>;
}

export class PgClient implements DatabaseClient {
  private pool: Pool;

  constructor(pool: Pool) {
    this.pool = pool;
  }

  async query<T>(text: string, values?: unknown[]): Promise<T[]> {
    const result: QueryResult = await this.pool.query(text, values);
    return result.rows as T[];
  }

  async queryOne<T>(text: string, values?: unknown[]): Promise<T | null> {
    const rows = await this.query<T>(text, values);
    return rows[0] ?? null;
  }

  async execute(text: string, values?: unknown[]): Promise<{ rowsAffected: number }> {
    const result: QueryResult = await this.pool.query(text, values);
    return { rowsAffected: result.rowCount ?? 0 };
  }
}"#
        .to_string()
    }
}

impl DriverGenerator for PgGenerator {
    fn generate(&self, ir: &SqlcxIR) -> Result<Vec<GeneratedFile>> {
        generate_driver_files(&PgTypeMap, self.generate_client(), ir)
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
    fn generates_pg_client() {
        let gen_ = PgGenerator;
        let content = gen_.generate_client();
        assert!(content.contains("import { Pool"));
        assert!(content.contains("export class PgClient implements DatabaseClient"));
        assert!(content.contains("result.rows"));
        assert!(content.contains("result.rowCount"));
        insta::assert_snapshot!("pg_client", content);
    }

    #[test]
    fn generates_pg_query_functions() {
        let ir = parse_fixture_ir();
        let gen_ = PgGenerator;
        let files = gen_.generate(&ir).unwrap();
        let query_file = files
            .iter()
            .find(|f| f.path.ends_with(".queries.ts"))
            .unwrap();
        assert!(query_file.content.contains("export async function getUser"));
        insta::assert_snapshot!("pg_queries", query_file.content);
    }
}
