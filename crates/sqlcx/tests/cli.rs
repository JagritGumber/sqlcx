use std::process::Command;

fn sqlcx_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sqlcx"))
}

#[test]
fn cli_help() {
    let output = sqlcx_bin().arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("generate"));
    assert!(stdout.contains("check"));
    assert!(stdout.contains("init"));
}

#[test]
fn cli_init_scaffolds_project() {
    let dir = tempfile::tempdir().unwrap();

    let output = sqlcx_bin()
        .arg("init")
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(dir.path().join("sqlcx.toml").exists());
    assert!(dir.path().join("sql/schema.sql").exists());
    assert!(dir.path().join("sql/queries/users.sql").exists());

    // Verify generate works on the scaffolded project
    let output = sqlcx_bin()
        .arg("generate")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "generate after init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(dir.path().join("src/db/schema.ts").exists());
    assert!(dir.path().join("src/db/client.ts").exists());
}

#[test]
fn cli_init_refuses_if_config_exists() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("sqlcx.toml"), "existing").unwrap();

    let output = sqlcx_bin()
        .arg("init")
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("already exists"));
}

#[test]
fn cli_generate_with_fixtures() {
    let dir = tempfile::tempdir().unwrap();
    let sql_dir = dir.path().join("sql");
    std::fs::create_dir_all(sql_dir.join("queries")).unwrap();

    std::fs::copy(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/schema.sql"),
        sql_dir.join("schema.sql"),
    )
    .unwrap();
    std::fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/queries/users.sql"
        ),
        sql_dir.join("queries/users.sql"),
    )
    .unwrap();

    let out_dir = dir.path().join("src/db");

    // TOML literal strings (single quotes) avoid backslash escape processing on Windows paths.
    std::fs::write(
        dir.path().join("sqlcx.toml"),
        format!(
            "sql = '{}'\nparser = \"postgres\"\n\n[[targets]]\nlanguage = \"typescript\"\nout = '{}'\nschema = \"typebox\"\ndriver = \"bun-sql\"\n",
            sql_dir.display(),
            out_dir.display()
        ),
    )
    .unwrap();

    let output = sqlcx_bin()
        .arg("generate")
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out_dir.join("schema.ts").exists());
    assert!(out_dir.join("client.ts").exists());
    // Query file should exist (users.queries.ts)
    let query_files: Vec<_> = std::fs::read_dir(&out_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".queries.ts"))
        .collect();
    assert!(!query_files.is_empty());
}

#[test]
fn cli_check_validates_without_writing() {
    let dir = tempfile::tempdir().unwrap();
    let sql_dir = dir.path().join("sql");
    std::fs::create_dir_all(sql_dir.join("queries")).unwrap();

    std::fs::copy(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/schema.sql"),
        sql_dir.join("schema.sql"),
    )
    .unwrap();
    std::fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/queries/users.sql"
        ),
        sql_dir.join("queries/users.sql"),
    )
    .unwrap();

    // TOML literal strings (single quotes) avoid backslash escape processing on Windows paths.
    std::fs::write(
        dir.path().join("sqlcx.toml"),
        format!(
            "sql = '{}'\nparser = \"postgres\"\n\n[[targets]]\nlanguage = \"typescript\"\nout = \"./src/db\"\nschema = \"typebox\"\ndriver = \"bun-sql\"\n",
            sql_dir.display()
        ),
    )
    .unwrap();

    let output = sqlcx_bin()
        .arg("check")
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(!dir.path().join("src/db/schema.ts").exists());
}

#[test]
fn cli_generate_with_relative_out_path() {
    let dir = tempfile::tempdir().unwrap();
    let sql_dir = dir.path().join("sql");
    std::fs::create_dir_all(sql_dir.join("queries")).unwrap();

    std::fs::copy(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/schema.sql"),
        sql_dir.join("schema.sql"),
    )
    .unwrap();
    std::fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/queries/users.sql"
        ),
        sql_dir.join("queries/users.sql"),
    )
    .unwrap();

    // Use RELATIVE out path -- this is what users actually write in config.
    // TOML literal strings (single quotes) avoid backslash escape processing on Windows paths.
    std::fs::write(
        dir.path().join("sqlcx.toml"),
        format!(
            "sql = '{}'\nparser = \"postgres\"\n\n[[targets]]\nlanguage = \"typescript\"\nout = \"./src/db\"\nschema = \"typebox\"\ndriver = \"bun-sql\"\n",
            sql_dir.display()
        ),
    )
    .unwrap();

    let output = sqlcx_bin()
        .arg("generate")
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    // Files should be at ./src/db/ relative to cwd, NOT double-nested
    assert!(dir.path().join("src/db/schema.ts").exists(), "schema.ts not found at src/db/");
    assert!(dir.path().join("src/db/client.ts").exists(), "client.ts not found at src/db/");
    // Should NOT be double-nested
    assert!(!dir.path().join("src/db/src/db/schema.ts").exists(), "double-nested path detected!");
}
