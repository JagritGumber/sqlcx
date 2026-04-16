#[cfg(feature = "migrate")]
mod migrate_cmd;

use clap::{Parser, Subcommand};
use schemars::schema_for;
use serde::{Deserialize, Serialize};
use sqlcx_core::{
    cache::{compute_hash, read_cache, write_cache, SqlFile},
    config::{load_config, SqlcxConfig, TargetConfig},
    generator::resolve_language,
    ir::SqlcxIR,
    parser::resolve_parser,
};
use std::path::{Path, PathBuf};

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

fn run_pipeline(write_output: bool) -> sqlcx_core::error::Result<()> {
    let cwd = std::env::current_dir()?;

    eprintln!("Loading config from {}", cwd.display());
    let config = load_config(&cwd)?;

    // Resolve SQL directory against cwd (may be relative)
    let sql_dir = resolve_path(&cwd, &config.sql);
    eprintln!("Scanning SQL files in {}", sql_dir.display());

    let all_sql = collect_sql_files(&sql_dir)?;

    // Read all SQL files into structs once
    let sql_file_structs: Vec<SqlFile> = all_sql
        .iter()
        .map(|p| -> sqlcx_core::error::Result<SqlFile> {
            Ok(SqlFile {
                path: p.to_string_lossy().into_owned(),
                content: std::fs::read_to_string(p)?,
            })
        })
        .collect::<sqlcx_core::error::Result<Vec<_>>>()?;

    // Partition using already-loaded content (no double reads)
    let queries_dir = sql_dir.join("queries");
    let (schema_files, query_files) = partition_sql_files(&queries_dir, &sql_file_structs);

    let hash = compute_hash(&sql_file_structs, &config.parser);
    let cache_dir = cwd.join(".sqlcx");

    let ir = if let Some(cached) = read_cache(&cache_dir, &hash)? {
        eprintln!("Cache hit — using cached IR");
        cached
    } else {
        eprintln!("Cache miss — parsing SQL files");
        let ir = build_ir(&config.parser, &schema_files, &query_files)?;
        write_cache(&cache_dir, &ir, &hash)?;
        ir
    };

    let validated_targets = validate_targets(&config)?;

    if write_output {
        for (target, merged_target, plugin) in validated_targets {
            let files = plugin.generate(&ir, &merged_target)?;
            let out_dir = resolve_path(&cwd, &target.out);
            std::fs::create_dir_all(&out_dir)?;
            sync_generated_files(&out_dir, &files)?;
            for file in files {
                let dest = out_dir.join(&file.path);
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                eprintln!("Writing {}", dest.display());
                std::fs::write(&dest, &file.content)?;
            }
        }
        eprintln!("Done.");
    } else {
        eprintln!(
            "Check passed — {} tables, {} queries, {} enums",
            ir.tables.len(),
            ir.queries.len(),
            ir.enums.len()
        );
    }

    Ok(())
}

fn run_schema() -> sqlcx_core::error::Result<()> {
    let schema = schema_for!(SqlcxConfig);
    println!("{}", serde_json::to_string_pretty(&schema)?);
    Ok(())
}

fn validate_targets(config: &SqlcxConfig) -> sqlcx_core::error::Result<Vec<ValidatedTarget>> {
    let mut validated = Vec::with_capacity(config.targets.len());
    for target in &config.targets {
        let mut merged_target = target.clone();
        for (k, v) in &config.overrides {
            merged_target
                .overrides
                .entry(k.clone())
                .or_insert(v.clone());
        }
        let plugin = resolve_language(
            &merged_target.language,
            &merged_target.schema,
            &merged_target.driver,
        )?;
        validated.push((target.clone(), merged_target, plugin));
    }
    Ok(validated)
}

#[derive(Serialize, Deserialize, Default)]
struct OutputManifest {
    files: Vec<String>,
}

type ValidatedTarget = (
    TargetConfig,
    TargetConfig,
    Box<dyn sqlcx_core::generator::LanguagePlugin>,
);

fn sync_generated_files(
    out_dir: &Path,
    files: &[sqlcx_core::generator::GeneratedFile],
) -> sqlcx_core::error::Result<()> {
    let manifest_path = out_dir.join(".sqlcx-manifest.json");
    let previous = if manifest_path.exists() {
        serde_json::from_str::<OutputManifest>(&std::fs::read_to_string(&manifest_path)?)
            .unwrap_or_default()
    } else {
        OutputManifest::default()
    };

    let current: std::collections::BTreeSet<String> =
        files.iter().map(|file| file.path.clone()).collect();

    for stale in previous.files {
        if !current.contains(&stale) {
            let stale_path = out_dir.join(&stale);
            if stale_path.exists() {
                std::fs::remove_file(&stale_path)?;
            }
        }
    }

    let manifest = OutputManifest {
        files: current.into_iter().collect(),
    };
    std::fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
    Ok(())
}

fn resolve_path(base: &Path, s: &str) -> PathBuf {
    let p = PathBuf::from(s);
    if p.is_absolute() {
        p
    } else {
        base.join(p)
    }
}

fn collect_sql_files(sql_dir: &Path) -> sqlcx_core::error::Result<Vec<PathBuf>> {
    let pattern = format!("{}/**/*.sql", sql_dir.display());
    let mut paths = Vec::new();
    for entry in glob::glob(&pattern).map_err(|e| {
        sqlcx_core::error::SqlcxError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            e.to_string(),
        ))
    })? {
        match entry {
            Ok(p) => paths.push(p),
            Err(e) => eprintln!("warning: glob error: {e}"),
        }
    }
    paths.sort();
    Ok(paths)
}

/// Partition SQL files using already-loaded content.
/// Schema files: not inside a "queries" directory and not containing `-- name:`.
/// Query files: inside a "queries" directory or containing `-- name:`.
fn partition_sql_files(queries_dir: &Path, files: &[SqlFile]) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut schema = Vec::new();
    let mut queries = Vec::new();
    for f in files {
        let p = PathBuf::from(&f.path);
        let is_in_queries_dir = p.starts_with(queries_dir);
        let has_name_annotation = f.content.contains("-- name:");
        if is_in_queries_dir || has_name_annotation {
            queries.push(p);
        } else {
            schema.push(p);
        }
    }
    (schema, queries)
}

fn build_ir(
    parser_name: &str,
    schema_files: &[PathBuf],
    query_files: &[PathBuf],
) -> sqlcx_core::error::Result<SqlcxIR> {
    let parser = resolve_parser(parser_name)?;

    let mut all_tables = Vec::new();
    let mut all_enums = Vec::new();

    for path in schema_files {
        let sql = std::fs::read_to_string(path)?;
        let (tables, enums) = parser.parse_schema(&sql)?;
        all_tables.extend(tables);
        all_enums.extend(enums);
    }

    let mut all_queries = Vec::new();
    for path in query_files {
        let sql = std::fs::read_to_string(path)?;
        let source = path.to_string_lossy().into_owned();
        let queries = parser.parse_queries(&sql, &all_tables, &all_enums, &source)?;
        all_queries.extend(queries);
    }

    Ok(SqlcxIR {
        tables: all_tables,
        queries: all_queries,
        enums: all_enums,
    })
}
