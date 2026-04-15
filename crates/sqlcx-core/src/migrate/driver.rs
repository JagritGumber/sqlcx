use crate::error::Result;
use crate::migrate::file::MigrationFile;
use crate::migrate::state::AppliedMigration;

/// Abstraction over a database connection capable of applying migrations.
///
/// Implementations are responsible for:
/// - Creating the state table if it does not already exist
/// - Listing the currently-applied migrations
/// - Applying a single migration file and recording state in one transaction
pub trait MigrationDriver {
    fn ensure_state_table(&mut self) -> Result<()>;
    fn list_applied(&mut self) -> Result<Vec<AppliedMigration>>;
    fn apply_migration(&mut self, file: &MigrationFile) -> Result<()>;
}
