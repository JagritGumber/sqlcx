use crate::error::{Result, SqlcxError};
use crate::migrate::driver::MigrationDriver;
use crate::migrate::file::MigrationFile;
use crate::migrate::state::AppliedMigration;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationStatus {
    Pending,
    Applied,
    Drifted { expected: String, found: String },
}

#[derive(Debug)]
pub struct MigrationOutcome {
    pub version: String,
    pub name: String,
    pub status: MigrationStatus,
}

pub fn compute_status(
    files: &[MigrationFile],
    applied: &[AppliedMigration],
) -> Vec<MigrationOutcome> {
    let by_version: HashMap<&str, &AppliedMigration> =
        applied.iter().map(|a| (a.version.as_str(), a)).collect();
    files
        .iter()
        .map(|f| {
            let status = match by_version.get(f.version.as_str()) {
                None => MigrationStatus::Pending,
                Some(a) if a.checksum == f.checksum => MigrationStatus::Applied,
                Some(a) => MigrationStatus::Drifted {
                    expected: a.checksum.clone(),
                    found: f.checksum.clone(),
                },
            };
            MigrationOutcome {
                version: f.version.clone(),
                name: f.name.clone(),
                status,
            }
        })
        .collect()
}

/// Apply all pending migrations in version order. Stops on first error.
/// Drift detection: if any already-applied migration has a different checksum
/// on disk, abort before applying anything.
pub fn run_pending<D: MigrationDriver>(
    driver: &mut D,
    files: &[MigrationFile],
) -> Result<Vec<String>> {
    driver.ensure_state_table()?;
    let applied = driver.list_applied()?;
    let statuses = compute_status(files, &applied);
    for outcome in &statuses {
        if let MigrationStatus::Drifted { expected, found } = &outcome.status {
            return Err(SqlcxError::Migrate(format!(
                "drift detected on {}: expected checksum {}, on-disk {}",
                outcome.version, expected, found
            )));
        }
    }
    let applied_versions: std::collections::HashSet<&str> =
        applied.iter().map(|a| a.version.as_str()).collect();
    let mut newly_applied = Vec::new();
    for file in files {
        if applied_versions.contains(file.version.as_str()) {
            continue;
        }
        driver.apply_migration(file)?;
        newly_applied.push(file.version.clone());
    }
    Ok(newly_applied)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn mk_file(version: &str, content: &str) -> MigrationFile {
        MigrationFile {
            version: version.to_string(),
            name: "test".to_string(),
            path: PathBuf::from(format!("{version}_test.sql")),
            content: content.to_string(),
            checksum: crate::migrate::file::compute_checksum(content),
        }
    }

    #[test]
    fn status_all_pending_when_none_applied() {
        let files = vec![mk_file("1", "a"), mk_file("2", "b")];
        let out = compute_status(&files, &[]);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].status, MigrationStatus::Pending);
    }

    #[test]
    fn status_detects_drift() {
        let files = vec![mk_file("1", "modified")];
        let applied = vec![AppliedMigration {
            version: "1".to_string(),
            name: "test".to_string(),
            checksum: crate::migrate::file::compute_checksum("original"),
        }];
        let out = compute_status(&files, &applied);
        assert!(matches!(out[0].status, MigrationStatus::Drifted { .. }));
    }
}
