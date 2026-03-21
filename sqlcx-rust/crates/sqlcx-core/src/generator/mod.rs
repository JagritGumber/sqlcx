pub mod typescript;

use crate::config::TargetConfig;
use crate::error::Result;
use crate::ir::{Overrides, SqlcxIR};

pub struct GeneratedFile {
    pub path: String,
    pub content: String,
}

pub trait SchemaGenerator {
    fn generate(&self, ir: &SqlcxIR, overrides: &Overrides) -> Result<GeneratedFile>;
}

pub trait DriverGenerator {
    fn generate(&self, ir: &SqlcxIR) -> Result<Vec<GeneratedFile>>;
}

pub trait LanguagePlugin {
    fn generate(&self, ir: &SqlcxIR, config: &TargetConfig) -> Result<Vec<GeneratedFile>>;
}

pub fn resolve_language(name: &str, schema: &str, driver: &str) -> Result<Box<dyn LanguagePlugin>> {
    match name {
        "typescript" => Ok(Box::new(typescript::TypeScriptPlugin::new(schema, driver)?)),
        _ => Err(crate::error::SqlcxError::UnknownLanguage(name.to_string())),
    }
}
