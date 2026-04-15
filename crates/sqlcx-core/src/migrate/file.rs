use crate::error::{Result, SqlcxError};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct MigrationFile {
    pub version: String,
    pub name: String,
    pub path: PathBuf,
    pub content: String,
    pub checksum: String,
}

pub fn compute_checksum(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn parse_filename(filename: &str) -> Option<(String, String)> {
    let stem = filename.strip_suffix(".sql")?;
    let (version, name) = stem.split_once('_')?;
    if version.is_empty() || name.is_empty() {
        return None;
    }
    if !version.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((version.to_string(), name.to_string()))
}

pub fn discover_migrations(dir: &Path) -> Result<Vec<MigrationFile>> {
    if !dir.exists() {
        return Err(SqlcxError::Migrate(format!(
            "migration directory not found: {}",
            dir.display()
        )));
    }
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let filename = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        let (version, name) = match parse_filename(filename) {
            Some(parts) => parts,
            None => continue,
        };
        let content = std::fs::read_to_string(&path)?;
        let checksum = compute_checksum(&content);
        files.push(MigrationFile {
            version,
            name,
            path,
            content,
            checksum,
        });
    }
    files.sort_by(|a, b| a.version.cmp(&b.version));
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_filename_valid() {
        assert_eq!(
            parse_filename("20260415153000_create_users.sql"),
            Some(("20260415153000".to_string(), "create_users".to_string()))
        );
    }

    #[test]
    fn parse_filename_invalid() {
        assert_eq!(parse_filename("create_users.sql"), None);
        assert_eq!(parse_filename("20260415153000.sql"), None);
        assert_eq!(parse_filename("abc_create_users.sql"), None);
        assert_eq!(parse_filename("20260415153000_create_users.txt"), None);
    }

    #[test]
    fn checksum_deterministic() {
        assert_eq!(
            compute_checksum("CREATE TABLE users (id INT);"),
            compute_checksum("CREATE TABLE users (id INT);")
        );
    }

    #[test]
    fn discover_sorts_by_version() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("20260101000000_first.sql"), "SELECT 1;").unwrap();
        std::fs::write(dir.path().join("20260202000000_second.sql"), "SELECT 2;").unwrap();
        std::fs::write(dir.path().join("not_a_migration.txt"), "ignore").unwrap();
        let files = discover_migrations(dir.path()).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].version, "20260101000000");
        assert_eq!(files[1].version, "20260202000000");
    }
}
