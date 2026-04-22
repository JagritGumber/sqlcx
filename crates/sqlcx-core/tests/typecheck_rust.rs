// Rust e2e: generates code for every Rust driver, writes it into
// tests/typecheck-rust/src/generated/, and runs `cargo check` on the
// scaffold crate. Validates that generated Rust actually compiles against
// real sqlx / tokio-postgres / chrono / serde.

mod common;

use common::{Dialect, generate, run_cmd, workspace_root, write_files};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

// The four tests all rewrite tests/typecheck-rust/src/lib.rs + src/generated/,
// and `cargo check` serialises on the target directory lock anyway, so run
// them one at a time to keep failures attributable.
fn serialize() -> &'static Mutex<()> {
    static L: OnceLock<Mutex<()>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(()))
}

fn typecheck_dir() -> PathBuf {
    workspace_root().join("tests/typecheck-rust")
}

fn module_name(driver: &str) -> String {
    driver.replace('-', "_")
}

fn run_cargo_check(driver: &str, dialect: Dialect) {
    let _guard = serialize().lock().unwrap_or_else(|p| p.into_inner());
    let files = generate("rust", "serde", driver, dialect);
    let module = module_name(driver);
    let gen_root = typecheck_dir().join("src/generated");
    let gen_dir = gen_root.join(&module);
    let _ = fs::remove_dir_all(&gen_root);
    write_files(&gen_dir, &files);

    // Produce src/lib.rs that declares the module tree so cargo sees the files.
    let submodules: Vec<String> = fs::read_dir(&gen_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("rs"))
        .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    let mut module_rs = String::from("#![allow(unused, dead_code, non_snake_case)]\n");
    for sub in &submodules {
        module_rs.push_str(&format!("pub mod {sub};\n"));
    }
    fs::write(gen_dir.join("mod.rs"), module_rs).unwrap();
    fs::write(
        typecheck_dir().join("src/lib.rs"),
        format!("#![allow(unused)]\n#[path = \"generated/{module}/mod.rs\"]\npub mod generated;\n"),
    )
    .unwrap();

    let mut cmd = Command::new("cargo");
    cmd.args(["check", "--quiet"]).current_dir(typecheck_dir());
    let result = run_cmd(&mut cmd, &format!("cargo check ({driver})"));
    match result {
        Ok(true) => {}
        Ok(false) => eprintln!("Skipping Rust typecheck — cargo not available"),
        Err(e) => panic!("{e}"),
    }
}

#[test]
fn serde_sqlx_postgres_compiles() {
    run_cargo_check("sqlx-postgres", Dialect::Postgres);
}

#[test]
fn serde_sqlx_mysql_compiles() {
    run_cargo_check("sqlx-mysql", Dialect::Mysql);
}

#[test]
fn serde_sqlx_sqlite_compiles() {
    run_cargo_check("sqlx-sqlite", Dialect::Sqlite);
}

#[test]
fn serde_tokio_postgres_compiles() {
    run_cargo_check("tokio-postgres", Dialect::Postgres);
}
