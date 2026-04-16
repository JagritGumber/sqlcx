use crate::error::{Result, SqlcxError};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn create_new_migration(dir: &Path, name: &str) -> Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let safe_name = sanitize_name(name);
    if safe_name.is_empty() {
        return Err(SqlcxError::Migrate(
            "migration name cannot be empty".to_string(),
        ));
    }
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| SqlcxError::Migrate(e.to_string()))?
        .as_secs();
    let version = format_timestamp(secs);
    let filename = format!("{}_{}.sql", version, safe_name);
    let path = dir.join(&filename);
    let body = format!("-- migration: {}\n\n", safe_name);
    std::fs::write(&path, body)?;
    Ok(path)
}

fn sanitize_name(name: &str) -> String {
    name.trim()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

/// Format unix seconds as YYYYMMDDHHMMSS using Howard Hinnant's civil-from-days.
fn format_timestamp(total_secs: u64) -> String {
    let second = total_secs % 60;
    let total_mins = total_secs / 60;
    let minute = total_mins % 60;
    let total_hours = total_mins / 60;
    let hour = total_hours % 24;
    let days = total_hours / 24;

    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    if m <= 2 {
        y += 1;
    }
    format!(
        "{:04}{:02}{:02}{:02}{:02}{:02}",
        y, m, d, hour, minute, second
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_timestamp_known_values() {
        // 2026-04-15 00:00:00 UTC = 1776211200
        assert_eq!(format_timestamp(1776211200), "20260415000000");
        // Epoch
        assert_eq!(format_timestamp(0), "19700101000000");
    }

    #[test]
    fn sanitize_converts_spaces_and_specials() {
        assert_eq!(sanitize_name("create users"), "create_users");
        assert_eq!(sanitize_name("  hello!  world  "), "hello___world");
        assert_eq!(sanitize_name("drop-table-foo"), "drop_table_foo");
    }

    #[test]
    fn create_new_writes_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = create_new_migration(dir.path(), "create users").unwrap();
        assert!(path.exists());
        let filename = path.file_name().unwrap().to_str().unwrap();
        assert!(filename.ends_with("_create_users.sql"));
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("-- migration: create_users"));
    }

    #[test]
    fn create_new_empty_name_fails() {
        let dir = tempfile::tempdir().unwrap();
        assert!(create_new_migration(dir.path(), "   ").is_err());
    }
}
