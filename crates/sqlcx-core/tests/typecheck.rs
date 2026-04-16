use sqlcx_core::{
    config::TargetConfig, generator::resolve_language, ir::SqlcxIR,
    parser::postgres::PostgresParser, parser::DatabaseParser,
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

fn build_fixture_ir() -> SqlcxIR {
    let schema_sql = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/schema.sql"
    ));
    let queries_sql = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/queries/users.sql"
    ));
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

fn target_config(schema: &str, driver: &str) -> TargetConfig {
    TargetConfig {
        language: "typescript".to_string(),
        out: "./generated".to_string(),
        schema: schema.to_string(),
        driver: driver.to_string(),
        overrides: HashMap::new(),
    }
}

fn run_tsc(
    typecheck_base: &str,
    subdir: &str,
    files: &[sqlcx_core::generator::GeneratedFile],
) -> std::result::Result<bool, String> {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let gen_dir = workspace_root
        .join(typecheck_base)
        .join("generated")
        .join(subdir);
    fs::create_dir_all(&gen_dir).unwrap();

    for file in files {
        let filename = Path::new(&file.path).file_name().unwrap();
        fs::write(gen_dir.join(filename), &file.content).unwrap();
    }

    let output = Command::new("npx")
        .args([
            "tsc",
            "--noEmit",
            "--project",
            &format!("{}/tsconfig.json", typecheck_base),
        ])
        .current_dir(&workspace_root)
        .output();

    fs::remove_dir_all(&gen_dir).ok();

    match output {
        Ok(result) => {
            if result.status.success() {
                Ok(true)
            } else {
                Err(format!(
                    "tsc failed:\nstdout: {}\nstderr: {}",
                    String::from_utf8_lossy(&result.stdout),
                    String::from_utf8_lossy(&result.stderr)
                ))
            }
        }
        Err(_) => Ok(false),
    }
}

#[test]
fn typebox_bunsql_typechecks() {
    let ir = build_fixture_ir();
    let plugin = resolve_language("typescript", "typebox", "bun-sql").unwrap();
    let files = plugin
        .generate(&ir, &target_config("typebox", "bun-sql"))
        .unwrap();
    match run_tsc("tests/typecheck", "typebox-bunsql", &files) {
        Ok(false) => eprintln!("Skipping typecheck — npx not available"),
        Ok(true) => {}
        Err(e) => panic!("{}", e),
    }
}

#[test]
fn zod_v4_bunsql_typechecks() {
    let ir = build_fixture_ir();
    let plugin = resolve_language("typescript", "zod", "bun-sql").unwrap();
    let files = plugin
        .generate(&ir, &target_config("zod", "bun-sql"))
        .unwrap();
    match run_tsc("tests/typecheck", "zod-bunsql", &files) {
        Ok(false) => eprintln!("Skipping typecheck — npx not available"),
        Ok(true) => {}
        Err(e) => panic!("{}", e),
    }
}

#[test]
fn zod_v3_bunsql_typechecks() {
    let ir = build_fixture_ir();
    let plugin = resolve_language("typescript", "zod/v3", "bun-sql").unwrap();
    let files = plugin
        .generate(&ir, &target_config("zod/v3", "bun-sql"))
        .unwrap();
    match run_tsc("tests/typecheck-zod3", "zod3-bunsql", &files) {
        Ok(false) => eprintln!("Skipping typecheck — npx not available"),
        Ok(true) => {}
        Err(e) => panic!("{}", e),
    }
}

#[test]
fn typebox_pg_typechecks() {
    let ir = build_fixture_ir();
    let plugin = resolve_language("typescript", "typebox", "pg").unwrap();
    let files = plugin
        .generate(&ir, &target_config("typebox", "pg"))
        .unwrap();
    match run_tsc("tests/typecheck", "typebox-pg", &files) {
        Ok(false) => eprintln!("Skipping typecheck — npx not available"),
        Ok(true) => {}
        Err(e) => panic!("{}", e),
    }
}
