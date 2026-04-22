// Python e2e: runs `python -m py_compile` on the generated output for every
// Py driver. py_compile catches syntax errors without requiring the driver
// libraries to be installed. Skips gracefully if python3/python is missing.

mod common;

use common::{Dialect, generate, run_cmd, workspace_root, write_files};
use std::process::Command;

fn python_bin() -> Option<&'static str> {
    ["python3", "python"]
        .into_iter()
        .find(|bin| Command::new(bin).arg("--version").output().is_ok())
}

fn run_py_compile(subdir: &str, driver: &str, dialect: Dialect) {
    let files = generate("python", "pydantic", driver, dialect);
    let gen_dir = workspace_root()
        .join("tests/typecheck-python/generated")
        .join(subdir);
    write_files(&gen_dir, &files);

    let Some(bin) = python_bin() else {
        eprintln!("Skipping Python typecheck — python not available");
        let _ = std::fs::remove_dir_all(&gen_dir);
        return;
    };

    // Collect .py files and compile each; py_compile short-circuits on first
    // syntax error and returns non-zero.
    let py_files: Vec<_> = std::fs::read_dir(&gen_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("py"))
        .collect();
    assert!(!py_files.is_empty(), "no .py files generated for {driver}");

    let mut args = vec!["-m".to_string(), "py_compile".to_string()];
    for f in &py_files {
        args.push(f.to_string_lossy().into_owned());
    }
    let mut cmd = Command::new(bin);
    cmd.args(&args);
    let result = run_cmd(&mut cmd, &format!("py_compile ({driver})"));
    let _ = std::fs::remove_dir_all(&gen_dir);
    match result {
        Ok(true) => {}
        Ok(false) => eprintln!("Skipping Python typecheck — {bin} not runnable"),
        Err(e) => panic!("{e}"),
    }
}

#[test]
fn pydantic_psycopg_compiles() {
    run_py_compile("psycopg", "psycopg", Dialect::Postgres);
}

#[test]
fn pydantic_asyncpg_compiles() {
    run_py_compile("asyncpg", "asyncpg", Dialect::Postgres);
}

#[test]
fn pydantic_mysql_connector_compiles() {
    run_py_compile("mysql-connector", "mysql-connector", Dialect::Mysql);
}

#[test]
fn pydantic_sqlite3_compiles() {
    run_py_compile("sqlite3", "sqlite3", Dialect::Sqlite);
}
