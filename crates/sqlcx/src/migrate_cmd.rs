use clap::Subcommand;
use sqlcx_core::config::{MigrateConfig, SqlcxConfig};
use sqlcx_core::error::{Result, SqlcxError};
use sqlcx_core::migrate::{
    compute_status, create_new_migration, discover_migrations, run_pending, MigrationDriver,
    MigrationStatus, PostgresDriver,
};
use std::path::{Path, PathBuf};

#[derive(Subcommand)]
pub enum MigrateCommand {
    /// Create a new timestamped migration file
    New { name: String },
    /// Apply all pending migrations
    Up,
    /// Show status of all migrations (pending, applied, drifted)
    Status,
}

pub fn run(cwd: &Path, config: &SqlcxConfig, cmd: MigrateCommand) -> Result<bool> {
    let mig = config
        .migrate
        .as_ref()
        .ok_or_else(|| SqlcxError::Migrate("no [migrate] section in sqlcx config".to_string()))?;
    let dir = resolve_path(cwd, &mig.dir);
    match cmd {
        MigrateCommand::New { name } => {
            let path = create_new_migration(&dir, &name)?;
            eprintln!("Created {}", path.display());
            Ok(false)
        }
        MigrateCommand::Status => cmd_status(&dir, mig).map(|_| false),
        MigrateCommand::Up => cmd_up(&dir, mig),
    }
}

fn cmd_status(dir: &Path, mig: &MigrateConfig) -> Result<()> {
    let files = discover_migrations(dir)?;
    let url = resolve_database_url(mig)?;
    let mut driver = PostgresDriver::connect(&url)?;
    driver.ensure_state_table()?;
    let applied = driver.list_applied()?;
    let statuses = compute_status(&files, &applied);
    if statuses.is_empty() {
        eprintln!("No migrations found in {}", dir.display());
        return Ok(());
    }
    for outcome in statuses {
        let tag = match outcome.status {
            MigrationStatus::Pending => "pending",
            MigrationStatus::Applied => "applied",
            MigrationStatus::Drifted { .. } => "DRIFTED",
        };
        eprintln!("{:<10} {} {}", tag, outcome.version, outcome.name);
    }
    Ok(())
}

fn cmd_up(dir: &Path, mig: &MigrateConfig) -> Result<bool> {
    let files = discover_migrations(dir)?;
    let url = resolve_database_url(mig)?;
    let mut driver = PostgresDriver::connect(&url)?;
    let applied = run_pending(&mut driver, &files)?;
    if applied.is_empty() {
        eprintln!("No pending migrations.");
        return Ok(false);
    }
    for v in &applied {
        eprintln!("Applied {}", v);
    }
    Ok(mig.auto_regenerate)
}

fn resolve_database_url(mig: &MigrateConfig) -> Result<String> {
    if let Some(url) = &mig.database_url {
        return Ok(url.clone());
    }
    std::env::var("SQLCX_DATABASE_URL").map_err(|_| {
        SqlcxError::Migrate(
            "database_url not set in config and SQLCX_DATABASE_URL env var is empty".to_string(),
        )
    })
}

fn resolve_path(base: &Path, s: &str) -> PathBuf {
    let p = PathBuf::from(s);
    if p.is_absolute() {
        p
    } else {
        base.join(p)
    }
}
