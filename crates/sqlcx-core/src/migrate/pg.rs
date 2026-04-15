use crate::error::{Result, SqlcxError};
use crate::migrate::driver::MigrationDriver;
use crate::migrate::file::MigrationFile;
use crate::migrate::state::{
    AppliedMigration, CREATE_STATE_TABLE_SQL, INSERT_APPLIED_SQL, SELECT_APPLIED_SQL,
};
use postgres::{Client, NoTls};

pub struct PostgresDriver {
    client: Client,
}

impl PostgresDriver {
    pub fn connect(database_url: &str) -> Result<Self> {
        let client = Client::connect(database_url, NoTls)
            .map_err(|e| SqlcxError::Migrate(format!("connection failed: {e}")))?;
        Ok(Self { client })
    }
}

impl MigrationDriver for PostgresDriver {
    fn ensure_state_table(&mut self) -> Result<()> {
        self.client
            .batch_execute(CREATE_STATE_TABLE_SQL)
            .map_err(|e| SqlcxError::Migrate(format!("ensure_state_table: {e}")))?;
        Ok(())
    }

    fn list_applied(&mut self) -> Result<Vec<AppliedMigration>> {
        let rows = self
            .client
            .query(SELECT_APPLIED_SQL, &[])
            .map_err(|e| SqlcxError::Migrate(format!("list_applied: {e}")))?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(AppliedMigration {
                version: row.get(0),
                name: row.get(1),
                checksum: row.get(2),
            });
        }
        Ok(out)
    }

    fn apply_migration(&mut self, file: &MigrationFile) -> Result<()> {
        let mut tx = self
            .client
            .transaction()
            .map_err(|e| SqlcxError::Migrate(format!("begin tx: {e}")))?;
        tx.batch_execute(&file.content)
            .map_err(|e| SqlcxError::Migrate(format!("{}: {e}", file.version)))?;
        tx.execute(
            INSERT_APPLIED_SQL,
            &[&file.version, &file.name, &file.checksum],
        )
        .map_err(|e| SqlcxError::Migrate(format!("record state {}: {e}", file.version)))?;
        tx.commit()
            .map_err(|e| SqlcxError::Migrate(format!("commit {}: {e}", file.version)))?;
        Ok(())
    }
}
