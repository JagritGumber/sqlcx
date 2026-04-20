// Go e2e: generates code for every Go driver, writes it into
// tests/typecheck-go/generated/<driver>/, then runs `go build` against that
// package. Skips if `go` is not on PATH — CI installs go and exercises the
// full matrix.

mod common;

use common::{Dialect, generate, run_cmd, workspace_root, write_files};
use std::path::PathBuf;
use std::process::Command;

fn typecheck_dir() -> PathBuf {
    workspace_root().join("tests/typecheck-go")
}

fn module_name(driver: &str) -> String {
    driver.replace('-', "_")
}

fn run_go_build(driver: &str, dialect: Dialect) {
    if Command::new("go").arg("version").output().is_err() {
        eprintln!("Skipping Go typecheck — go not available");
        return;
    }

    let files = generate("go", "structs", driver, dialect);
    let module = module_name(driver);
    let pkg_dir = typecheck_dir().join("generated").join(&module);
    let _ = std::fs::remove_dir_all(&pkg_dir);
    write_files(&pkg_dir, &files);

    // Rename package line to avoid collision across parallel packages under
    // the same module. `go build` takes the package-relative path.
    let pkg_path = format!("./generated/{module}/...");
    let mut cmd = Command::new("go");
    cmd.args(["build", &pkg_path]).current_dir(typecheck_dir());
    let result = run_cmd(&mut cmd, &format!("go build ({driver})"));
    match result {
        Ok(true) => {}
        Ok(false) => eprintln!("Skipping Go typecheck — go not runnable"),
        Err(e) => panic!("{e}"),
    }
}

#[test]
fn structs_pgx_compiles() {
    run_go_build("pgx", Dialect::Postgres);
}

#[test]
fn structs_database_sql_postgres_compiles() {
    run_go_build("database-sql", Dialect::Postgres);
}

#[test]
fn structs_database_sql_mysql_compiles() {
    run_go_build("database-sql-mysql", Dialect::Mysql);
}

#[test]
fn structs_database_sql_sqlite_compiles() {
    run_go_build("database-sql-sqlite", Dialect::Sqlite);
}
