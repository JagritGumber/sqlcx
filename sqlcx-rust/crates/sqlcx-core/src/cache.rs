use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

use crate::error::Result;
use crate::ir::SqlcxIR;

pub struct SqlFile {
    pub path: String,
    pub content: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CacheFile {
    hash: String,
    ir: SqlcxIR,
}

/// Compute SHA-256 hash of SQL files. Files are sorted by path, each path+content
/// separated by null bytes (matching the TS implementation).
pub fn compute_hash(files: &[SqlFile]) -> String {
    let mut sorted: Vec<&SqlFile> = files.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    let mut hasher = Sha256::new();
    for f in &sorted {
        hasher.update(f.path.as_bytes());
        hasher.update(b"\0");
        hasher.update(f.content.as_bytes());
        hasher.update(b"\0");
    }
    format!("{:x}", hasher.finalize())
}

/// Write IR cache atomically (temp file → rename).
pub fn write_cache(cache_dir: &Path, ir: &SqlcxIR, hash: &str) -> Result<()> {
    fs::create_dir_all(cache_dir)?;
    let data = CacheFile {
        hash: hash.to_string(),
        ir: ir.clone(),
    };
    let cache_path = cache_dir.join("ir.json");
    let temp_path = cache_path.with_extension("json.tmp");
    let json = serde_json::to_string(&data)?;
    fs::write(&temp_path, &json)?;
    fs::rename(&temp_path, &cache_path)?;
    Ok(())
}

/// Read cached IR. Returns None on miss (wrong hash, no file, corrupted).
pub fn read_cache(cache_dir: &Path, expected_hash: &str) -> Result<Option<SqlcxIR>> {
    let cache_path = cache_dir.join("ir.json");
    if !cache_path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&cache_path)?;
    let data: CacheFile = match serde_json::from_str(&content) {
        Ok(d) => d,
        Err(_) => return Ok(None),
    };
    if data.hash != expected_hash {
        return Ok(None);
    }
    Ok(Some(data.ir))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::*;

    fn sample_ir() -> SqlcxIR {
        SqlcxIR {
            tables: vec![],
            queries: vec![],
            enums: vec![],
        }
    }

    #[test]
    fn compute_hash_deterministic() {
        let files = vec![
            SqlFile {
                path: "a.sql".to_string(),
                content: "SELECT 1;".to_string(),
            },
            SqlFile {
                path: "b.sql".to_string(),
                content: "SELECT 2;".to_string(),
            },
        ];
        assert_eq!(compute_hash(&files), compute_hash(&files));
    }

    #[test]
    fn compute_hash_order_independent() {
        let a = vec![
            SqlFile {
                path: "b.sql".to_string(),
                content: "SELECT 2;".to_string(),
            },
            SqlFile {
                path: "a.sql".to_string(),
                content: "SELECT 1;".to_string(),
            },
        ];
        let b = vec![
            SqlFile {
                path: "a.sql".to_string(),
                content: "SELECT 1;".to_string(),
            },
            SqlFile {
                path: "b.sql".to_string(),
                content: "SELECT 2;".to_string(),
            },
        ];
        assert_eq!(compute_hash(&a), compute_hash(&b));
    }

    #[test]
    fn cache_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let cache_dir = dir.path().join(".sqlcx");
        let ir = sample_ir();
        write_cache(&cache_dir, &ir, "abc123").unwrap();
        let loaded = read_cache(&cache_dir, "abc123").unwrap();
        assert!(loaded.is_some());
    }

    #[test]
    fn cache_miss_on_hash_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let cache_dir = dir.path().join(".sqlcx");
        write_cache(&cache_dir, &sample_ir(), "v1").unwrap();
        assert!(read_cache(&cache_dir, "v2").unwrap().is_none());
    }

    #[test]
    fn cache_miss_on_no_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_cache(&dir.path().join(".sqlcx"), "any")
            .unwrap()
            .is_none());
    }
}
