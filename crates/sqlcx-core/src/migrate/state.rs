#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedMigration {
    pub version: String,
    pub name: String,
    pub checksum: String,
}

pub const STATE_TABLE: &str = "_sqlcx_migrations";

pub const CREATE_STATE_TABLE_SQL: &str = "\
CREATE TABLE IF NOT EXISTS _sqlcx_migrations (
  version    TEXT PRIMARY KEY,
  name       TEXT NOT NULL,
  checksum   TEXT NOT NULL,
  applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
)";

pub const SELECT_APPLIED_SQL: &str =
    "SELECT version, name, checksum FROM _sqlcx_migrations ORDER BY version";

pub const INSERT_APPLIED_SQL: &str =
    "INSERT INTO _sqlcx_migrations (version, name, checksum) VALUES ($1, $2, $3)";
