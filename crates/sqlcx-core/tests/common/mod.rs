// Shared helpers for e2e typecheck integration tests.
// Each language has its own typecheck_<lang>.rs file that pulls these in.
// Individual test binaries use only a subset, so silence dead-code warnings.
#![allow(dead_code)]

use sqlcx_core::{
    config::TargetConfig,
    generator::{GeneratedFile, resolve_language},
    ir::SqlcxIR,
    parser::{DatabaseParser, mysql::MySqlParser, postgres::PostgresParser, sqlite::SqliteParser},
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Clone, Copy)]
pub enum Dialect {
    Postgres,
    Mysql,
    Sqlite,
}

pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

fn read_fixture(rel: &str) -> String {
    fs::read_to_string(workspace_root().join(rel)).expect(rel)
}

pub fn build_ir(dialect: Dialect) -> SqlcxIR {
    let (schema_rel, query_rel, parser): (_, _, Box<dyn DatabaseParser>) = match dialect {
        Dialect::Postgres => (
            "schema.sql",
            "queries/users.sql",
            Box::new(PostgresParser::new()),
        ),
        Dialect::Mysql => (
            "mysql_schema.sql",
            "mysql_queries/users.sql",
            Box::new(MySqlParser::new()),
        ),
        Dialect::Sqlite => (
            "sqlite_schema.sql",
            "sqlite_queries/users.sql",
            Box::new(SqliteParser::new()),
        ),
    };
    let schema_sql = read_fixture(&format!("tests/fixtures/{schema_rel}"));
    let queries_sql = read_fixture(&format!("tests/fixtures/{query_rel}"));
    let (tables, enums) = parser.parse_schema(&schema_sql).unwrap();
    let queries = parser
        .parse_queries(&queries_sql, &tables, &enums, "queries/users.sql")
        .unwrap();
    SqlcxIR {
        tables,
        queries,
        enums,
    }
}

pub fn target_config(language: &str, schema: &str, driver: &str) -> TargetConfig {
    TargetConfig {
        language: language.to_string(),
        out: "./generated".to_string(),
        schema: schema.to_string(),
        driver: driver.to_string(),
        overrides: HashMap::new(),
    }
}

pub fn generate(
    language: &str,
    schema: &str,
    driver: &str,
    dialect: Dialect,
) -> Vec<GeneratedFile> {
    let ir = build_ir(dialect);
    let plugin = resolve_language(language, schema, driver).unwrap();
    plugin
        .generate(&ir, &target_config(language, schema, driver))
        .unwrap()
}

pub fn write_files(dir: &Path, files: &[GeneratedFile]) {
    fs::create_dir_all(dir).unwrap();
    for file in files {
        let filename = Path::new(&file.path).file_name().unwrap();
        fs::write(dir.join(filename), &file.content).unwrap();
    }
}

/// Run a command; return Ok(true) if success, Ok(false) if the binary is
/// missing (test should be skipped), Err with output if the command ran but
/// failed (genuine e2e failure).
pub fn run_cmd(cmd: &mut Command, label: &str) -> Result<bool, String> {
    match cmd.stdin(Stdio::null()).output() {
        Ok(out) if out.status.success() => Ok(true),
        Ok(out) => Err(format!(
            "{label} failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )),
        Err(_) => Ok(false),
    }
}
