use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Clone, Debug)]
pub struct SqlcxConfig {
    pub sql: String,
    pub parser: String,
    pub targets: Vec<TargetConfig>,
    #[serde(default)]
    pub overrides: HashMap<String, String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct TargetConfig {
    pub language: String,
    pub out: String,
    pub schema: String,
    pub driver: String,
}
