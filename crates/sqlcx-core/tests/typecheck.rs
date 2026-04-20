// TypeScript e2e: runs `tsc --noEmit` on the generated output for every
// supported (schema, driver) combination. Skips gracefully if `npx` is missing.

mod common;

use common::{Dialect, generate, run_cmd, workspace_root, write_files};
use std::process::Command;

fn run_tsc(typecheck_base: &str, subdir: &str, schema: &str, driver: &str, dialect: Dialect) {
    let files = generate("typescript", schema, driver, dialect);
    let gen_dir = workspace_root()
        .join(typecheck_base)
        .join("generated")
        .join(subdir);
    write_files(&gen_dir, &files);

    let tsconfig = format!("{typecheck_base}/tsconfig.json");
    let mut cmd = Command::new("npx");
    cmd.args(["tsc", "--noEmit", "--project", &tsconfig])
        .current_dir(workspace_root());

    let result = run_cmd(&mut cmd, &format!("tsc ({schema}, {driver})"));
    let _ = std::fs::remove_dir_all(&gen_dir);
    match result {
        Ok(true) => {}
        Ok(false) => eprintln!("Skipping typecheck — npx not available"),
        Err(e) => panic!("{e}"),
    }
}

#[test]
fn typebox_bunsql_typechecks() {
    run_tsc(
        "tests/typecheck",
        "typebox-bunsql",
        "typebox",
        "bun-sql",
        Dialect::Postgres,
    );
}

#[test]
fn zod_v4_bunsql_typechecks() {
    run_tsc(
        "tests/typecheck",
        "zod-bunsql",
        "zod",
        "bun-sql",
        Dialect::Postgres,
    );
}

#[test]
fn zod_v3_bunsql_typechecks() {
    run_tsc(
        "tests/typecheck-zod3",
        "zod3-bunsql",
        "zod/v3",
        "bun-sql",
        Dialect::Postgres,
    );
}

#[test]
fn typebox_pg_typechecks() {
    run_tsc(
        "tests/typecheck",
        "typebox-pg",
        "typebox",
        "pg",
        Dialect::Postgres,
    );
}

#[test]
fn typebox_mysql2_typechecks() {
    run_tsc(
        "tests/typecheck",
        "typebox-mysql2",
        "typebox",
        "mysql2",
        Dialect::Mysql,
    );
}

#[test]
fn typebox_better_sqlite3_typechecks() {
    run_tsc(
        "tests/typecheck",
        "typebox-bettersqlite3",
        "typebox",
        "better-sqlite3",
        Dialect::Sqlite,
    );
}
