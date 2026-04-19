#[cfg(feature = "migrate")]
mod migrate_cmd;

use clap::{Parser, Subcommand};
use schemars::schema_for;
use sqlcx::run_pipeline;
use sqlcx_core::config::SqlcxConfig;
#[cfg(feature = "migrate")]
use sqlcx_core::config::load_config;

#[derive(Parser)]
#[command(
    name = "sqlcx",
    about = "SQL-first cross-language type-safe code generator"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse SQL and generate type-safe code for all targets
    Generate,
    /// Validate SQL files and config without generating
    Check,
    /// Scaffold a new sqlcx project
    Init,
    /// Emit JSON Schema for config validation
    Schema,
    /// Manage database migrations
    #[cfg(feature = "migrate")]
    Migrate {
        #[command(subcommand)]
        cmd: migrate_cmd::MigrateCommand,
    },
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> sqlcx_core::error::Result<()> {
    match cli.command {
        Commands::Generate => run_pipeline(true),
        Commands::Check => run_pipeline(false),
        Commands::Init => run_init(),
        Commands::Schema => run_schema(),
        #[cfg(feature = "migrate")]
        Commands::Migrate { cmd } => {
            let cwd = std::env::current_dir()?;
            let config = load_config(&cwd)?;
            let should_regen = migrate_cmd::run(&cwd, &config, cmd)?;
            if should_regen {
                eprintln!("Re-generating typed clients...");
                run_pipeline(true)?;
            }
            Ok(())
        }
    }
}

fn run_init() -> sqlcx_core::error::Result<()> {
    let cwd = std::env::current_dir()?;

    let config_path = cwd.join("sqlcx.toml");
    if config_path.exists() {
        return Err(sqlcx_core::error::SqlcxError::ConfigInvalid(
            "sqlcx.toml already exists in this directory".to_string(),
        ));
    }

    let sql_dir = cwd.join("sql");
    let queries_dir = sql_dir.join("queries");
    let migrations_dir = sql_dir.join("migrations");
    std::fs::create_dir_all(&queries_dir)?;
    std::fs::create_dir_all(&migrations_dir)?;

    std::fs::write(
        &config_path,
        r#"sql    = "./sql"
parser = "postgres"

[[targets]]
language = "typescript"
out      = "./src/db"
schema   = "typebox"
driver   = "bun-sql"

[migrate]
dir             = "./sql/migrations"
auto_regenerate = true
# database_url  = "postgres://user:pass@localhost:5432/mydb"
# or set SQLCX_DATABASE_URL in your environment
"#,
    )?;

    std::fs::write(
        sql_dir.join("schema.sql"),
        r#"CREATE TABLE users (
  id         SERIAL      PRIMARY KEY,
  name       TEXT        NOT NULL,
  email      TEXT        NOT NULL UNIQUE,
  created_at TIMESTAMP   NOT NULL DEFAULT NOW()
);
"#,
    )?;

    std::fs::write(
        queries_dir.join("users.sql"),
        r#"-- name: GetUser :one
SELECT * FROM users WHERE id = $1;

-- name: ListUsers :many
SELECT id, name, email FROM users ORDER BY created_at DESC;

-- name: CreateUser :exec
INSERT INTO users (name, email) VALUES ($1, $2);
"#,
    )?;

    eprintln!("Created sqlcx.toml");
    eprintln!("Created sql/schema.sql");
    eprintln!("Created sql/queries/users.sql");
    eprintln!("Created sql/migrations/");
    eprintln!();
    eprintln!("Run `sqlcx generate` to generate typed code.");
    eprintln!("Run `sqlcx migrate new <name>` to create your first migration.");
    Ok(())
}

fn run_schema() -> sqlcx_core::error::Result<()> {
    let schema = schema_for!(SqlcxConfig);
    println!("{}", serde_json::to_string_pretty(&schema)?);
    Ok(())
}
