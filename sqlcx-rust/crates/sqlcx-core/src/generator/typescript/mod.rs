pub mod typebox;
pub mod bun_sql;

use crate::error::Result;
use crate::config::TargetConfig;
use crate::generator::{GeneratedFile, LanguagePlugin};
use crate::ir::SqlcxIR;

pub struct TypeScriptPlugin {
    pub schema_name: String,
    pub driver_name: String,
}

impl TypeScriptPlugin {
    pub fn new(schema: &str, driver: &str) -> Result<Self> {
        Ok(Self {
            schema_name: schema.to_string(),
            driver_name: driver.to_string(),
        })
    }
}

impl LanguagePlugin for TypeScriptPlugin {
    fn generate(&self, _ir: &SqlcxIR, _config: &TargetConfig) -> Result<Vec<GeneratedFile>> {
        todo!("Implemented in Task 10")
    }
}
