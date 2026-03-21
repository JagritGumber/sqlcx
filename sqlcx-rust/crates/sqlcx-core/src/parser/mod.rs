pub mod mysql;
pub mod postgres;

use crate::error::Result;
use crate::ir::{EnumDef, QueryDef, TableDef};

pub trait DatabaseParser {
    fn parse_schema(&self, sql: &str) -> Result<(Vec<TableDef>, Vec<EnumDef>)>;
    fn parse_queries(
        &self,
        sql: &str,
        tables: &[TableDef],
        enums: &[EnumDef],
        source_file: &str,
    ) -> Result<Vec<QueryDef>>;
}

pub fn resolve_parser(name: &str) -> Result<Box<dyn DatabaseParser>> {
    match name {
        "postgres" => Ok(Box::new(postgres::PostgresParser::new())),
        "mysql" => Ok(Box::new(mysql::MySqlParser::new())),
        _ => Err(crate::error::SqlcxError::UnknownParser(name.to_string())),
    }
}
