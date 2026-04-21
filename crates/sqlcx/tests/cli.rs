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
    assert!(stdout.contains("schema"));
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
    assert!(dir.path().join("src/db/users.queries.ts").exists());
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
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/schema.sql"
        ),
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
    let query_files: Vec<_> = std::fs::read_dir(&out_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".queries.ts"))
        .collect();
    assert!(!query_files.is_empty());
    assert!(!out_dir.join("client.ts").exists());
}

#[test]
fn cli_check_validates_without_writing() {
    let dir = tempfile::tempdir().unwrap();
    let sql_dir = dir.path().join("sql");
    std::fs::create_dir_all(sql_dir.join("queries")).unwrap();

    std::fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/schema.sql"
        ),
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
fn cli_check_fails_on_invalid_driver() {
    let dir = tempfile::tempdir().unwrap();
    let sql_dir = dir.path().join("sql");
    std::fs::create_dir_all(&sql_dir).unwrap();

    std::fs::write(
        sql_dir.join("schema.sql"),
        "CREATE TABLE users (id SERIAL PRIMARY KEY, name TEXT NOT NULL);",
    )
    .unwrap();

    std::fs::write(
        dir.path().join("sqlcx.toml"),
        "sql = \"./sql\"\nparser = \"postgres\"\n\n[[targets]]\nlanguage = \"typescript\"\nout = \"./src/db\"\nschema = \"typebox\"\ndriver = \"definitely-not-real\"\n",
    )
    .unwrap();

    let output = sqlcx_bin()
        .arg("check")
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown driver generator"));
}

#[test]
fn cli_schema_emits_json_schema() {
    let output = sqlcx_bin().arg("schema").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"$schema\""));
    assert!(stdout.contains("\"SqlcxConfig\""));
}

#[test]
fn cli_generate_with_relative_out_path() {
    let dir = tempfile::tempdir().unwrap();
    let sql_dir = dir.path().join("sql");
    std::fs::create_dir_all(sql_dir.join("queries")).unwrap();

    std::fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/schema.sql"
        ),
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
    assert!(
        dir.path().join("src/db/schema.ts").exists(),
        "schema.ts not found at src/db/"
    );
    assert!(
        dir.path().join("src/db/users.queries.ts").exists(),
        "users.queries.ts not found at src/db/"
    );
    // Should NOT be double-nested
    assert!(
        !dir.path().join("src/db/src/db/schema.ts").exists(),
        "double-nested path detected!"
    );
}

#[test]
fn cli_generate_prunes_stale_query_files() {
    let dir = tempfile::tempdir().unwrap();
    let sql_dir = dir.path().join("sql");
    let queries_dir = sql_dir.join("queries");
    std::fs::create_dir_all(&queries_dir).unwrap();

    std::fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/schema.sql"
        ),
        sql_dir.join("schema.sql"),
    )
    .unwrap();

    std::fs::write(
        queries_dir.join("users.sql"),
        "-- name: GetUser :one\nSELECT * FROM users WHERE id = $1;\n",
    )
    .unwrap();

    std::fs::write(
        dir.path().join("sqlcx.toml"),
        "sql = \"./sql\"\nparser = \"postgres\"\n\n[[targets]]\nlanguage = \"typescript\"\nout = \"./src/db\"\nschema = \"typebox\"\ndriver = \"bun-sql\"\n",
    )
    .unwrap();

    let first = sqlcx_bin()
        .arg("generate")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        first.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(dir.path().join("src/db/users.queries.ts").exists());

    std::fs::remove_file(queries_dir.join("users.sql")).unwrap();

    let second = sqlcx_bin()
        .arg("generate")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        second.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(!dir.path().join("src/db/users.queries.ts").exists());
}

#[test]
fn cli_generate_accepts_multi_table_inner_join() {
    // JOIN queries with qualified columns now succeed via the multi-table
    // resolver path. Single-table qualified selects are still rejected —
    // that's a separate effort (PR #32).
    let dir = tempfile::tempdir().unwrap();
    let sql_dir = dir.path().join("sql");
    let queries_dir = sql_dir.join("queries");
    std::fs::create_dir_all(&queries_dir).unwrap();

    std::fs::write(
        sql_dir.join("schema.sql"),
        "CREATE TABLE users (id SERIAL PRIMARY KEY, org_id INTEGER NOT NULL, name TEXT NOT NULL);\nCREATE TABLE orgs (id SERIAL PRIMARY KEY, slug TEXT NOT NULL);\n",
    )
    .unwrap();

    std::fs::write(
        queries_dir.join("users.sql"),
        "-- name: ListUsersWithOrg :many\nSELECT users.id, orgs.slug FROM users JOIN orgs ON orgs.id = users.org_id;\n",
    )
    .unwrap();

    std::fs::write(
        dir.path().join("sqlcx.toml"),
        "sql = \"./sql\"\nparser = \"postgres\"\n\n[[targets]]\nlanguage = \"typescript\"\nout = \"./src/db\"\nschema = \"typebox\"\ndriver = \"bun-sql\"\n",
    )
    .unwrap();

    let output = sqlcx_bin()
        .arg("generate")
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn cli_generate_keeps_files_from_multiple_targets_in_same_out_dir() {
    let dir = tempfile::tempdir().unwrap();
    let sql_dir = dir.path().join("sql");
    let queries_dir = sql_dir.join("queries");
    std::fs::create_dir_all(&queries_dir).unwrap();

    std::fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/schema.sql"
        ),
        sql_dir.join("schema.sql"),
    )
    .unwrap();
    std::fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/queries/users.sql"
        ),
        queries_dir.join("users.sql"),
    )
    .unwrap();

    std::fs::write(
        dir.path().join("sqlcx.toml"),
        "sql = \"./sql\"\nparser = \"postgres\"\n\n[[targets]]\nlanguage = \"typescript\"\nout = \"./shared\"\nschema = \"typebox\"\ndriver = \"bun-sql\"\n\n[[targets]]\nlanguage = \"python\"\nout = \"./shared\"\nschema = \"pydantic\"\ndriver = \"psycopg\"\n",
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
    assert!(dir.path().join("shared/schema.ts").exists());
    assert!(dir.path().join("shared/users.queries.ts").exists());
    assert!(!dir.path().join("shared/client.ts").exists());
    assert!(dir.path().join("shared/models.py").exists());
    assert!(dir.path().join("shared/client.py").exists());
    assert!(dir.path().join("shared/users_queries.py").exists());
}
